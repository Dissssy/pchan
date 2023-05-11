use crate::filters::{Bearer, Ratelimited};
use crate::unclaimedfiles::File;
use common::structs::{CreatePost, CreateThread, FileInfo, SafeBoard};
use common::{hash_with_salt, structs::CreateBoard};
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

use crate::filters::priveleged_endpoint;

pub fn priveleged_api_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    // PUT /board - creates a new board
    let putboard = warp::path!("api" / "v1" / "board")
        .and(warp::put())
        .and(warp::body::json::<CreateBoard>())
        .and_then(|board: CreateBoard| async move {
            match crate::database::Database::create_board(
                &mut crate::POOL.get().await.unwrap(),
                board.discriminator,
                board.name,
            )
            .await
            {
                Ok(_) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"OK".to_owned(),
                )),
                Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &e.to_string(),
                )),
            }
        });

    let adduser = warp::path!("api" / "add" / "user")
        .and(warp::post())
        .and(warp::body::json::<UserSafe>())
        .and_then({
            move |user: UserSafe| async move {
                let mut conn = crate::POOL
                    .get()
                    .await
                    .map_err(|_| warp::reject::reject())?;

                if let Err(e) = crate::database::Database::add_token(
                    &mut conn,
                    hash_with_salt(&user.id, &crate::statics::HASH_SALT),
                )
                .await
                {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                };
                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"OK".to_owned(),
                ))
            }
        });

    let removeuser = warp::path!("api" / "remove" / "user")
        .and(warp::post())
        .and(warp::body::json::<UserSafe>())
        .and_then({
            move |user: UserSafe| async move {
                let mut conn = crate::POOL
                    .get()
                    .await
                    .map_err(|_| warp::reject::reject())?;

                if let Err(e) = crate::database::Database::remove_token(
                    &mut conn,
                    hash_with_salt(&user.id, &crate::statics::HASH_SALT),
                )
                .await
                {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                };
                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"OK".to_owned(),
                ))
            }
        });

    let setusers = warp::path!("api" / "set" / "user")
        .and(warp::post())
        .and(warp::body::json::<Vec<UserSafe>>())
        .and_then({
            move |users: Vec<UserSafe>| async move {
                let mut conn = crate::POOL
                    .get()
                    .await
                    .map_err(|_| warp::reject::reject())?;

                let tokens = users
                    .iter()
                    .map(|u| hash_with_salt(&u.id, &crate::statics::HASH_SALT))
                    .collect::<Vec<String>>();

                if let Err(e) = crate::database::Database::sync_tokens(&mut conn, tokens).await {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                };
                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"OK".to_owned(),
                ))
            }
        });

    priveleged_endpoint().and(adduser.or(removeuser).or(setusers).or(putboard))
}

pub fn api_endpoints() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    // GET /board - returns a list of all boards

    let getboards = warp::path!("api" / "v1" / "board")
        .and(warp::get())
        .and_then({
            || async move {
                match crate::database::Database::get_boards(&mut crate::POOL.get().await.unwrap())
                    .await
                {
                    Ok(boards) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                            &boards.iter().map(|b| b.safe()).collect::<Vec<SafeBoard>>(),
                        ))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // GET /board/{discriminator} - returns info about the board including a list of threads

    let getboard = warp::path!("api" / "v1" / "board" / String)
        .and(warp::get())
        .and_then({
            |disc: String| async move {
                let mut conn = crate::POOL.get().await.unwrap();
                match crate::database::Database::get_board(&mut conn, disc).await {
                    Ok(board) => match board.with_threads(&mut conn).await {
                        Ok(board) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&board),
                        ),
                        Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&e.to_string()),
                        ),
                    },
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // POST /board/{discriminator}/thread - creates a new thread

    let postthread = warp::path!("api" / "v1" / "board" / String / "thread")
        .and(warp::post())
        .and(warp::body::json::<CreateThread>())
        .and(warp::header::<Bearer>("authorization"))
        .and_then({
            |disc: String, thread: CreateThread, auth: Bearer| async move {
                if let Err(e) = verify_post(&thread.post, Some(&thread.topic)) {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                }

                match crate::database::Database::create_thread(
                    &mut crate::POOL.get().await.unwrap(),
                    disc,
                    thread,
                    auth.token,
                )
                .await
                {
                    Ok(thread) => {
                        *crate::MANUAL_FILE_TRIM.lock().await = true;
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                            &thread.thread_post,
                        ))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // GET /{discriminator}/thread/{thread_id} - returns info about the thread including a list of posts

    let getthread = warp::path!("api" / "v1" / "board" / String / "thread" / i64)
        .and(warp::get())
        .and_then({
            |disc: String, thread: i64| async move {
                match crate::database::Database::get_thread(
                    &mut crate::POOL.get().await.unwrap(),
                    disc,
                    thread,
                )
                .await
                {
                    Ok(thread) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&thread))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // POST /{discriminator}/thread/{thread_id} - creates a new post in the thread

    let postinthread = warp::path!("api" / "v1" / "board" / String / "thread" / i64)
        .and(warp::post())
        .and(warp::body::json::<CreatePost>())
        .and(warp::header::<Bearer>("authorization"))
        .and_then(
            |disc: String, rawthread: i64, post: CreatePost, auth: Bearer| async move {
                if let Err(e) = verify_post(&post, None) {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                }

                let mut conn = match crate::POOL.get().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&e.to_string()),
                        );
                    }
                };
                let board =
                    match crate::database::Database::get_board(&mut conn, disc.clone()).await {
                        Ok(v) => v,
                        Err(e) => {
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&e.to_string()),
                            );
                        }
                    };
                let thread = match crate::database::Database::get_thread_from_post_number(
                    &mut conn, board.id, rawthread,
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&e.to_string()),
                        );
                    }
                };
                let mut files = thread
                    .posts
                    .iter()
                    .flat_map(|p| p.file.clone())
                    .collect::<Vec<FileInfo>>();
                if let Some(thread_file) = thread.thread_post.file.clone() {
                    files.push(thread_file);
                }
                if post.file.is_some() && files.len() >= 100 {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"Thread already has 100 files".to_owned(),
                    ));
                }
                match crate::database::Database::create_post(
                    &mut conn,
                    board.id,
                    disc,
                    thread.thread_post.post_number,
                    post,
                    auth.token,
                    Some(files),
                )
                .await
                {
                    Ok(post) => match post.safe(&mut conn).await {
                        Ok(post) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&post),
                        ),
                        Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&e.to_string()),
                        ),
                    },
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            },
        );

    // GET /{discriminator}/post/{post_id} - returns info about the post

    let getpost = warp::path!("api" / "v1" / "board" / String / "post" / i64)
        .and(warp::get())
        .and_then({
            |disc: String, post: i64| async move {
                match crate::database::Database::get_post(
                    &mut crate::POOL.get().await.unwrap(),
                    disc,
                    post,
                )
                .await
                {
                    Ok(post) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&post))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // DELETE /{discriminator}/post/{post_number} - deletes a post

    let deletepost = warp::path!("api" / "v1" / "board" / String / "post" / i64)
        .and(warp::delete())
        .and(warp::header::<Bearer>("authorization"))
        .and_then({
            |disc: String, post: i64, auth: Bearer| async move {
                match crate::database::Database::delete_post(
                    &mut crate::POOL.get().await.unwrap(),
                    disc,
                    post,
                    auth.token,
                )
                .await
                {
                    Ok(post) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&post))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // POST /file - uploads a file and returns the ID so you can claim it for a post

    let uploadfile = warp::path!("api" / "v1" / "file")
        .and(warp::post())
        .and(warp::multipart::form().max_length(1024 * 1024 * 100))
        .and(warp::header::<Bearer>("authorization"))
        .and_then({
            |mut form: warp::multipart::FormData, auth: Bearer| async move {
                use futures::TryStreamExt;
                {
                    if crate::UNCLAIMED_FILES.lock().await.has_pending(&auth.token) {
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"You already have a file pending"),
                        );
                    }
                }
                while let Ok(Some(p)) = form.try_next().await {
                    if p.name() == "file" {
                        use bytes::BufMut;
                        let fct;
                        let fext;
                        {
                            let content_type = p.content_type().map(|s| s.to_string());
                            match content_type {
                                Some(ct) => {
                                    fct = ct;
                                }
                                None => {
                                    return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                        warp::reply::json(&"File has no content type"),
                                    );
                                }
                            }
                            let extension = p.filename().and_then(|s| s.split('.').last());
                            match extension {
                                Some(ext) => {
                                    fext = ext.to_string();
                                }
                                None => {
                                    return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                        warp::reply::json(&"File has no extension"),
                                    );
                                }
                            }
                        }
                        let pstream = p.stream();
                        let value = match pstream
                            .try_fold(Vec::new(), |mut acc, data| {
                                acc.put(data);
                                async move { Ok(acc) }
                            })
                            .await
                        {
                            Ok(stream) => stream,
                            Err(e) => {
                                return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                    warp::reply::json(&e.to_string()),
                                );
                            }
                        };

                        return match crate::UNCLAIMED_FILES
                            .lock()
                            .await
                            .add_file(
                                File::builder()
                                    .extension(fext)
                                    .mimetype(fct)
                                    .data(value)
                                    .build()
                                    .unwrap(),
                                auth.token,
                            )
                            .await
                        {
                            Ok(id) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&id),
                            ),
                            Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&e.to_string()),
                            ),
                        };
                    }
                }

                Ok(warp::reply::json(&"No file found"))
            }
        });

    // GET /token - returns the user's token

    let gettoken = warp::path!("api" / "v1" / "token")
        .and(warp::get())
        .and(warp::cookie("token"))
        .and_then({
            |token: String| async move {
                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&token))
            }
        });

    // GET /{discriminator}/banner - returns a random banner

    let getbanner = warp::path!("api" / "v1" / "board" / String / "banner")
        .and(warp::get())
        .and_then({
            |disc: String| async move {
                match crate::database::Database::get_random_banner(
                    disc,
                    &mut crate::POOL.get().await.unwrap(),
                )
                .await
                {
                    Ok(banner) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&banner))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // POST /subscribe - sets the user's push notification url

    let subscribble = warp::path!("api" / "v1" / "subscribe")
        .and(warp::post())
        .and(warp::body::json::<SubscriptionData>())
        .and(warp::cookie::<String>("token"))
        .and_then({
            |sub: SubscriptionData, token: String| async move {
                match crate::database::Database::set_user_push_url(
                    &mut crate::POOL.get().await.unwrap(),
                    token,
                    Some(sub.to_database_string()),
                )
                .await
                {
                    Ok(_) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&"ok"))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    crate::filters::ratelimit()
        .and(
            getpost
                .or(subscribble)
                .or(getbanner)
                .or(deletepost)
                .or(postinthread)
                .or(getthread)
                .or(postthread)
                .or(getboard)
                .or(getboards)
                .or(uploadfile)
                .or(gettoken),
        )
        .recover(|err: Rejection| async move {
            if let Some(r) = err.find::<Ratelimited>() {
                Ok::<warp::reply::Response, warp::reject::Rejection>(
                    warp::reply::json(&format!(
                        "You are being ratelimited. Try again in {} seconds.",
                        r.seconds
                    ))
                    .into_response(),
                )
            } else if err.is_not_found() {
                return Err(err);
            } else {
                return Ok::<warp::reply::Response, warp::reject::Rejection>(
                    warp::reply::with_status(
                        warp::reply::json(&format!("error: {:?}", err)),
                        warp::http::StatusCode::IM_A_TEAPOT,
                    )
                    .into_response(),
                );
            }
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSafe {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionData {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

impl SubscriptionData {
    pub fn to_database_string(&self) -> String {
        format!(
            "{}|{}|{}",
            self.endpoint.to_string(),
            self.keys.p256dh.to_string(),
            self.keys.auth.to_string()
        )
    }

    // pub fn from_database_string(s: &str) -> Option<Self> {
    //     let mut split = s.split('|');
    //     let endpoint = split.next()?.to_string();
    //     let p256dh = split.next()?.to_string();
    //     let auth = split.next()?.to_string();
    //     if split.next().is_some() {
    //         return None;
    //     }
    //     Some(Self {
    //         endpoint,
    //         keys: SubscriptionKeys { p256dh, auth },
    //     })
    // }
}

fn verify_post(post: &CreatePost, topic: Option<&String>) -> anyhow::Result<()> {
    if let Some(ref a) = post.author {
        if a.len() > 30 {
            return Err(anyhow::anyhow!("Author name too long"));
        }
    }
    if post.content.len() > 5000 {
        return Err(anyhow::anyhow!("Post content too long"));
    }
    if let Some(t) = topic {
        if t.len() > 100 {
            return Err(anyhow::anyhow!("Topic name too long"));
        }
    }
    Ok(())
}
