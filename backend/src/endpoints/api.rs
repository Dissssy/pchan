use crate::filters::{viewer, Ratelimited};
use crate::unclaimedfiles::File;
use common::structs::{
    CreateBoard, CreatePost, CreateThread, FileInfo, SafeBoard, SubscriptionData,
};
use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};


/// Inert endpoints the retired Discord bot may still be calling.
///
/// They stay registered so the bot gets a definite answer rather than a 404, touch no state,
/// and do not parse their body, so Discord user IDs are not ingested at all. The warning is
/// the only way to observe whether the bot is still running; it records nothing identifying.
///
/// No longer behind the shared static token: board creation was its last real user and now
/// lives at POST /api/v1/admin/boards behind the admin role. That retires a secret compared
/// in non-constant time, and with it the Bearer header path entirely.
pub fn retired_bot_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    fn gone(route: &str) -> warp::reply::WithStatus<warp::reply::Json> {
        log::warn!("retired Discord bot endpoint called: {route}");
        warp::reply::with_status(
            warp::reply::json(
                &"This endpoint has been retired; membership is no longer managed via Discord.",
            ),
            warp::http::StatusCode::GONE,
        )
    }

    warp::post().and(
        warp::path!("api" / "add" / "user")
            .map(|| gone("POST /api/add/user"))
            .or(warp::path!("api" / "remove" / "user").map(|| gone("POST /api/remove/user")))
            .unify()
            .or(warp::path!("api" / "set" / "user").map(|| gone("POST /api/set/user")))
            .unify(),
    )
}

pub fn api_endpoints() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    // GET /board - returns a list of all boards

    let getboards = warp::path!("api" / "v1" / "board")
        .and(warp::get())
        .and(viewer())
        .and_then({
            |viewer: database::Viewer| async move {
                match crate::database_bindings::Database::get_boards(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &viewer,
                )
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
        .and(viewer())
        .and_then({
            |disc: String, viewer: database::Viewer| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        log::error!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };
                match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    &viewer,
                )
                .await
                {
                    Ok(board) => match board
                        .with_threads(&mut conn, &viewer)
                        .await
                    {
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
        .and(viewer())
        .and_then({
            |disc: String, thread: CreateThread, viewer: database::Viewer| async move {
                if let Err(e) = verify_post(&thread.post, Some(&thread.topic)) {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                }

                match crate::database_bindings::Database::create_thread(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    thread,
                    &viewer,
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
        .and(viewer())
        .and_then({
            |disc: String, thread: i64, viewer: database::Viewer| async move {
                match crate::database_bindings::Database::get_thread(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    thread,
                    &viewer,
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
        .and(viewer())
        .and_then(
            |disc: String, rawthread: i64, post: CreatePost, viewer: database::Viewer| async move {
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
                let board = match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    &viewer,
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
                let thread = match crate::database_bindings::Database::get_raw_thread(
                    &mut conn, &disc, rawthread,
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
                let tid = thread.id;
                let thread = match thread
                    .with_posts(&mut conn, &viewer)
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
                match crate::database_bindings::Database::create_post(
                    &mut conn,
                    board.id,
                    &disc,
                    tid,
                    post,
                    &viewer,
                    true,
                )
                .await
                {
                    Ok(post) => match post
                        .safe(&mut conn, &viewer)
                        .await
                    {
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
        .and(viewer())
        .and_then({
            |disc: String, post: i64, viewer: database::Viewer| async move {
                match crate::database_bindings::Database::get_post(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    &viewer,
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
        .and(viewer())
        .and_then({
            |disc: String, post: i64, viewer: database::Viewer| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        log::error!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };
                match crate::database_bindings::Database::delete_post(
                    &mut conn, &disc, post, &viewer,
                )
                .await
                {
                    Ok((deleted, was_moderation)) => {
                        // Self-deletions are not logged: an entry naming the actor and the
                        // post would link a member to their own post.
                        if was_moderation {
                            let board = crate::database_bindings::Database::get_raw_board(
                                &mut conn, &disc,
                            )
                            .await
                            .ok()
                            .map(|b| b.id);
                            crate::auth::log_action(
                                &mut conn,
                                &viewer,
                                "delete_post",
                                board,
                                Some(&post.to_string()),
                                None,
                            )
                            .await;
                        }
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                            &deleted,
                        ))
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
        .and(viewer())
        .and_then({
            |mut form: warp::multipart::FormData, viewer: database::Viewer| async move {
                use futures::TryStreamExt;
                {
                    if crate::UNCLAIMED_FILES
                        .lock()
                        .await
                        .has_pending(&viewer)
                    {
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
                        let pstream = p.stream().try_fold(Vec::new(), |mut acc, data| {
                            acc.put(data);
                            async move { Ok(acc) }
                        });
                        let value = match pstream.await {
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
                                    .expect("failed to build file response"),
                                &viewer,
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

    // PUT /share/{file_path} (will continue as something like /files/mimetype/id.type which we want to capture) - generates a path for a file to be shared that includes the FileSig query parameters

    let sharefile = warp::path!("api" / "v1" / "share" / "files" / ..)
        .and(warp::path::tail())
        .and(warp::put())
        .and(viewer())
        .and_then({
            |tail: warp::path::Tail, _: database::Viewer| async move {
                let file = format!("/files/{}", tail.as_str());
                // ensure the file exists
                log::trace!("file: {}", file);
                let fileinfo = match crate::database_bindings::Database::get_file(
                    &file,
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                )
                .await
                {
                    Ok(file) => file,
                    Err(_) => {
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"File not found"),
                        );
                    }
                };

                if fileinfo.board.private {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"Cannot share private file",
                    ));
                }

                // generate a share link
                let crate::filters::FileSig { bgn, exp, sig } =
                    crate::filters::FileSig::generate(&file).await;

                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&format!(
                    "{}?bgn={}&exp={}&sig={}",
                    file, bgn, exp, sig
                )))
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
        .and(viewer())
        .and_then({
            |disc: String, viewer: database::Viewer| async move {
                match crate::database_bindings::Database::get_random_banner(
                    &disc,
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &viewer,
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

    // GET /api/v1/board/{board_discriminator}/post/{post_number}/watching - returns true or false depending on if the user is watching the post or not

    let get_watching = warp::path!("api" / "v1" / "board" / String / "post" / i64 / "watching")
        .and(warp::get())
        .and(viewer())
        .and_then({
            |disc: String, post: i64, viewer: database::Viewer| async move {
                match crate::database_bindings::Database::get_watching(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    &viewer,
                )
                .await
                {
                    Ok(watching) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                        warp::reply::json(&watching),
                    ),
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // PUT /api/v1/board/{board_discriminator}/post/{post_number}/watching - sets the user's watching status for the post

    let put_watching = warp::path!("api" / "v1" / "board" / String / "post" / i64 / "watching")
        .and(warp::put())
        .and(viewer())
        .and(warp::body::json::<bool>())
        .and_then({
            |disc: String, post: i64, viewer: database::Viewer, watching: bool| async move {
                match crate::database_bindings::Database::set_watching(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    &viewer,
                    watching,
                )
                .await
                {
                    Ok(v) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&v))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // PUT /api/v1/board/{board_discriminator}/invite?info=string - creates an invite code for the board

    let create_invite = warp::path!("api" / "v1" / "board" / String / "invite")
        .and(warp::put())
        .and(warp::query::<InviteCodeHolder>())
        .and(viewer())
        .and_then({
            |disc: String, invite_code_holder: InviteCodeHolder, viewer: database::Viewer| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        log::error!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };

                let board = match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    &viewer,
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

                match crate::database_bindings::Database::generate_board_access_code(
                    &mut conn,
                    &viewer,
                    board.id,
                    invite_code_holder.info,
                )
                .await
                {
                    Ok(invite) => {
                        crate::auth::log_action(
                            &mut conn,
                            &viewer,
                            "issue_invite",
                            Some(board.id),
                            None,
                            None,
                        )
                        .await;
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&invite))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // PUT /api/v1/board/{board_discriminator}/moderator?info=string - creates a moderator code for the board

    let create_moderator = warp::path!("api" / "v1" / "board" / String / "moderator")
        .and(warp::put())
        .and(warp::query::<InviteCodeHolder>())
        .and(viewer())
        .and_then({
            |disc: String, invite_code_holder: InviteCodeHolder, viewer: database::Viewer| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        log::error!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };

                let board = match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    &viewer,
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

                match crate::database_bindings::Database::generate_board_moderator_code(
                    &mut conn,
                    &viewer,
                    board.id,
                    invite_code_holder.info,
                )
                .await
                {
                    Ok(invite) => {
                        crate::auth::log_action(
                            &mut conn,
                            &viewer,
                            "issue_moderator",
                            Some(board.id),
                            None,
                            None,
                        )
                        .await;
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&invite))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                }
            }
        });

    // PATCH /api/v1/consume_code?info=string - accepts an invite code

    let consume_code = warp::path!("api" / "v1" / "consume_code")
        .and(warp::patch())
        .and(warp::query::<InviteCodeHolder>())
        .and(viewer())
        .and_then({
            |invite_code_holder: InviteCodeHolder, viewer: database::Viewer| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        log::error!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };

                match crate::database_bindings::Database::consume_code(
                    &mut conn,
                    &viewer,
                    invite_code_holder.info,
                )
                .await
                {
                    Ok(_) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&"ok"))
                    }
                    Err(e) => {
                        log::trace!("error consuming code: {}", e);
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                            &"Invalid code".to_string(),
                        ))
                    }
                }
            }
        });

    // ------------------------------------------------------------ member + moderation

    // GET /me - what the frontend needs to decide which controls to show. No identity.
    let me = warp::path!("api" / "v1" / "me")
        .and(warp::get())
        .and(viewer())
        .and_then(|viewer: database::Viewer| async move {
            let moderates_any_board = if viewer.role.can_moderate_site() {
                true
            } else {
                match crate::POOL.get().await {
                    Ok(mut conn) => database::moderated_boards(&mut conn, &viewer)
                        .await
                        .map(|b| b.map(|v| !v.is_empty()).unwrap_or(true))
                        .unwrap_or(false),
                    Err(e) => {
                        log::error!("error connecting to backend: {}", e);
                        false
                    }
                }
            };
            Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                &common::structs::MeInfo {
                    role: viewer.role.as_str().to_owned(),
                    banned: viewer.ban != database::Ban::None,
                    moderates_any_board,
                },
            ))
        });

    // POST /me/rotate_author_seed - sever yourself from your own posting history.
    let rotate_seed = warp::path!("api" / "v1" / "me" / "rotate_author_seed")
        .and(warp::post())
        .and(viewer())
        .and_then(|viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            match crate::auth::rotate_author_seed(&mut conn, &viewer.id).await {
                Ok(()) => Ok(warp::reply::json(&"ok")),
                Err(e) => {
                    log::error!("could not rotate author seed: {e}");
                    Ok(warp::reply::json(&"Could not rotate identity"))
                }
            }
        });

    // GET /access_requests - the review queue. Nothing identifying is returned.
    let list_requests = warp::path!("api" / "v1" / "access_requests")
        .and(warp::get())
        .and(crate::filters::site_mod())
        .and_then(|_: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            match crate::auth::list_pending_requests(&mut conn).await {
                Ok(v) => Ok(warp::reply::json(&v)),
                Err(e) => {
                    log::error!("could not list join requests: {e}");
                    Ok(warp::reply::json(&"Could not list requests"))
                }
            }
        });

    // POST /access_requests/{id}/{approve|decline}
    let decide_request = warp::path!("api" / "v1" / "access_requests" / String / String)
        .and(warp::post())
        .and(crate::filters::site_mod())
        .and_then(|id: String, action: String, actor: database::Viewer| async move {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"Invalid request id",
                ));
            };
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok(warp::reply::json(&"error connecting to backend"));
                }
            };
            let outcome = match action.as_str() {
                "approve" => crate::auth::approve_request(&mut conn, &id).await,
                "decline" => crate::auth::decline_request(&mut conn, &id).await,
                _ => return Ok(warp::reply::json(&"Unknown action")),
            };
            match outcome {
                Ok(true) => {
                    crate::auth::log_action(
                        &mut conn,
                        &actor,
                        if action == "approve" { "approve_request" } else { "decline_request" },
                        None,
                        Some(&id.to_string()),
                        None,
                    )
                    .await;
                    Ok(warp::reply::json(&"ok"))
                }
                Ok(false) => Ok(warp::reply::json(&"No such pending request")),
                Err(e) => {
                    log::error!("could not {action} join request: {e}");
                    Ok(warp::reply::json(&"Could not record the decision"))
                }
            }
        });

    // POST /post/{id}/ban_author - the server resolves the author; the caller is never told
    // who it was.
    // Gated by viewer(), not site_mod(): a board moderator may ban from their own board, and
    // the scope check lives in ban_post_author where the actor's authority is known.
    let ban_author = warp::path!("api" / "v1" / "board" / String / "post" / i64 / "ban_author")
        .and(warp::post())
        .and(warp::body::json::<common::structs::BanRequest>())
        .and(viewer())
        .and_then(|disc: String, post_number: i64, req: common::structs::BanRequest, viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            match crate::auth::ban_post_author(
                &mut conn,
                &disc,
                post_number,
                &viewer,
                req.sitewide,
                req.reason.as_deref(),
            )
            .await
            {
                Ok(true) => Ok(warp::reply::json(&"Author banned")),
                Ok(false) => Ok(warp::reply::json(&"No current author for that post")),
                Err(e) => {
                    // The message distinguishes "not allowed" from "went wrong" without
                    // naming anyone.
                    log::warn!("ban refused or failed: {e}");
                    Ok(warp::reply::json(&format!("{e}")))
                }
            }
        });

    // DELETE /admin/bans/{id} - lift a ban. Scope authority is checked server-side.
    let unban = warp::path!("api" / "v1" / "admin" / "bans" / String)
        .and(warp::delete())
        .and(viewer())
        .and_then(|id: String, viewer: database::Viewer| async move {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"Invalid ban id",
                ));
            };
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok(warp::reply::json(&"error connecting to backend"));
                }
            };
            match crate::auth::unban(&mut conn, &id, &viewer).await {
                Ok(true) => Ok(warp::reply::json(&"Ban lifted")),
                Ok(false) => Ok(warp::reply::json(&"No such ban")),
                Err(e) => {
                    log::warn!("unban refused or failed: {e}");
                    Ok(warp::reply::json(&format!("{e}")))
                }
            }
        });

    // POST /admin/bans/{id}/escalate - widen a board ban to the whole site. Site-level only.
    let escalate_ban = warp::path!("api" / "v1" / "admin" / "bans" / String / "escalate")
        .and(warp::post())
        .and(warp::body::json::<common::structs::BanRequest>())
        .and(viewer())
        .and_then(|id: String, req: common::structs::BanRequest, viewer: database::Viewer| async move {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"Invalid ban id",
                ));
            };
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok(warp::reply::json(&"error connecting to backend"));
                }
            };
            match crate::auth::escalate_ban(&mut conn, &id, &viewer, req.reason.as_deref()).await {
                Ok(true) => Ok(warp::reply::json(&"Ban widened to the whole site")),
                Ok(false) => Ok(warp::reply::json(&"No such ban")),
                Err(e) => {
                    log::warn!("ban escalation refused or failed: {e}");
                    Ok(warp::reply::json(&format!("{e}")))
                }
            }
        });

    // POST /board/{disc}/post/{n}/report - any member may flag a post. No reporter recorded.
    let report_post = warp::path!("api" / "v1" / "board" / String / "post" / i64 / "report")
        .and(warp::post())
        .and(warp::body::json::<common::structs::ReportRequest>())
        .and(viewer())
        .and_then(|disc: String, post_number: i64, req: common::structs::ReportRequest, viewer: database::Viewer| async move {
            let reason = req.reason.trim().to_owned();
            if reason.is_empty() {
                return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"Say what is wrong with it.",
                ));
            }
            if reason.len() > 1000 {
                return Ok(warp::reply::json(&"That reason is too long."));
            }

            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok(warp::reply::json(&"error connecting to backend"));
                }
            };

            // Through get_board, so a post on a board the reporter cannot see cannot be
            // reported — and the existence of such a board is not revealed.
            let board = match crate::database_bindings::Database::get_board(&mut conn, &disc, &viewer).await {
                Ok(b) => b,
                Err(e) => return Ok(warp::reply::json(&e.to_string())),
            };

            let post = {
                use database::posts::dsl as p;
                use diesel::prelude::*;
                use diesel_async::RunQueryDsl;
                p::posts
                    .filter(p::board.eq(board.id))
                    .filter(p::post_number.eq(post_number))
                    .select(p::id)
                    .first::<i64>(&mut conn)
                    .await
                    .ok()
            };

            let Some(post) = post else {
                return Ok(warp::reply::json(&"No such post"));
            };

            match database::create_report(&mut conn, post, board.id, &reason).await {
                Ok(()) => Ok(warp::reply::json(&"Reported. A moderator will look.")),
                Err(e) => {
                    log::error!("could not record a report: {e}");
                    Ok(warp::reply::json(&"Could not file that report"))
                }
            }
        });

    // GET /admin/reports - the inbox, scoped to boards the viewer moderates.
    let list_reports = warp::path!("api" / "v1" / "admin" / "reports")
        .and(warp::get())
        .and(viewer())
        .and_then(|viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };

            let scope = match database::moderated_boards(&mut conn, &viewer).await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("could not resolve moderated boards: {e}");
                    return Ok(warp::reply::json(&"Could not load reports"));
                }
            };

            // Not a moderator anywhere: an empty inbox rather than an error, so the panel can
            // render the same component tree for everyone.
            if matches!(&scope, Some(b) if b.is_empty()) {
                return Ok(warp::reply::json(&Vec::<common::structs::ReportSummary>::new()));
            }

            match database::open_reports(&mut conn, scope.as_deref()).await {
                Ok(rows) => Ok(warp::reply::json(
                    &rows
                        .into_iter()
                        .map(|(r, disc, number)| common::structs::ReportSummary {
                            id: r.id.to_string(),
                            board_discriminator: disc,
                            post_number: number,
                            reason: r.reason,
                            created_at: r.created_at,
                        })
                        .collect::<Vec<_>>(),
                )),
                Err(e) => {
                    log::error!("could not list reports: {e}");
                    Ok(warp::reply::json(&"Could not load reports"))
                }
            }
        });

    // POST /admin/reports/{id}/resolve - mark handled.
    let resolve_report = warp::path!("api" / "v1" / "admin" / "reports" / String / "resolve")
        .and(warp::post())
        .and(viewer())
        .and_then(|id: String, viewer: database::Viewer| async move {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"Invalid report id",
                ));
            };
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok(warp::reply::json(&"error connecting to backend"));
                }
            };

            // Resolve first to learn the board, then check authority over that board.
            let board = {
                use database::reports::dsl as r;
                use diesel::prelude::*;
                use diesel_async::RunQueryDsl;
                r::reports
                    .filter(r::id.eq(id))
                    .select(r::board_id)
                    .first::<i64>(&mut conn)
                    .await
                    .ok()
            };
            let Some(board) = board else {
                return Ok(warp::reply::json(&"No such report"));
            };

            let permitted = viewer.role.can_moderate_site()
                || database::permission_level(&mut conn, board, &viewer)
                    .await
                    .map(|p| p.is_moderator())
                    .unwrap_or(false);
            if !permitted {
                return Ok(warp::reply::json(&"Not a moderator of that board"));
            }

            match database::resolve_report(&mut conn, &id, &viewer.id).await {
                Ok(Some(b)) => {
                    crate::auth::log_action(&mut conn, &viewer, "resolve_report", Some(b), None, None)
                        .await;
                    Ok(warp::reply::json(&"Resolved"))
                }
                Ok(None) => Ok(warp::reply::json(&"Already resolved")),
                Err(e) => {
                    log::error!("could not resolve report: {e}");
                    Ok(warp::reply::json(&"Could not resolve that report"))
                }
            }
        });

    // ------------------------------------------------------------ admin panel

    // Boards. Creation moves here from the static-token endpoint so it is gated by a real
    // role rather than one shared secret compared in non-constant time.
    let create_board = warp::path!("api" / "v1" / "admin" / "boards")
        .and(warp::post())
        .and(warp::body::json::<CreateBoard>())
        .and(crate::filters::site_admin())
        .and_then(|board: CreateBoard, viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            match crate::database_bindings::Database::create_board(
                &mut conn,
                &board.discriminator,
                &board.name,
            )
            .await
            {
                Ok(()) => {
                    crate::auth::log_action(
                        &mut conn,
                        &viewer,
                        "create_board",
                        None,
                        Some(&board.discriminator),
                        None,
                    )
                    .await;
                    Ok(warp::reply::json(&"Board created"))
                }
                Err(e) => Ok(warp::reply::json(&e.to_string())),
            }
        });

    let edit_board = warp::path!("api" / "v1" / "admin" / "boards" / String)
        .and(warp::patch())
        .and(warp::body::json::<common::structs::BoardUpdate>())
        .and(crate::filters::site_admin())
        .and_then(|disc: String, update: common::structs::BoardUpdate, viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            let board = match crate::database_bindings::Database::get_raw_board(&mut conn, &disc).await {
                Ok(b) => b,
                Err(_) => return Ok(warp::reply::json(&"No such board")),
            };
            match database::update_board(
                &mut conn,
                board.id,
                update.name.as_deref(),
                update.private,
            )
            .await
            {
                Ok(()) => {
                    crate::auth::log_action(
                        &mut conn, &viewer, "edit_board", Some(board.id), Some(&disc), None,
                    )
                    .await;
                    Ok(warp::reply::json(&"Board updated"))
                }
                Err(e) => Ok(warp::reply::json(&e.to_string())),
            }
        });

    let delete_board = warp::path!("api" / "v1" / "admin" / "boards" / String)
        .and(warp::delete())
        .and(crate::filters::site_admin())
        .and_then(|disc: String, viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            let board = match crate::database_bindings::Database::get_raw_board(&mut conn, &disc).await {
                Ok(b) => b,
                Err(_) => return Ok(warp::reply::json(&"No such board")),
            };
            match database::delete_board(&mut conn, board.id).await {
                Ok(()) => {
                    crate::auth::log_action(
                        &mut conn, &viewer, "delete_board", None, Some(&disc), None,
                    )
                    .await;
                    Ok(warp::reply::json(&"Board deleted"))
                }
                Err(e) => Ok(warp::reply::json(&e.to_string())),
            }
        });

    // Moderator grants. Grants, not people: nothing here resolves to a member.
    let list_moderators = warp::path!("api" / "v1" / "admin" / "moderators")
        .and(warp::get())
        .and(crate::filters::site_admin())
        .and_then(|_: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            match database::moderator_grants(&mut conn).await {
                Ok(rows) => Ok(warp::reply::json(
                    &rows
                        .into_iter()
                        .map(|(id, disc, label, redeemed)| common::structs::ModeratorGrant {
                            id: id.to_string(),
                            board_discriminator: disc,
                            label,
                            redeemed,
                        })
                        .collect::<Vec<_>>(),
                )),
                Err(e) => {
                    log::error!("could not list moderator grants: {e}");
                    Ok(warp::reply::json(&"Could not load moderators"))
                }
            }
        });

    let revoke_moderator = warp::path!("api" / "v1" / "admin" / "moderators" / String)
        .and(warp::delete())
        .and(crate::filters::site_admin())
        .and_then(|id: String, viewer: database::Viewer| async move {
            let Ok(id) = id.parse::<uuid::Uuid>() else {
                return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &"Invalid grant id",
                ));
            };
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok(warp::reply::json(&"error connecting to backend"));
                }
            };
            match database::revoke_grant(&mut conn, &id).await {
                Ok(Some(board)) => {
                    crate::auth::log_action(
                        &mut conn, &viewer, "revoke_moderator", Some(board), None, None,
                    )
                    .await;
                    Ok(warp::reply::json(&"Moderator revoked"))
                }
                Ok(None) => Ok(warp::reply::json(&"No such grant")),
                Err(e) => {
                    log::error!("could not revoke grant: {e}");
                    Ok(warp::reply::json(&"Could not revoke that grant"))
                }
            }
        });

    // The audit log. Admins see everything; a moderator sees only their own actions, because
    // the full log correlates bans to posts and is the one place that exception lives.
    let audit_log = warp::path!("api" / "v1" / "admin" / "log")
        .and(warp::get())
        .and(viewer())
        .and_then(|viewer: database::Viewer| async move {
            let mut conn = match crate::POOL.get().await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("error connecting to backend: {}", e);
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &"error connecting to backend",
                    ));
                }
            };
            let scope = if viewer.role.is_admin() { None } else { Some(&viewer.id) };
            match database::moderation_entries(&mut conn, scope, 200).await {
                Ok(rows) => Ok(warp::reply::json(
                    &rows
                        .into_iter()
                        .map(|(action, board, target, detail, at)| common::structs::LogEntry {
                            action,
                            board_discriminator: board,
                            target,
                            detail,
                            created_at: at,
                        })
                        .collect::<Vec<_>>(),
                )),
                Err(e) => {
                    log::error!("could not read the moderation log: {e}");
                    Ok(warp::reply::json(&"Could not load the log"))
                }
            }
        });

    crate::filters::ratelimit()
        .and(
            getpost
                .or(getbanner)
                .or(deletepost)
                .or(postinthread)
                .or(getthread)
                .or(postthread)
                .or(getboard)
                .or(getboards)
                .or(uploadfile)
                .or(sharefile)
                .or(gettoken)
                .or(get_watching)
                .or(put_watching)
                .or(create_invite)
                .or(create_moderator)
                .or(consume_code)
                .or(me)
                .or(rotate_seed)
                .or(list_requests)
                .or(decide_request)
                .or(ban_author)
                .or(unban)
                .or(escalate_ban)
                .or(report_post)
                .or(list_reports)
                .or(resolve_report)
                .or(create_board)
                .or(edit_board)
                .or(delete_board)
                .or(list_moderators)
                .or(revoke_moderator)
                .or(audit_log)
                .or(notifications()),
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
pub struct InviteCodeHolder {
    pub info: String,
}

pub fn notifications() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    // GET /notifications - SSE endpoint to listen for generic push notifications

    let pushnotifs = warp::path!("api" / "v1" / "notifications")
        .and(viewer())
        .and_then(|viewer: database::Viewer| async move {
            Ok::<_, warp::reject::Rejection>(warp::sse::reply(warp::sse::keep_alive().stream({
                // Per-member channel, keyed on the member id to match the dispatcher.
                crate::PUSH_NOTIFS
                    .lock()
                    .await
                    .subscribe(&viewer.id.to_string())
                    .await
            })))
        });

    // GET /push/thread/{number} - temporary SSE endpoint to listen for specific thread updates

    let threadnotifs = warp::path!(
        "api" / "v1" / "board" / String / "thread" / i64 / "notifications"
    )
    .and(viewer())
    .and_then(|board: String, thread: i64, viewer: database::Viewer| async move {
        let mut conn = crate::POOL
            .get()
            .await
            .map_err(|_| warp::reject::reject())?;

        // get_board performs the private-board access check. Without it this stream is
        // readable by any member, on any board they were never invited to. Rejecting rather
        // than returning a status also avoids confirming that a private board exists.
        crate::database_bindings::Database::get_board(&mut conn, &board, &viewer)
            .await
            .map_err(|_| warp::reject::reject())?;

        Ok::<_, warp::reject::Rejection>(warp::sse::reply(warp::sse::keep_alive().stream({
            crate::PUSH_NOTIFS
                .lock()
                .await
                .subscribe(&format!("board: {} | thread: {}", board, thread))
                .await
        })))
    });

    // POST /subscribe - sets the user's push notification url

    let subscribble = warp::path!("api" / "v1" / "subscribe")
        .and(warp::post())
        .and(warp::body::json::<SubscriptionData>())
        .and(viewer())
        .and_then({
            |sub: SubscriptionData, viewer: database::Viewer| async move {
                match crate::database_bindings::Database::add_user_push_url(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &viewer,
                    sub,
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

    pushnotifs
        .or(threadnotifs)
        .map(|reply| {
            warp::reply::with_header(
                warp::reply::with_header(
                    warp::reply::with_header(reply, warp::http::header::CACHE_CONTROL, "no-cache"),
                    warp::http::header::CONTENT_TYPE,
                    "text/event-stream",
                ),
                "X-Accel-Buffering",
                "no",
            )
        })
        .or(subscribble)
}

// {
//     "endpoint":"endpoint_url",
//     "expirationTime":null,
//     "keys":{
//         "auth":"auth thing",
//         "p256dh":"p256dh thing"
//     }
// }

// impl SubscriptionData {
//     pub fn to_database_string(&self) -> String {
//         format!(
//             "{}|{}|{}",
//             self.endpoint.to_string(),
//             self.keys.p256dh.to_string(),
//             self.keys.auth.to_string()
//         )
//     }

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
// }

fn verify_post(post: &CreatePost, topic: Option<&String>) -> anyhow::Result<()> {
    if let Some(ref a) = post.author {
        if a.len() > 30 {
            return Err(anyhow::anyhow!("Author too long"));
        }
    }
    if post.content.len() > 5000 {
        return Err(anyhow::anyhow!("Post content too long"));
    }
    if let Some(t) = topic {
        if t.len() > 100 {
            return Err(anyhow::anyhow!("Topic too long"));
        }
    }
    Ok(())
}
