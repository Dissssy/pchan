use std::{
    collections::{self, HashMap},
    sync::Arc,
};

use async_lock::Mutex;
use common::structs::{
    BoardWithThreads, CreatePost, CreateThread, SafeBoard, SafePost, ThreadWithPosts,
};
use typemap::{Key, TypeMap};
use wasm_timer::Instant;
use yew::prelude::*;
use yew_hooks::UseLocalStorageHandle;

mod board;
mod post;
mod thread;
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
        //
        Ok(token)
    }

    fn formatted_token(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub async fn get_boards(&self) -> Result<Vec<SafeBoard>, ApiError> {
        let ident = "".to_owned();
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

        if let Some(v) = v.get(&ident).cloned() {
            //
            Ok(v)
        } else {
            //
            // GET /api/v1/board -> Vec<Board>
            let token = self.formatted_token();
            let res = board::get_boards(&token).await;
            if let Ok(res) = &res {
                v.set(&ident, res.clone());
            }
            res
        }
    }

    pub async fn get_board(&self, board: &str) -> Result<BoardWithThreads, ApiError> {
        let ident = format!("{}", board);
        // attempt cache hit
        let mut cache = self.cache.lock().await;
        let v = {
            match cache.entry::<CachedValue<BoardWithThreads>>() {
                typemap::Entry::Occupied(val) => val.into_mut(),
                typemap::Entry::Vacant(hole) => {
                    hole.insert(CachedValue::new(std::time::Duration::from_secs(30)))
                }
            }
        };

        if let Some(v) = v.get(&ident).cloned() {
            //
            Ok(v)
        } else {
            //
            // GET /api/v1/{} -> Board
            let token = self.formatted_token();
            let res = board::get_board(&token, board).await;
            if let Ok(res) = &res {
                v.set(&ident, res.clone());
                let p = {
                    match cache.entry::<CachedValue<SafePost>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(30)))
                        }
                    }
                };

                res.threads.iter().for_each(|res| {
                    let ident = format!("{}-{}", board, res.thread_post.post_number);
                    p.set(&ident, res.thread_post.clone());

                    res.posts.iter().for_each(|post| {
                        let ident = format!("{}-{}", board, post.post_number);
                        p.set(&ident, post.clone());
                    });
                });
            }
            res
        }
    }

    pub async fn get_thread(&self, board: &str, thread: &str) -> Result<ThreadWithPosts, ApiError> {
        let ident = format!("{}-{}", board, thread);
        // attempt cache hit

        let mut cache = self.cache.lock().await;
        let v = {
            match cache.entry::<CachedValue<ThreadWithPosts>>() {
                typemap::Entry::Occupied(val) => val.into_mut(),
                typemap::Entry::Vacant(hole) => {
                    hole.insert(CachedValue::new(std::time::Duration::from_secs(30)))
                }
            }
        };

        if let Some(v) = v.get(&ident).cloned() {
            Ok(v)
        } else {
            //
            // GET /api/v1/{}/{} -> Thread
            let token = self.formatted_token();
            let res = thread::get_thread(&token, board, thread).await;
            if let Ok(res) = &res {
                v.set(&ident, res.clone());
                let p = {
                    match cache.entry::<CachedValue<SafePost>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };

                let ident = format!("{}-{}", board, res.thread_post.post_number);
                p.set(&ident, res.thread_post.clone());

                res.posts.iter().for_each(|post| {
                    let ident = format!("{}-{}", board, post.post_number);
                    p.set(&ident, post.clone());
                });
            }
            res
        }
    }

    pub async fn get_post(&self, board: &str, post: &str) -> Result<SafePost, ApiError> {
        let ident = format!("{}-{}", board, post);
        // attempt cache hit
        let mut cache = self.cache.lock().await;
        let v = {
            match cache.entry::<CachedValue<SafePost>>() {
                typemap::Entry::Occupied(val) => val.into_mut(),
                typemap::Entry::Vacant(hole) => {
                    hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                }
            }
        };

        if let Some(v) = v.get(&ident).cloned() {
            //
            Ok(v)
        } else {
            //
            // GET /api/v1/{}/{} -> Post
            let token = self.formatted_token();
            let res = post::get_post(&token, board, post).await;
            if let Ok(res) = &res {
                v.set(&ident, res.clone());
            }
            res
        }
    }

    pub async fn create_file(&self, file: web_sys::File) -> Result<String, ApiError> {
        let token = self.formatted_token();
        let form_data = web_sys::FormData::new().map_err(|e| ApiError::Other(format!("{e:?}")))?;

        form_data
            .append_with_blob("file", &file)
            .map_err(|e| ApiError::Other(format!("{e:?}")))?;

        let raw_res = gloo_net::http::Request::post("/api/v1/file")
            .header("authorization", &token)
            .body(&form_data)
            .send()
            .await
            .map_err(|e| ApiError::Gloo(format!("{e:?}")))?;
        let res = raw_res
            .text()
            .await
            .map_err(|e| ApiError::Gloo(format!("{e:?}")))?;
        let file_id = serde_json::from_str::<String>(&res).map_err(|e| {
            if !raw_res.ok() {
                ApiError::Api(raw_res.status_text())
            } else {
                ApiError::Serde(format!("{e:?} SERDE ERROR FROM {res}"))
            }
        })?;
        if file_id.contains(' ') {
            Err(ApiError::Api(file_id))
        } else {
            Ok(file_id)
        }
    }

    pub async fn create_thread(
        &self,
        board: &str,
        thread: CreateThread,
    ) -> Result<SafePost, ApiError> {
        let token = self.formatted_token();
        let res = thread::create_thread(&token, board, thread).await;
        if let Ok(post) = &res {
            {
                // invalidate board cache
                let mut cache = self.cache.lock().await;
                let v = {
                    match cache.entry::<CachedValue<BoardWithThreads>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };
                v.remove(board);
            }
            // now get the thread and board to cache them
            let _ = self
                .get_thread(board, &post.thread_post_number.to_string())
                .await;
            let _ = self.get_board(board).await;
        }
        res
    }

    pub async fn create_post(
        &self,
        board: &str,
        thread: &str,
        post: CreatePost,
    ) -> Result<SafePost, ApiError> {
        let token = self.formatted_token();
        let res = post::create_post(&token, board, thread, post).await;
        if res.is_ok() {
            {
                // invalidate thread cache
                let mut cache = self.cache.lock().await;
                let v = {
                    match cache.entry::<CachedValue<ThreadWithPosts>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };
                let ident = format!("{}-{}", board, thread);
                v.remove(&ident);

                // invalidate board cache
                let v = {
                    match cache.entry::<CachedValue<BoardWithThreads>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };
                let ident = format!("{}", board);
                v.remove(&ident);
            }
            // now get the board and thread again for the cache
            let _ = self.get_board(board).await;
            let _ = self.get_thread(board, thread).await;
        }
        res
    }

    pub async fn delete_post(&self, board: &str, post: &str) -> Result<i64, ApiError> {
        let token = self.formatted_token();
        let full_post = self.get_post(board, post).await?;
        let res = post::delete_post(&token, board, post).await;
        if res.is_ok() {
            {
                // remove from cache
                let mut cache = self.cache.lock().await;
                let v = {
                    match cache.entry::<CachedValue<SafePost>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };
                let ident = format!("{}-{}", board, post);
                v.remove(&ident);

                // remove thread from cache
                let v = {
                    match cache.entry::<CachedValue<ThreadWithPosts>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };
                let ident = format!("{}-{}", board, full_post.thread_post_number);
                v.remove(&ident);

                // remove board from cache
                let v = {
                    match cache.entry::<CachedValue<BoardWithThreads>>() {
                        typemap::Entry::Occupied(val) => val.into_mut(),
                        typemap::Entry::Vacant(hole) => {
                            hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                        }
                    }
                };
                let ident = format!("{}", board);
                v.remove(&ident);
            }
            // now get the board and thread again for the cache
            let _ = self.get_board(board).await;
            let _ = self
                .get_thread(board, &full_post.thread_post_number.to_string())
                .await;
        }
        res
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    Gloo(String),
    Serde(String),
    Api(String),
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
    pub fn standard_html<F>(&self, source: &'static str, then: F) -> Result<Html, ApiError>
    where
        F: Fn(&T) -> Html,
    {
        match self {
            ApiState::Pending => Ok(html! {}),
            ApiState::Loading => Ok(html! {
                <crate::components::Spinner />
            }),
            ApiState::ContextError(s) => Ok(html! {
                <crate::components::ContextError cause={s.clone()} source={source} />
            }),
            ApiState::Error(e) => Err(e.clone()),
            ApiState::Loaded(data) => Ok(then(data)),
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
        // gloo::console::log!(format!(
        //     "getting \"{}\" for \"{}\"",
        //     identifier,
        //     std::any::type_name::<T>()
        // ));
        if let Some((instant, value)) = self.values.get(identifier) {
            if instant.elapsed() < self.ttl {
                return Some(value);
            }
        }
        None
    }
    pub fn set(&mut self, identifier: &str, value: T) {
        // gloo::console::log!(format!(
        //     "setting \"{}\" for \"{}\"",
        //     identifier,
        //     std::any::type_name::<T>()
        // ));
        match self.values.entry(identifier.to_string()) {
            collections::hash_map::Entry::Vacant(hole) => {
                //
                hole.insert((Instant::now(), value));
            }
            collections::hash_map::Entry::Occupied(mut val) => {
                let (t, _) = val.get();
                if t.elapsed() < self.ttl {
                    return;
                }
                //
                val.insert((Instant::now(), value));
            }
        }
    }
    pub fn remove(&mut self, identifier: &str) {
        // gloo::console::log!(format!(
        //     "removing \"{}\" for \"{}\"",
        //     identifier,
        //     std::any::type_name::<T>()
        // ));
        self.values.remove(identifier);
    }
}
