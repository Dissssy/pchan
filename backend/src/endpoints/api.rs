use common::hash_with_salt;
use serde::{Deserialize, Serialize};
use warp::Filter;

use crate::filters::priveleged_endpoint;

pub fn priveleged_api_endpoints(
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
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

    priveleged_endpoint().and(adduser.or(removeuser).or(setusers))
}

pub fn api_endpoints() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    let addboard =
        warp::path!("api" / "board" / "add" / String).and_then(|disc: String| async move {
            match crate::database::Database::create_board(
                &mut crate::POOL.get().await.unwrap(),
                disc,
                "test".to_owned(),
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

    let getboards = addboard.or(warp::path!("api" / "board").and_then(|| async move {
        match crate::database::Database::get_boards(&mut crate::POOL.get().await.unwrap()).await {
            Ok(boards) => {
                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&boards))
            }
            Err(e) => {
                Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&e.to_string()))
            }
        }
    }));

    let getpost = getboards.or(warp::path!("api" / String / "post" / i64).and_then(
        |disc: String, thread: i64| async move {
            match crate::POOL.get().await {
                Ok(mut conn) => {
                    match crate::database::Database::get_post(&mut conn, disc, thread).await {
                        Ok(post) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&post),
                        ),
                        Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(
                            warp::reply::json(&e.to_string()),
                        ),
                    }
                }
                Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &e.to_string(),
                )),
            }
        },
    ));

    let getthread = getpost.or(warp::path!("api" / String / i64).and_then(
        |disc: String, thread: i64| async move {
            let mut conn = crate::POOL.get().await.unwrap();
            match crate::database::Database::get_thread(&mut conn, disc, thread).await {
                Ok(thread) => {
                    Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&thread))
                }
                Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &e.to_string(),
                )),
            }
        },
    ));

    getthread.or(
        warp::path!("api" / String).and_then(|disc: String| async move {
            let mut conn = crate::POOL.get().await.unwrap();
            match crate::database::Database::get_board(&mut conn, disc).await {
                Ok(board) => match board.with_threads(&mut conn).await {
                    Ok(board) => {
                        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(&board))
                    }
                    Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                        &e.to_string(),
                    )),
                },
                Err(e) => Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
                    &e.to_string(),
                )),
            }
        }),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSafe {
    pub id: String,
}
