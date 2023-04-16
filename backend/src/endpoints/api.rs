use crate::filters::Bearer;
use crate::schema::CreateBoard;
use crate::schema::CreatePost;
use crate::schema::CreateThread;
use common::hash_with_salt;
use serde::{Deserialize, Serialize};
use warp::Filter;

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
                let mut data = crate::DATA.lock().await;
                data.add_auth(hash_with_salt(&user.id, &crate::statics::HASH_SALT))
                    .await
                    .map_err(|e| {
                        println!("Error: {e}");
                        warp::reject::reject()
                    })?;
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
                let mut data = crate::DATA.lock().await;
                data.remove_auth(hash_with_salt(&user.id, &crate::statics::HASH_SALT))
                    .await
                    .map_err(|e| {
                        println!("Error: {e}");
                        warp::reject::reject()
                    })?;
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
                let mut data = crate::DATA.lock().await;
                data.sync_auth(
                    users
                        .iter()
                        .map(|u| hash_with_salt(&u.id, &crate::statics::HASH_SALT))
                        .collect::<Vec<String>>(),
                )
                .await
                .map_err(|e| {
                    println!("Error: {e}");
                    warp::reject::reject()
                })?;
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
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&boards))
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

    // POST /board/{discriminator} - creates a new thread

    let postthread = warp::path!("api" / "v1" / "board" / String)
        .and(warp::post())
        .and(warp::body::json::<CreateThread>())
        .and(warp::header::<Bearer>("authorization"))
        .and_then({
            |disc: String, thread: CreateThread, auth: Bearer| async move {
                match crate::database::Database::create_thread(
                    &mut crate::POOL.get().await.unwrap(),
                    disc,
                    thread,
                    auth.token,
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

    // GET /{discriminator}/{thread_id} - returns info about the thread including a list of posts

    let getthread = warp::path!("api" / "v1" / "board" / String / i64)
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

    // POST /{discriminator}/{thread_id} - creates a new post in the thread

    let postinthread = warp::path!("api" / "v1" / "board" / String / i64)
        .and(warp::post())
        .and(warp::body::json::<CreatePost>())
        .and(warp::header::<Bearer>("authorization"))
        .and_then(
            |disc: String, thread: i64, post: CreatePost, auth: Bearer| async move {
                let mut conn = crate::POOL
                    .get()
                    .await
                    .map_err(|_| warp::reject::reject())?;
                let board = crate::database::Database::get_board(&mut conn, disc.clone())
                    .await
                    .map_err(|_| warp::reject::reject())?;
                let thread = crate::database::Database::get_thread_from_post_number(
                    &mut conn, board.id, thread,
                )
                .await
                .map_err(|_| warp::reject::reject())?;
                match crate::database::Database::create_post(
                    &mut conn, board.id, disc, thread.id, post, auth.token,
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

    // let getboards = warp::path!("api" / "board").and_then(|| async move {
    //     match crate::database::Database::get_boards(&mut crate::POOL.get().await.unwrap()).await {
    //         Ok(boards) => {
    //             Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&boards))
    //         }
    //         Err(e) => {
    //             Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&e.to_string()))
    //         }
    //     }
    // });

    // let getpost = getboards.or(warp::path!("api" / String / "post" / i64).and_then(
    //     |disc: String, thread: i64| async move {
    //         match crate::POOL.get().await {
    //             Ok(mut conn) => {
    //                 match crate::database::Database::get_post(&mut conn, disc, thread).await {
    //                     Ok(post) => Ok::<warp::reply::Json, warp::reject::Rejection>(
    //                         warp::reply::json(&post),
    //                     ),
    //                     Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(
    //                         warp::reply::json(&e.to_string()),
    //                     ),
    //                 }
    //             }
    //             Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
    //                 &e.to_string(),
    //             )),
    //         }
    //     },
    // ));

    // let getthread = getpost.or(warp::path!("api" / String / i64).and_then(
    //     |disc: String, thread: i64| async move {
    //         let mut conn = crate::POOL.get().await.unwrap();
    //         match crate::database::Database::get_thread(&mut conn, disc, thread).await {
    //             Ok(thread) => {
    //                 Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&thread))
    //             }
    //             Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
    //                 &e.to_string(),
    //             )),
    //         }
    //     },
    // ));

    // getthread.or(
    //     warp::path!("api" / String).and_then(|disc: String| async move {
    //         let mut conn = crate::POOL.get().await.unwrap();
    //         match crate::database::Database::get_board(&mut conn, disc).await {
    //             Ok(board) => match board.with_threads(&mut conn).await {
    //                 Ok(board) => {
    //                     Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&board))
    //                 }
    //                 Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
    //                     &e.to_string(),
    //                 )),
    //             },
    //             Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
    //                 &e.to_string(),
    //             )),
    //         }
    //     }),
    // )

    getpost
        .or(postinthread)
        .or(getthread)
        .or(postthread)
        .or(getboard)
        .or(getboards)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSafe {
    pub id: String,
}
