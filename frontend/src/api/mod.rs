use anyhow::{anyhow, Result};
use common::structs::{
    BoardWithThreads, CreateFile, CreatePost, CreateThread, SafePost, ThreadWithPosts,
};
use web_sys::File;
use yew::UseStateHandle;

use common::structs::Reply;

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

    pub async fn get_banner(
        &mut self,
        discrim: String,
        banner: UseStateHandle<Option<common::structs::Banner>>,
    ) {
        let fetch = gloo_net::http::Request::get(&format!("/api/v1/{}/banner", discrim))
            .send()
            .await;
        match fetch {
            Ok(f) => match f.json::<common::structs::Banner>().await {
                Ok(boardses) => {
                    banner.set(Some(boardses));
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

    pub async fn get_boards(
        &mut self,
        boards: UseStateHandle<Option<Vec<common::structs::SafeBoard>>>,
    ) {
        let fetch = gloo_net::http::Request::get("/api/v1/board").send().await;
        match fetch {
            Ok(f) => match f.json::<Vec<common::structs::SafeBoard>>().await {
                Ok(mut boardses) => {
                    boardses.sort_by(|a, b| a.discriminator.cmp(&b.discriminator));
                    boards.set(Some(boardses));
                }
                Err(e) => {
                    gloo::console::log!(format!("{e:?}"));
                }
            },
            Err(_) => {
                boards.set(Some(vec![]));
            }
        }
    }
    pub async fn get_board(&mut self, discrim: &str) -> Result<BoardWithThreads> {
        let fetch = gloo_net::http::Request::get(&format!("/api/v1/board/{}", discrim))
            .send()
            .await;
        match fetch {
            Ok(f) => match f.json::<BoardWithThreads>().await {
                Ok(boardses) => Ok(boardses),
                Err(e) => {
                    gloo::console::log!(format!("{e:?}"));
                    Err(e.into())
                }
            },
            Err(e) => {
                gloo::console::log!(format!("{e:?}"));
                Err(e.into())
            }
        }
    }
    pub async fn upload_file(
        &mut self,
        file: UseStateHandle<Option<File>>,
        spoiler: bool,
    ) -> Result<Option<CreateFile>> {
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
            Some(CreateFile {
                id: file_id,
                spoiler,
            })
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
        thread: CreateThread,
        context: ThreadContext,
    ) -> Result<SafePost> {
        let token = self.get_token().await?;
        let url = format!("/api/v1/board/{}", context.board_discriminator);

        let res = gloo_net::http::Request::post(&url)
            .header("authorization", &format!("Bearer {token}"))
            .json(&thread)?
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

        let url = format!(
            "/api/v1/board/{}/post/{}",
            context.board_discriminator, context.post_number
        );
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
    pub async fn get_thread(
        &mut self,
        board_discriminator: &str,
        thread_id: &str,
    ) -> Result<ThreadWithPosts> {
        let url = format!("/api/v1/board/{}/{}", board_discriminator, thread_id);
        let fetch = gloo_net::http::Request::get(&url).send().await;
        match fetch {
            Ok(f) => match f.json::<ThreadWithPosts>().await {
                Ok(thread) => Ok(thread),
                Err(e) => Err(anyhow!(format!("{e:?}"))),
            },
            Err(e) => {
                gloo::console::log!(format!("{e:?}"));
                Err(anyhow!(format!("{e:?}")))
            }
        }
    }
    pub async fn delete_post(
        &mut self,
        board_discriminator: &str,
        post_number: &str,
    ) -> Result<()> {
        // DELETE /{discriminator}/post/{post_number} - deletes a post
        let token = self.get_token().await?;

        let url = format!("/api/v1/board/{}/post/{}", board_discriminator, post_number);
        let fetch = gloo_net::http::Request::delete(&url)
            .header("authorization", &format!("Bearer {token}"))
            .send()
            .await;
        match fetch {
            Ok(f) => match f.text().await {
                Ok(v) => match v.parse::<i64>() {
                    Ok(_) => Ok(()),
                    Err(_) => match serde_json::from_str::<String>(&v) {
                        Ok(s) => Err(anyhow!(s)),
                        Err(_) => Err(anyhow!("could not parse error")),
                    },
                },
                Err(e) => Err(anyhow!(format!("{e:?}"))),
            },
            Err(e) => {
                gloo::console::log!(format!("{e:?}"));
                Err(anyhow!(format!("{e:?}")))
            }
        }
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
