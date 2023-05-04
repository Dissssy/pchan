use std::{collections::HashMap, sync::Arc};

use async_lock::Mutex;
use common::structs::{BoardWithThreads, SafeBoard};
use typemap::{Key, TypeMap};
use wasm_timer::Instant;
use yew::prelude::*;
use yew_hooks::UseLocalStorageHandle;

mod board;
mod token;

#[derive(Clone)]
pub struct Api {
    pub token: String,
    pub cache: Arc<Mutex<TypeMap>>,
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api")
            .field("token", &self.token)
            .field("cache", &"Mutex<TypeMap>")
            .finish()
    }
}

impl PartialEq for Api {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl Api {
    pub async fn new(token: UseLocalStorageHandle<String>) -> Result<Self, ApiError> {
        let token = if let Some(token) = &*token {
            token.clone()
        } else {
            let t = Self::get_token().await?;
            token.set(t.clone());
            t
        };

        Ok(Self {
            token,
            cache: Arc::new(Mutex::new(TypeMap::new())),
        })
    }
    async fn get_token() -> Result<String, ApiError> {
        let token = token::get_token().await?;
        gloo::console::log!(format!("token: {}", token));
        Ok(token)
    }

    fn formatted_token(&self) -> String {
        format!("Bearer {}", self.token)
    }

    // every api call is going to be called dispatch_<instruction> that takes a UseStateHandle<Result<T>> and returns a future.
    pub async fn get_boards(&self) -> Result<Vec<SafeBoard>, ApiError> {
        // attempted cache hit
        // attempt cache hit
        let mut cache = self.cache.lock().await;
        let v = {
            match cache.entry::<CachedValue<Vec<SafeBoard>>>() {
                typemap::Entry::Occupied(val) => val.into_mut(),
                typemap::Entry::Vacant(hole) => {
                    hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                }
            }
        };

        if let Some(v) = v.get("").cloned() {
            gloo::console::log!("get_boards cache hit");
            Ok(v)
        } else {
            gloo::console::log!("get_boards cache miss");
            // GET /api/v1/board -> Vec<Board>
            let token = self.formatted_token();
            let res = board::get_boards(&token).await;
            if let Ok(res) = &res {
                v.set("", res.clone());
            }
            res
        }
    }

    pub async fn get_board(&self, board: &str) -> Result<BoardWithThreads, ApiError> {
        // attempt cache hit
        let mut cache = self.cache.lock().await;
        let v = {
            match cache.entry::<CachedValue<BoardWithThreads>>() {
                typemap::Entry::Occupied(val) => val.into_mut(),
                typemap::Entry::Vacant(hole) => {
                    hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                }
            }
        };

        if let Some(v) = v.get(board).cloned() {
            gloo::console::log!("get_board cache hit");
            Ok(v)
        } else {
            gloo::console::log!("get_board cache miss");
            // GET /api/v1/{} -> Board
            let token = self.formatted_token();
            let res = board::get_board(&token, board).await;
            if let Ok(res) = &res {
                v.set(board, res.clone());
            }
            res
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    Gloo(String),
    Serde(String),
    Other(String),
    // TODO: add error types :(
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiState<T> {
    Pending,
    Loading,
    Loaded(T),
    ContextError(String),
    Error(ApiError),
}

impl<T> ApiState<T> {
    pub fn standard_html<F>(&self, source: &'static str, then: F) -> Html
    where
        F: Fn(&T) -> Html,
    {
        match self {
            ApiState::Pending => {
                html! {}
            }
            ApiState::Loading => {
                html! {
                    <crate::components::Spinner />
                }
            }
            ApiState::ContextError(s) => {
                html! {
                    <crate::components::ContextError cause={s.clone()} source={source} />
                }
            }
            ApiState::Error(e) => {
                gloo::console::log!(format!("Error: {:?}", e));
                html! {}
            }
            ApiState::Loaded(data) => then(data),
        }
    }
}

pub struct CachedValue<T> {
    values: HashMap<String, (Instant, T)>,
    ttl: std::time::Duration,
}

impl<T> Default for CachedValue<T> {
    fn default() -> Self {
        CachedValue {
            values: HashMap::new(),
            ttl: std::time::Duration::from_secs(30),
        }
    }
}

impl<T: 'static> Key for CachedValue<T> {
    type Value = CachedValue<T>;
}

impl<T> CachedValue<T> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            values: HashMap::new(),
            ttl,
        }
    }
    pub fn get(&self, identifier: &str) -> Option<&T> {
        if let Some((instant, value)) = self.values.get(identifier) {
            if instant.elapsed() < self.ttl {
                return Some(value);
            }
        }
        None
    }
    pub fn set(&mut self, identifier: &str, value: T) {
        self.values
            .insert(identifier.to_string(), (Instant::now(), value));
    }
}
