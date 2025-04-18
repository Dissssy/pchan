use common::structs::{
    Banner, BoardWithThreads, CreatePost, CreateThread, SafeBoard, SafePost, ThreadWithPosts,
};
use gloo_net::http::Request;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Display;
use yew::prelude::*;
use yew_hooks::UseLocalStorageHandle;

mod token;

pub struct Api {
    pub token: AttrValue,
    #[cfg(feature = "cache-base")]
    pub cache: std::rc::Rc<async_lock::Mutex<typemap_ors::TypeMap>>,
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
            AttrValue::from(token.clone())
        } else {
            let t = Self::get_token().await?;
            token.set(t.to_string());
            t
        };

        Ok(Self {
            token,
            #[cfg(feature = "cache-base")]
            cache: std::rc::Rc::new(async_lock::Mutex::new(typemap_ors::TypeMap::new())),
        })
    }

    async fn get_token() -> Result<AttrValue, ApiError> {
        let token = token::get_token().await?;
        //
        Ok(token)
    }

    fn formatted_token(&self) -> String {
        format!("Bearer {}", self.token)
    }

    #[allow(unused_variables)]
    pub async fn get_boards(&self, override_cache: bool) -> Result<Vec<SafeBoard>, ApiError> {
        let ident = String::new();
        // attempt cache hit
        #[cfg(feature = "cache-boards")]
        let v: Option<Vec<SafeBoard>> = if override_cache {
            None
        } else {
            let mut cache = self.cache.lock().await;
            match cache.entry::<CachedValue<Vec<SafeBoard>>>() {
                typemap_ors::Entry::Occupied(val) => val.get().get(&ident).cloned(),
                typemap_ors::Entry::Vacant(hole) => hole
                    .insert(CachedValue::new(std::time::Duration::from_secs(300)))
                    .get(&ident)
                    .cloned(),
            }
        };

        #[cfg(not(feature = "cache-boards"))]
        let v = None;

        if let Some(v) = v {
            //
            Ok(v)
        } else {
            //
            // GET /api/v1/board -> Vec<Board>
            let token = self.formatted_token();
            match standard_get::<Vec<SafeBoard>>("/api/v1/board", &token).await {
                Ok(res) => {
                    #[cfg(feature = "cache-boards")]
                    {
                        let mut cache = self.cache.lock().await;
                        match cache.entry::<CachedValue<Vec<SafeBoard>>>() {
                            typemap_ors::Entry::Occupied(val) => {
                                val.into_mut().set(&ident, res.clone())
                            }
                            typemap_ors::Entry::Vacant(hole) => hole
                                .insert(CachedValue::new(std::time::Duration::from_secs(300)))
                                .set(&ident, res.clone()),
                        };
                    }
                    Ok(res)
                }
                Err(e) => {
                    gloo::console::error!(format!("Error getting boards: {}", *e));
                    Err(e)
                }
            }
        }
    }

    #[allow(unused_variables)]
    pub async fn get_board(
        &self,
        board: impl Display + ToString + Copy,
        override_cache: bool,
    ) -> Result<BoardWithThreads, ApiError> {
        let ident = format!("{}", board);
        // attempt cache hit
        #[cfg(feature = "cache-board")]
        let v = if override_cache {
            None
        } else {
            let mut cache = self.cache.lock().await;
            match cache.entry::<CachedValue<BoardWithThreads>>() {
                typemap_ors::Entry::Occupied(val) => val.get().get(&ident).cloned(),
                typemap_ors::Entry::Vacant(hole) => hole
                    .insert(CachedValue::new(std::time::Duration::from_secs(30)))
                    .get(&ident)
                    .cloned(),
            }
        };

        #[cfg(not(feature = "cache-board"))]
        let v = None;

        if let Some(v) = v {
            //
            Ok(v)
        } else {
            //
            // GET /api/v1/{} -> Board
            let token = self.formatted_token();
            match standard_get::<BoardWithThreads>(&format!("/api/v1/board/{}", board), &token)
                .await
            {
                Ok(res) => {
                    #[cfg(any(feature = "cache-board", feature = "cache-post"))]
                    let mut cache = self.cache.lock().await;
                    #[cfg(feature = "cache-board")]
                    {
                        let v = {
                            match cache.entry::<CachedValue<BoardWithThreads>>() {
                                typemap_ors::Entry::Occupied(val) => val.into_mut(),
                                typemap_ors::Entry::Vacant(hole) => hole
                                    .insert(CachedValue::new(std::time::Duration::from_secs(30))),
                            }
                        };
                        v.set(&ident, res.clone());
                    }
                    #[cfg(feature = "cache-post")]
                    {
                        let p = {
                            match cache.entry::<CachedValue<SafePost>>() {
                                typemap_ors::Entry::Occupied(val) => val.into_mut(),
                                typemap_ors::Entry::Vacant(hole) => hole
                                    .insert(CachedValue::new(std::time::Duration::from_secs(30))),
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
                    Ok(res)
                }
                Err(e) => {
                    gloo::console::error!(format!("Error getting board: {}", *e));
                    Err(e)
                }
            }
        }
    }

    #[allow(unused_variables)]
    pub async fn get_thread(
        &self,
        board: impl Display + ToString + Copy,
        thread: impl Display + ToString + Copy,
        override_cache: bool,
    ) -> Result<ThreadWithPosts, ApiError> {
        let ident = format!("{}-{}", board, thread);
        // attempt cache hit

        #[cfg(feature = "cache-thread")]
        let v = if override_cache {
            None
        } else {
            let mut cache = self.cache.lock().await;
            match cache.entry::<CachedValue<ThreadWithPosts>>() {
                typemap_ors::Entry::Occupied(val) => val.into_mut().get(&ident).cloned(),
                typemap_ors::Entry::Vacant(hole) => hole
                    .insert(CachedValue::new(std::time::Duration::from_secs(30)))
                    .get(&ident)
                    .cloned(),
            }
        };

        #[cfg(not(feature = "cache-thread"))]
        let v = None;

        if let Some(v) = v {
            Ok(v)
        } else {
            //
            // GET /api/v1/{}/{} -> Thread
            let token = self.formatted_token();
            match standard_get::<ThreadWithPosts>(
                &format!("/api/v1/board/{}/thread/{}", board, thread),
                &token,
            )
            .await
            {
                Ok(res) => {
                    #[cfg(any(feature = "cache-thread", feature = "cache-post"))]
                    let mut cache = self.cache.lock().await;
                    #[cfg(feature = "cache-thread")]
                    {
                        let v = {
                            match cache.entry::<CachedValue<ThreadWithPosts>>() {
                                typemap_ors::Entry::Occupied(val) => val.into_mut(),
                                typemap_ors::Entry::Vacant(hole) => hole
                                    .insert(CachedValue::new(std::time::Duration::from_secs(30))),
                            }
                        };
                        v.set(&ident, res.clone());
                    }
                    #[cfg(feature = "cache-post")]
                    {
                        let p = {
                            match cache.entry::<CachedValue<SafePost>>() {
                                typemap_ors::Entry::Occupied(val) => val.into_mut(),
                                typemap_ors::Entry::Vacant(hole) => hole
                                    .insert(CachedValue::new(std::time::Duration::from_secs(300))),
                            }
                        };

                        let ident = format!("{}-{}", board, res.thread_post.post_number);
                        p.set(&ident, res.thread_post.clone());

                        res.posts.iter().for_each(|post| {
                            let ident = format!("{}-{}", board, post.post_number);
                            p.set(&ident, post.clone());
                        });
                    }
                    Ok(res)
                }
                Err(e) => {
                    gloo::console::error!(format!("Error getting thread: {}", *e));
                    Err(e)
                }
            }
        }
    }

    #[allow(unused_variables)]
    pub async fn get_post(
        &self,
        board: impl Display + ToString + Copy,
        post: impl Display + ToString + Copy,
        override_cache: bool,
    ) -> Result<SafePost, ApiError> {
        let ident = format!("{}-{}", board, post);
        // attempt cache hit
        #[cfg(feature = "cache-post")]
        let v = if override_cache {
            None
        } else {
            let mut cache = self.cache.lock().await;
            match cache.entry::<CachedValue<SafePost>>() {
                typemap_ors::Entry::Occupied(val) => val.into_mut().get(&ident).cloned(),
                typemap_ors::Entry::Vacant(hole) => hole
                    .insert(CachedValue::new(std::time::Duration::from_secs(300)))
                    .get(&ident)
                    .cloned(),
            }
        };

        #[cfg(not(feature = "cache-post"))]
        let v = None;

        if let Some(v) = v {
            Ok(v)
        } else {
            let token = self.formatted_token();
            match standard_get::<SafePost>(
                &format!("/api/v1/board/{}/post/{}", board, post),
                &token,
            )
            .await
            {
                Ok(res) => {
                    #[cfg(feature = "cache-post")]
                    {
                        let mut cache = self.cache.lock().await;
                        let v = {
                            match cache.entry::<CachedValue<SafePost>>() {
                                typemap_ors::Entry::Occupied(val) => val.into_mut(),
                                typemap_ors::Entry::Vacant(hole) => hole
                                    .insert(CachedValue::new(std::time::Duration::from_secs(300))),
                            }
                        };
                        v.set(&ident, res.clone());
                    }
                    Ok(res)
                }
                Err(e) => {
                    gloo::console::error!(format!("Error getting post: {}", *e));
                    Err(e)
                }
            }
        }
    }

    pub async fn create_file(&self, file: web_sys::File) -> Result<AttrValue, ApiError> {
        let token = self.formatted_token();
        let form_data = web_sys::FormData::new()
            .map_err(|e| ApiError::Other(AttrValue::from(format!("{e:?}"))))?;

        form_data
            .append_with_blob("file", &file)
            .map_err(|e| ApiError::Other(AttrValue::from(format!("{e:?}"))))?;

        let raw_res = gloo_net::http::Request::post("/api/v1/file")
            .header("authorization", &token)
            .body(&form_data)
            .map_err(|e| ApiError::Gloo(AttrValue::from(format!("{e:?}"))))?
            .send()
            .await
            .map_err(|e| ApiError::Gloo(AttrValue::from(format!("{e}"))))?;
        let res = raw_res
            .text()
            .await
            .map_err(|e| ApiError::Gloo(AttrValue::from(format!("{e}"))))?;
        let file_id = serde_json::from_str::<String>(&res).map_err(|e| {
            if !raw_res.ok() {
                ApiError::Api(AttrValue::from(raw_res.status_text()))
            } else {
                ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {res}")))
            }
        })?;
        if file_id.contains(' ') {
            Err(ApiError::Api(AttrValue::from(file_id)))
        } else {
            Ok(AttrValue::from(file_id))
        }
    }

    pub async fn share_file(
        &self,
        file: &str, // /files/mimetype/id.ext
    ) -> Result<String, ApiError> {
        // PUT /api/v1/share/files/mimetype/id.ext
        let token = self.formatted_token();
        standard_put::<String, ()>(&format!("/api/v1/share{}", file), &token, &()).await
    }

    pub async fn create_thread(
        &self,
        board: impl Display + ToString + Copy,
        thread: CreateThread,
    ) -> Result<SafePost, ApiError> {
        let token = self.formatted_token();
        // let res = thread::create_thread(&token, board, thread).await;
        let res = standard_post::<SafePost, CreateThread>(
            &format!("/api/v1/board/{}/thread", board),
            &token,
            &thread,
        )
        .await;
        if let Ok(post) = &res {
            let _ = self
                .get_thread(board, &post.thread_post_number.to_string(), true)
                .await;
            let _ = self.get_board(board, true).await;
        }
        res
    }

    pub async fn create_post(
        &self,
        board: impl Display + ToString + Copy,
        thread: impl Display + ToString + Copy,
        post: CreatePost,
    ) -> Result<SafePost, ApiError> {
        let token = self.formatted_token();
        // let res = post::create_post(&token, board, thread, post).await;
        let res = standard_post::<SafePost, CreatePost>(
            &format!("/api/v1/board/{}/thread/{}", board, thread),
            &token,
            &post,
        )
        .await;
        if res.is_ok() {
            let _ = self.get_board(board, true).await;
            let _ = self.get_thread(board, thread, true).await;
        }
        res
    }

    pub async fn delete_post(
        &self,
        board: impl Display + ToString + Copy,
        post: impl Display + ToString + Copy,
    ) -> Result<i64, ApiError> {
        let token = self.formatted_token();
        let full_post = self.get_post(board, post, true).await?;
        // let res = post::delete_post(&token, board, post).await;
        let res =
            standard_delete::<i64>(&format!("/api/v1/board/{}/post/{}", board, post), &token).await;
        if res.is_ok() {
            let _ = self.get_board(board, true).await;
            let _ = self
                .get_thread(board, &full_post.thread_post_number.to_string(), true)
                .await;
        }
        res
    }

    pub async fn set_watching(
        &self,
        board: impl Display + ToString + Copy,
        post: impl Display + ToString + Copy,
        watching: bool,
    ) -> Result<bool, ApiError> {
        let token = self.formatted_token();
        // PUT /api/v1/board/{board_discriminator}/post/{post_number}/watching
        standard_put(
            &format!("/api/v1/board/{}/post/{}/watching", board, post),
            &token,
            &watching,
        )
        .await
    }

    pub async fn get_watching(
        &self,
        board: impl Display + ToString + Copy,
        post: impl Display + ToString + Copy,
    ) -> Result<bool, ApiError> {
        let token = self.formatted_token();
        // GET /api/v1/board/{board_discriminator}/post/{post_number}/watching
        standard_get(
            &format!("/api/v1/board/{}/post/{}/watching", board, post),
            &token,
        )
        .await
    }

    pub async fn get_banner(&self, board: impl Display + ToString) -> Result<Banner, ApiError> {
        let token = self.formatted_token();

        // board::get_banner(&token, board).await
        standard_get(&format!("/api/v1/board/{}/banner", board), &token).await
    }

    pub async fn generate_board_invite_link(
        &self,
        board: impl Display + ToString,
        invite_name: impl Display + ToString,
    ) -> Result<String, ApiError> {
        let token = self.formatted_token();
        // board::generate_invite_link(&token, board, invite_name).await
        let res: String = standard_put(
            &format!("/api/v1/board/{}/invite?info={}", board, invite_name),
            &token,
            &(),
        )
        .await?;

        if res.contains(' ') {
            Err(ApiError::Api(AttrValue::from(res)))
        } else {
            Ok(res)
        }
    }

    pub async fn generate_moderator_invite_link(
        &self,
        board: impl Display + ToString,
        invite_name: impl Display + ToString,
    ) -> Result<String, ApiError> {
        let token = self.formatted_token();
        // board::generate_moderator_invite_link(&token, board, invite_name).await
        let res: String = standard_put(
            &format!("/api/v1/board/{}/moderator?info={}", board, invite_name),
            &token,
            &(),
        )
        .await?;

        if res.contains(' ') {
            Err(ApiError::Api(AttrValue::from(res)))
        } else {
            Ok(res)
        }
    }

    pub async fn consume_code(&self, invite_code: impl Display + ToString) -> Result<(), ApiError> {
        let token = self.formatted_token();
        // board::consume_code(&token, board, invite_code).await
        let res: String = standard_patch(
            &format!("/api/v1/consume_code?info={}", invite_code),
            &token,
            &(),
        )
        .await?;
        if res == "ok" {
            Ok(())
        } else {
            Err(ApiError::Api(AttrValue::from(res)))
        }
    }

    #[cfg(feature = "cache-base")]
    pub fn insert_thread_to_cache(&self, thread: ThreadWithPosts) {
        let ident = format!(
            "{}-{}",
            thread.thread_post.board_discriminator, thread.thread_post.thread_post_number
        );
        if let Some(mut cache) = self.cache.try_lock() {
            let v = {
                match cache.entry::<CachedValue<ThreadWithPosts>>() {
                    typemap_ors::Entry::Occupied(val) => val.into_mut(),
                    typemap_ors::Entry::Vacant(hole) => {
                        hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                    }
                }
            };
            v.set(&ident, thread);
        }
    }

    #[cfg(feature = "cache-base")]
    pub fn insert_post_to_cache(&self, post: SafePost) {
        let ident = format!("{}-{}", post.board_discriminator, post.thread_post_number);
        if let Some(mut cache) = self.cache.try_lock() {
            let v = {
                match cache.entry::<CachedValue<SafePost>>() {
                    typemap_ors::Entry::Occupied(val) => val.into_mut(),
                    typemap_ors::Entry::Vacant(hole) => {
                        hole.insert(CachedValue::new(std::time::Duration::from_secs(300)))
                    }
                }
            };
            v.set(&ident, post);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    Gloo(AttrValue),
    Serde(AttrValue),
    Api(AttrValue),
    Other(AttrValue),
    // TODO: add error types :(
}

impl std::ops::Deref for ApiError {
    type Target = AttrValue;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Gloo(v) => v,
            Self::Serde(v) => v,
            Self::Api(v) => v,
            Self::Other(v) => v,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiState<T> {
    Pending,
    Loading,
    Loaded(T),
    ContextError(AttrValue),
    Error(ApiError),
}

impl<T> ApiState<T> {
    pub fn standard_html<F>(&self, source: &'static str, then: F) -> Result<Html, ApiError>
    where
        F: Fn(&T) -> Html,
    {
        match self {
            Self::Pending => Ok(html! {}),
            Self::Loading => Ok(html! {
                <crate::components::Spinner />
            }),
            Self::ContextError(s) => Ok(html! {
                <crate::components::ContextError cause={s} source={source} />
            }),
            Self::Error(e) => Err(e.clone()),
            Self::Loaded(data) => Ok(then(data)),
        }
    }
    pub fn get_or(&self, other: T) -> T
    where
        T: Clone,
    {
        match self {
            Self::Loaded(data) => data.clone(),
            _ => other,
        }
    }
}

#[cfg(feature = "cache-base")]
pub struct CachedValue<T> {
    values: std::collections::HashMap<String, (wasm_timer::Instant, T)>,
    ttl: std::time::Duration,
}

#[cfg(feature = "cache-base")]
impl<T> Default for CachedValue<T> {
    fn default() -> Self {
        Self {
            values: std::collections::HashMap::new(),
            ttl: std::time::Duration::from_secs(30),
        }
    }
}

#[cfg(feature = "cache-base")]
impl<T: 'static> typemap_ors::Key for CachedValue<T> {
    type Value = Self;
}

#[cfg(feature = "cache-base")]
impl<T> CachedValue<T> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            values: std::collections::HashMap::new(),
            ttl,
        }
    }
    pub fn get(&self, identifier: impl Display + ToString) -> Option<&T> {
        // gloo::console::log!(format!(
        //     "getting \"{}\" for \"{}\"",
        //     identifier,
        //     std::any::type_name::<T>()
        // ));
        if let Some((instant, value)) = self.values.get(&identifier.to_string()) {
            if instant.elapsed() < self.ttl {
                // gloo::console::log!(format!(
                //     "retrieved \"{}\" for \"{}\" from cache",
                //     identifier,
                //     std::any::type_name::<T>()
                //         .split("::")
                //         .last()
                //         .unwrap_or("")
                //         .trim_end_matches('>')
                // ));
                return Some(value);
            }
        }
        None
    }
    pub fn set(&mut self, identifier: impl Display + ToString + Copy, value: T) {
        // gloo::console::log!(format!(
        //     "setting \"{}\" for \"{}\"",
        //     identifier,
        //     std::any::type_name::<T>()
        // ));
        match self.values.entry(identifier.to_string()) {
            std::collections::hash_map::Entry::Vacant(hole) => {
                //
                hole.insert((wasm_timer::Instant::now(), value));
            }
            std::collections::hash_map::Entry::Occupied(mut val) => {
                let (t, _) = val.get();
                if t.elapsed() < self.ttl {
                    return;
                }
                //
                val.insert((wasm_timer::Instant::now(), value));
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

pub async fn standard_get<T>(path: &str, token: &str) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let res = Request::get(path)
        .header("authorization", token)
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(AttrValue::from(v)),
        Err(_) => ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {res}"))),
    })
}

pub async fn standard_delete<T>(path: &str, token: &str) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let res = Request::delete(path)
        .header("authorization", token)
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(AttrValue::from(v)),
        Err(_) => ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {res}"))),
    })
}

pub async fn standard_post<T, E>(path: &str, token: &str, data: &E) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    E: Serialize,
{
    let res = Request::post(path)
        .header("authorization", token)
        .json(data)
        .map_err(|e| ApiError::Serde(AttrValue::from(e.to_string())))?
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(AttrValue::from(v)),
        Err(_) => ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {res}"))),
    })
}

pub async fn standard_put<T, E>(path: &str, token: &str, data: &E) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    E: Serialize,
{
    let res = Request::put(path)
        .header("authorization", token)
        .json(data)
        .map_err(|e| ApiError::Serde(AttrValue::from(e.to_string())))?
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(AttrValue::from(v)),
        Err(_) => ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {res}"))),
    })
}

pub async fn standard_patch<T, E>(path: &str, token: &str, data: &E) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    E: Serialize,
{
    let res = Request::patch(path)
        .header("authorization", token)
        .json(data)
        .map_err(|e| ApiError::Serde(AttrValue::from(e.to_string())))?
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(AttrValue::from(v)),
        Err(_) => ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {res}"))),
    })
}
