use anyhow::{anyhow, Result};
use common::structs::{BoardWithThreads, CreatePost, SafePost};
use web_sys::File;
use yew::UseStateHandle;

use crate::helpers::Reply;

#[derive(Default)]
pub struct Api {
    pub token: Option<String>,
}

impl Api {
    pub fn new() -> Self {
        Self::default()
    }
    pub async fn get_board_title(
        &mut self,
        discrim: String,
        title: UseStateHandle<Option<String>>,
    ) {
        let fetch = gloo_net::http::Request::get(&format!("/api/v1/board/{}", discrim))
            .send()
            .await;
        match fetch {
            Ok(f) => match f.json::<BoardWithThreads>().await {
                Ok(boardses) => {
                    title.set(Some(boardses.name));
                }
                Err(e) => {
                    gloo::console::log!(format!("{e:?}"));
                }
            },
            Err(e) => {
                gloo::console::log!(format!("{e:?}"));
            }
        }
    }
    pub async fn upload_file(
        &mut self,
        file: UseStateHandle<Option<File>>,
    ) -> Result<Option<String>> {
        let token = self.get_token().await?;
        let f = if let Some(f) = &*file {
            let form_data = web_sys::FormData::new().map_err(|e| anyhow!(format!("{:?}", e)))?;
            form_data
                .append_with_blob("file", f)
                .map_err(|e| anyhow!(format!("{:?}", e)))?;

            let res = gloo_net::http::Request::post("/api/v1/file")
                .header("authorization", &format!("Bearer {token}"))
                .body(&form_data)
                .send()
                .await?;
            let file_id = res.json::<String>().await?;
            if file_id.contains(' ') {
                return Err(anyhow::anyhow!("file upload failed"));
            }
            Some(file_id)
        } else {
            None
        };
        Ok(f)
    }
    pub async fn get_token(&mut self) -> Result<String> {
        if let Some(ref token) = self.token {
            Ok(token.clone())
        } else {
            let token = gloo_net::http::Request::get("/api/v1/")
                .send()
                .await?
                .json::<String>()
                .await?;
            self.token = Some(token.clone());
            Ok(token)
        }
    }
    pub async fn post_thread(
        &mut self,
        post: CreatePost,
        context: ThreadContext,
    ) -> Result<SafePost> {
        let token = self.get_token().await?;
        let url = format!("/api/v1/board/{}", context.board_discriminator);

        let res = gloo_net::http::Request::post(&url)
            .header("authorization", &format!("Bearer {token}"))
            .json(&post)?
            .send()
            .await?
            .text()
            .await?;

        serde_json::from_str::<SafePost>(&res).map_err(|e| {
            anyhow!(serde_json::from_str::<String>(&res)
                .unwrap_or(format!("could not parse error: {e:?}\nfrom body: {res:?}")))
        })
    }
    pub async fn post_reply(
        &mut self,
        post: CreatePost,
        context: ReplyContext,
    ) -> Result<SafePost> {
        let token = self.get_token().await?;

        // we are replying to a thread, post to /api/v1/board/{board_discriminator}/{thread_id}
        let url = format!(
            "/api/v1/board/{}/{}",
            context.board_discriminator, context.thread_id
        );

        let res = gloo_net::http::Request::post(&url)
            .header("authorization", &format!("Bearer {token}"))
            .json(&post)?
            .send()
            .await?
            .text()
            .await?;
        serde_json::from_str::<SafePost>(&res).map_err(|e| {
            anyhow!(serde_json::from_str::<String>(&res)
                .unwrap_or(format!("could not parse error: {e:?}\nfrom body: {res:?}")))
        })
    }
    pub async fn get_post(&mut self, context: &Reply) -> Result<SafePost> {
        let token = self.get_token().await?;

        // we are replying to a thread, post to /api/v1/board/{board_discriminator}/{thread_id}
        let url = format!(
            "/api/v1/board/{}/post/{}",
            context.board_discrim, context.post_number
        );
        // gloo::console::log!(format!("{url}"));
        let res = gloo_net::http::Request::get(&url)
            .header("authorization", &format!("Bearer {token}"))
            .send()
            .await?
            .text()
            .await?;
        serde_json::from_str::<SafePost>(&res).map_err(|e| {
            anyhow!(serde_json::from_str::<String>(&res)
                .unwrap_or(format!("could not parse error: {e:?}\nfrom body: {res:?}")))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReplyContext {
    pub board_discriminator: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadContext {
    pub board_discriminator: String,
}
