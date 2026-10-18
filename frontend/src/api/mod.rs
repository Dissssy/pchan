use common::structs::{
    Banner, BoardWithThreads, CreatePost, CreateThread, SafeBoard, SafePost, ThreadWithPosts,
};
use gloo_net::http::Request;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Display;
use yew::prelude::*;

/// The browser session cookie is the credential now, so `Api` holds none. It used to carry a
/// bearer token fetched from `/api/v1/token` and cached in localStorage, which meant an XSS
/// could read a permanently valid credential.
pub struct Api {
    #[cfg(feature = "cache-base")]
    pub cache: std::rc::Rc<async_lock::Mutex<typemap_ors::TypeMap>>,
}

impl std::fmt::Debug for Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api")
            .field("cache", &"Mutex<TypeMap>")
            .finish()
    }
}

impl PartialEq for Api {
    // All instances are equivalent now that there is no per-instance credential.
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Default for Api {
    fn default() -> Self {
        Self::new()
    }
}

impl Api {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "cache-base")]
            cache: std::rc::Rc::new(async_lock::Mutex::new(typemap_ors::TypeMap::new())),
        }
    }

    // ------------------------------------------------------------ membership

    /// Role and ban state for the current viewer. Never cached: a ban or a role change has
    /// to take effect on the next page load, not whenever a cache happens to expire.
    pub async fn get_me(&self) -> Result<common::structs::MeInfo, ApiError> {
        standard_get::<common::structs::MeInfo>("/api/v1/me").await
    }

    /// Submit a join request. Authorised by the join ticket cookie, not a session — the
    /// applicant does not have one yet.
    pub async fn submit_access_request(&self, disclosure: String) -> Result<String, ApiError> {
        standard_post::<String, _>(
            "/api/v1/access_request",
            &common::structs::AccessRequest { disclosure },
        )
        .await
    }

    pub async fn list_access_requests(
        &self,
    ) -> Result<Vec<common::structs::JoinRequestSummary>, ApiError> {
        standard_get::<Vec<common::structs::JoinRequestSummary>>("/api/v1/access_requests").await
    }

    /// `action` is "approve" or "decline".
    pub async fn decide_access_request(
        &self,
        id: &str,
        action: &str,
    ) -> Result<String, ApiError> {
        standard_post::<String, _>(
            &format!("/api/v1/access_requests/{id}/{action}"),
            &serde_json::Value::Null,
        )
        .await
    }

    /// Ban whoever wrote this post. The server resolves them; the response names nobody.
    pub async fn ban_post_author(
        &self,
        board: &str,
        post_number: i64,
    ) -> Result<String, ApiError> {
        standard_post::<String, _>(
            &format!("/api/v1/board/{board}/post/{post_number}/ban_author"),
            &serde_json::Value::Null,
        )
        .await
    }

    /// Replace your own author seed, orphaning everything you have posted.
    pub async fn rotate_author_seed(&self) -> Result<String, ApiError> {
        standard_post::<String, _>("/api/v1/me/rotate_author_seed", &serde_json::Value::Null).await
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
            match standard_get::<Vec<SafeBoard>>("/api/v1/board").await {
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
            match standard_get::<BoardWithThreads>(&format!("/api/v1/board/{}", board))
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
            match standard_get::<ThreadWithPosts>(
                &format!("/api/v1/board/{}/thread/{}", board, thread),
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
            match standard_get::<SafePost>(
                &format!("/api/v1/board/{}/post/{}", board, post),
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
        let form_data = web_sys::FormData::new()
            .map_err(|e| ApiError::Other(AttrValue::from(format!("{e:?}"))))?;

        form_data
            .append_with_blob("file", &file)
            .map_err(|e| ApiError::Other(AttrValue::from(format!("{e:?}"))))?;

        let raw_res = gloo_net::http::Request::post("/api/v1/file")
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
        standard_put::<String, ()>(&format!("/api/v1/share{}", file), &()).await
    }

    pub async fn create_thread(
        &self,
        board: impl Display + ToString + Copy,
        thread: CreateThread,
    ) -> Result<SafePost, ApiError> {
        // let res = thread::create_thread( board, thread).await;
        let res = standard_post::<SafePost, CreateThread>(
            &format!("/api/v1/board/{}/thread", board),
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
        // let res = post::create_post( board, thread, post).await;
        let res = standard_post::<SafePost, CreatePost>(
            &format!("/api/v1/board/{}/thread/{}", board, thread),
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
        let full_post = self.get_post(board, post, true).await?;
        // let res = post::delete_post( board, post).await;
        let res =
            standard_delete::<i64>(&format!("/api/v1/board/{}/post/{}", board, post)).await;
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
        // PUT /api/v1/board/{board_discriminator}/post/{post_number}/watching
        standard_put(
            &format!("/api/v1/board/{}/post/{}/watching", board, post),
            &watching,
        )
        .await
    }

    pub async fn get_watching(
        &self,
        board: impl Display + ToString + Copy,
        post: impl Display + ToString + Copy,
    ) -> Result<bool, ApiError> {
        // GET /api/v1/board/{board_discriminator}/post/{post_number}/watching
        standard_get(
            &format!("/api/v1/board/{}/post/{}/watching", board, post),
        )
        .await
    }

    pub async fn get_banner(&self, board: impl Display + ToString) -> Result<Banner, ApiError> {

        // board::get_banner( board).await
        standard_get(&format!("/api/v1/board/{}/banner", board)).await
    }

    pub async fn generate_board_invite_link(
        &self,
        board: impl Display + ToString,
        invite_name: impl Display + ToString,
    ) -> Result<String, ApiError> {
        // board::generate_invite_link( board, invite_name).await
        let res: String = standard_put(
            &format!("/api/v1/board/{}/invite?info={}", board, invite_name),
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
        // board::generate_moderator_invite_link( board, invite_name).await
        let res: String = standard_put(
            &format!("/api/v1/board/{}/moderator?info={}", board, invite_name),
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
        // board::consume_code( board, invite_code).await
        let res: String = standard_patch(
            &format!("/api/v1/consume_code?info={}", invite_code),
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
    /// The server said 401. Distinct from the other variants because the UI reacts to it by
    /// showing the login prompt rather than an error.
    ///
    /// Previously there was no such signal: the backend's 401 body is a bare JSON string, so
    /// it deserialized cleanly into whatever the caller expected and the failure was silent.
    Unauthenticated(AttrValue),
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
            Self::Unauthenticated(v) => v,
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
            // Handled here rather than handed back as an error: callers treat Err as "this
            // page is broken" and navigate to /404, which is the wrong response to simply
            // not being signed in.
            Self::Error(ApiError::Unauthenticated(_)) => Ok(html! {
                <crate::components::SignIn />
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

/// Turn a response into a value, mapping a 401 to [`ApiError::Unauthenticated`].
///
/// The status has to be checked *before* parsing: the backend's 401 body is a bare JSON
/// string, which deserializes cleanly into `String`-shaped targets and would otherwise be
/// mistaken for a successful result.
async fn handle_response<T>(res: gloo_net::http::Response) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    if res.status() == 401 {
        return Err(ApiError::Unauthenticated(AttrValue::Static("Not signed in")));
    }

    let body = res.text().await.map_err(|e| match e {
        gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
        v => ApiError::Other(AttrValue::from(v.to_string())),
    })?;

    serde_json::from_str(&body).map_err(|e| match serde_json::from_str::<String>(&body) {
        Ok(v) => ApiError::Api(AttrValue::from(v)),
        Err(_) => ApiError::Serde(AttrValue::from(format!("{e} SERDE ERROR FROM {body}"))),
    })
}

fn send_error(e: gloo_net::Error) -> ApiError {
    match e {
        gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
        v => ApiError::Other(AttrValue::from(v.to_string())),
    }
}

pub async fn standard_get<T>(path: &str) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let res = Request::get(path)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .send()
        .await
        .map_err(send_error)?
        ;

    handle_response(res).await
}

pub async fn standard_delete<T>(path: &str) -> Result<T, ApiError>
where
    T: DeserializeOwned,
{
    let res = Request::delete(path)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .send()
        .await
        .map_err(send_error)?
        ;

    handle_response(res).await
}

pub async fn standard_post<T, E>(path: &str, data: &E) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    E: Serialize,
{
    let res = Request::post(path)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .json(data)
        .map_err(|e| ApiError::Serde(AttrValue::from(e.to_string())))?
        .send()
        .await
        .map_err(send_error)?
        ;

    handle_response(res).await
}

pub async fn standard_put<T, E>(path: &str, data: &E) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    E: Serialize,
{
    let res = Request::put(path)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .json(data)
        .map_err(|e| ApiError::Serde(AttrValue::from(e.to_string())))?
        .send()
        .await
        .map_err(send_error)?
        ;

    handle_response(res).await
}

pub async fn standard_patch<T, E>(path: &str, data: &E) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    E: Serialize,
{
    let res = Request::patch(path)
        .credentials(web_sys::RequestCredentials::SameOrigin)
        .json(data)
        .map_err(|e| ApiError::Serde(AttrValue::from(e.to_string())))?
        .send()
        .await
        .map_err(send_error)?
        ;

    handle_response(res).await
}
