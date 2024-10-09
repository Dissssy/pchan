use crate::filters::{valid_token, MemberToken, Ratelimited, Token};
use crate::unclaimedfiles::File;
use common::structs::{
    CreateBoard, CreatePost, CreateThread, FileInfo, SafeBoard, SubscriptionData,
};
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
            match crate::database_bindings::Database::create_board(
                &mut match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        println!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                },
                &board.discriminator,
                &board.name,
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

                if let Err(e) = crate::database_bindings::Database::add_token(
                    &mut conn,
                    Token::from_id(&user.id).member_hash(),
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

                if let Err(e) = crate::database_bindings::Database::remove_token(
                    &mut conn,
                    Token::from_id(&user.id).member_hash(),
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
                    .map(|u| Token::from_id(&u.id).member_hash())
                    .collect::<Vec<MemberToken>>();

                if let Err(e) =
                    crate::database_bindings::Database::sync_tokens(&mut conn, tokens).await
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

    priveleged_endpoint().and(adduser.or(removeuser).or(setusers).or(putboard))
}

pub fn api_endpoints() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    // GET /board - returns a list of all boards

    let getboards = warp::path!("api" / "v1" / "board")
        .and(warp::get())
        .and(valid_token())
        .and_then({
            |mut token: Token| async move {
                match crate::database_bindings::Database::get_boards(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |disc: String, mut token: Token| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        println!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };
                match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    token.member_hash(),
                )
                .await
                {
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
        .and(valid_token())
        .and_then({
            |disc: String, thread: CreateThread, mut token: Token| async move {
                if let Err(e) = verify_post(&thread.post, Some(&thread.topic)) {
                    return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    ));
                }

                match crate::database_bindings::Database::create_thread(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    thread,
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |disc: String, thread: i64, mut token: Token| async move {
                match crate::database_bindings::Database::get_thread(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    thread,
                    token.member_hash(),
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
        .and(valid_token())
        .and_then(
            |disc: String, rawthread: i64, post: CreatePost, mut token: Token| async move {
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
                    token.member_hash(),
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
                let thread = match thread.with_posts(&mut conn).await {
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
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |disc: String, post: i64, mut token: Token| async move {
                match crate::database_bindings::Database::get_post(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |disc: String, post: i64, mut token: Token| async move {
                match crate::database_bindings::Database::delete_post(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |mut form: warp::multipart::FormData, mut token: Token| async move {
                use futures::TryStreamExt;
                {
                    if crate::UNCLAIMED_FILES
                        .lock()
                        .await
                        .has_pending(token.member_hash())
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
                                token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |tail: warp::path::Tail, _: Token| async move {
                let file = format!("/files/{}", tail.as_str());
                // ensure the file exists
                // println!("file: {}", file);
                let fileinfo = match crate::database_bindings::Database::get_file(
                    &file,
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
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

                match fileinfo.board.map(|b| b.private) {
                    Some(true) => {
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                            &"Cannot share private file",
                        ));
                    }
                    None => {
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                            &"Unable to determine if file is private, disallowing sharing for safety",
                        ));
                    }
                    _ => {}
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
        .and(valid_token())
        .and_then({
            |disc: String, mut token: Token| async move {
                match crate::database_bindings::Database::get_random_banner(
                    &disc,
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |disc: String, post: i64, mut token: Token| async move {
                match crate::database_bindings::Database::get_watching(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    token.member_hash(),
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
        .and(valid_token())
        .and(warp::body::json::<bool>())
        .and_then({
            |disc: String, post: i64, mut token: Token, watching: bool| async move {
                match crate::database_bindings::Database::set_watching(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    &disc,
                    post,
                    token.member_hash(),
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
        .and(valid_token())
        .and_then({
            |disc: String, invite_code_holder: InviteCodeHolder, mut token: Token| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        println!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };

                let board = match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    token.member_hash(),
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
                    token.member_hash(),
                    board.id,
                    invite_code_holder.info,
                )
                .await
                {
                    Ok(invite) => {
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
        .and(valid_token())
        .and_then({
            |disc: String, invite_code_holder: InviteCodeHolder, mut token: Token| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        println!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };

                let board = match crate::database_bindings::Database::get_board(
                    &mut conn,
                    &disc,
                    token.member_hash(),
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
                    token.member_hash(),
                    board.id,
                    invite_code_holder.info,
                )
                .await
                {
                    Ok(invite) => {
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
        .and(valid_token())
        .and_then({
            |invite_code_holder: InviteCodeHolder, mut token: Token| async move {
                let mut conn = match crate::POOL.get().await {
                    Ok(pool) => pool,
                    Err(e) => {
                        println!("error connecting to backend: {}", e);
                        return Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&"error connecting to backend"),
                        );
                    }
                };

                match crate::database_bindings::Database::consume_code(
                    &mut conn,
                    token.member_hash(),
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
        .and(valid_token())
        .and_then(|mut token: Token| async move {
            Ok::<_, warp::reject::Rejection>(warp::sse::reply(warp::sse::keep_alive().stream({
                crate::PUSH_NOTIFS
                    .lock()
                    .await
                    .subscribe(&token.member_hash().member_hash())
                    .await
            })))
        });

    // GET /push/thread/{number} - temporary SSE endpoint to listen for specific thread updates

    let threadnotifs = warp::path!(
        "api" / "v1" / "board" / String / "thread" / i64 / "notifications"
    )
    .and_then(|board: String, thread: i64| async move {
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
        .and(valid_token())
        .and_then({
            |sub: SubscriptionData, mut token: Token| async move {
                match crate::database_bindings::Database::add_user_push_url(
                    &mut match crate::POOL.get().await {
                        Ok(pool) => pool,
                        Err(e) => {
                            println!("error connecting to backend: {}", e);
                            return Ok::<warp::reply::Json, warp::reject::Rejection>(
                                warp::reply::json(&"error connecting to backend"),
                            );
                        }
                    },
                    token.member_hash(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSafe {
    pub id: String,
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
