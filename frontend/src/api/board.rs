use super::ApiError;
use common::structs::{BoardWithThreads, SafeBoard};
use gloo_net::http::Request;

pub async fn get_boards(token: &str) -> Result<Vec<SafeBoard>, ApiError> {
    let res = Request::get("/api/v1/board")
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

    serde_json::from_str(&res).map_err(|e| ApiError::Serde(format!("{e:?} from text \"{res}\"")))
}

pub async fn get_board(token: &str, board: &str) -> Result<BoardWithThreads, ApiError> {
    let res = Request::get(&format!("/api/v1/board/{}", board))
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

    serde_json::from_str(&res).map_err(|e| ApiError::Serde(format!("{e:?} from text \"{res}\"")))
}
