use super::ApiError;
use common::structs::{CreatePost, SafePost};
use gloo_net::http::Request;

pub async fn get_post(token: &str, board: &str, post: &str) -> Result<SafePost, ApiError> {
    let res = Request::get(&format!("/api/v1/board/{}/post/{}", board, post))
        .header("authorization", token)
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(e),
            v => ApiError::Other(v.to_string()),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(e.to_string()),
            v => ApiError::Other(v.to_string()),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(v),
        Err(_) => ApiError::Serde(format!("{e:?} SERDE ERROR FROM {res}")),
    })
}

pub async fn create_post(
    token: &str,
    board: &str,
    thread: &str,
    post: CreatePost,
) -> Result<SafePost, ApiError> {
    let res = Request::post(&format!("/api/v1/board/{}/thread/{}", board, thread))
        .header("authorization", token)
        .json(&post)
        .map_err(|e| ApiError::Serde(e.to_string()))?
        .send()
        .await
        .map_err(|e| match e {
            gloo_net::Error::GlooError(e) => ApiError::Gloo(e),
            v => ApiError::Other(v.to_string()),
        })?
        .text()
        .await
        .map_err(|e| match e {
            gloo_net::Error::SerdeError(e) => ApiError::Serde(e.to_string()),
            v => ApiError::Other(v.to_string()),
        })?;

    serde_json::from_str(&res).map_err(|e| match serde_json::from_str::<String>(&res) {
        Ok(v) => ApiError::Api(v),
        Err(_) => ApiError::Serde(format!("{e:?} SERDE ERROR FROM {res}")),
    })
}
