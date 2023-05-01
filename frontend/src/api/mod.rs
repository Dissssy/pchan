use common::structs::SafeBoard;
use gloo_net::http::Request;
use yew::UseStateHandle;
use yew_hooks::UseLocalStorageHandle;

#[derive(Debug, Clone, PartialEq)]
pub struct Api {
    pub token: String,
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

        Ok(Self { token })
    }
    async fn get_token() -> Result<String, ApiError> {
        let token = Request::get("/api/v1/")
            .send()
            .await
            .map_err(|e| match e {
                gloo_net::Error::GlooError(e) => ApiError::Gloo(e),
                v => ApiError::Other(v.to_string()),
            })?
            .json::<String>()
            .await
            .map_err(|e| match e {
                gloo_net::Error::SerdeError(e) => ApiError::Serde(e.to_string()),
                v => ApiError::Other(v.to_string()),
            })?;
        gloo::console::log!(format!("token: {}", token));
        Ok(token)
    }

    fn formatted_token(&self) -> String {
        format!("Bearer {}", self.token)
    }

    // every api call is going to be called dispatch_<instruction> that takes a UseStateHandle<Result<T>> and returns a future.
    pub fn dispatch_get_boards(
        &self,
        output: UseStateHandle<Option<Result<Vec<SafeBoard>, Option<ApiError>>>>,
    ) {
        // GET /api/v1/board -> Vec<Board>
        let t = self.formatted_token();
        wasm_bindgen_futures::spawn_local(async move {
            let t = match match Request::get("/api/v1/board")
                .header("authorization", &t)
                .send()
                .await
                .map_err(|e| match e {
                    gloo_net::Error::GlooError(e) => ApiError::Gloo(e),
                    v => ApiError::Other(v.to_string()),
                }) {
                Ok(v) => v,
                Err(e) => {
                    output.set(Some(Err(Some(e))));
                    return;
                }
            }
            .text()
            .await
            .map_err(|e| match e {
                gloo_net::Error::SerdeError(e) => ApiError::Serde(e.to_string()),
                v => ApiError::Other(v.to_string()),
            }) {
                Ok(v) => v,
                Err(e) => {
                    output.set(Some(Err(Some(e))));
                    return;
                }
            };

            match serde_json::from_str(&t) {
                Ok(v) => output.set(Some(Ok(v))),
                Err(e) => output.set(Some(Err(Some(ApiError::Serde(format!(
                    "{e:?} from text \"{t}\""
                )))))),
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    Gloo(String),
    Serde(String),
    Other(String),
    // TODO: add error types :(
}
