use super::ApiError;
use gloo_net::http::Request;
use yew::AttrValue;

pub async fn get_token() -> Result<AttrValue, ApiError> {
    Request::get("/api/v1/token")
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(AttrValue::from(e)),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })?
        .json::<String>()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(AttrValue::from(e.to_string())),
            v => ApiError::Other(AttrValue::from(v.to_string())),
        })
        .map(AttrValue::from)
}
