use super::ApiError;
use gloo_net::http::Request;

pub async fn get_token() -> Result<String, ApiError> {
    Request::get("/api/v1/")
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
        })
}
