#![feature(async_iterator)]
use std::sync::Arc;

use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;

use reqwest::header::HeaderValue;
use tokio::sync::Mutex;
use warp::{Filter, Reply};

mod database;
mod endpoints;
mod filters;
mod push;
pub mod schema;
mod statics;
mod unclaimedfiles;
use unclaimedfiles::UnclaimedFiles;
pub mod quotes;
use quotes::Quotes;

use std::collections::HashMap;

// use crate::database::Users;
use profanity::Profanity;

use crate::filters::{user_agent_is_scraper, always_allow_thumb, valid_token};

lazy_static::lazy_static! {
    pub static ref POOL: deadpool::managed::Pool<diesel_async::pooled_connection::AsyncDieselConnectionManager<diesel_async::AsyncPgConnection>> = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))).build().expect("Database build failed");
    // pub static ref DATA: Arc<Mutex<Users>> = Arc::new(Mutex::new(Users::new().unwrap()));
    pub static ref PROFANITY: Arc<Profanity> = Arc::new(Profanity::load_csv(env!("PROFANITY_PATH")).expect("Failed to load profanity list"));
    pub static ref UNCLAIMED_FILES: Arc<Mutex<UnclaimedFiles>> = Arc::new(Mutex::new(UnclaimedFiles::new(HashMap::new())));
    pub static ref MANUAL_FILE_TRIM: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref FS_LOCK: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    pub static ref QUOTES: Arc<Quotes> = Arc::new(Quotes::load("./quotes.txt".to_string()).expect("Failed to load quotes"));
    pub static ref RATELIMIT: Arc<Mutex<HashMap<String, tokio::time::Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    pub static ref PUSH_NOTIFS: Arc<Mutex<push::PushHolder>> = Arc::new(Mutex::new(push::PushHolder::new()));
}

fn is_safe_mimetype(mimetype: &str) -> bool {
    let mimetype = mimetype.to_lowercase();
    // println!("mimetype: {}", mimetype);
    // if the Content-Type header is not video/* audio/* or image/*, then force download. also check for svg because those can contain javascript
    let can_contain = vec!["video/", "audio/", "image/"];
    let overrides = vec!["svg"];

    // if the mimetype contains any of the can_contain strings, then it's not bad UNLESS it also contains any of the overrides
    can_contain.iter().any(|s| {
        // println!("checking if {} contains {} (result: {})", mimetype, s, f);
        mimetype.contains(s)
    }) && !overrides.iter().any(|s| {
        // println!("checking if {} contains {} (result: {})", mimetype, s, f);
        mimetype.contains(s)
    })
}

#[tokio::main]
async fn main() {
    env_logger::init();
    // println!("Starting backend with:");

    // {
    //     if let Err(e) = DATA.lock().await.open().await {
    //         println!("Error opening data: {e}");
    //     }
    // }

    // let oldroot = warp::get().and(
    //     warp::fs::dir(env!("FILE_STORAGE_PATH"))
    //         .or(warp::fs::dir("/git/pchan-dev/frontend/olddist"))
    //         .or(warp::fs::file("/git/pchan-dev/frontend/olddist/index.html")),
    // );

    let root = warp::get() /*.and(filters::is_beta())*/
        .and(
            warp::fs::dir(env!("FILE_STORAGE_PATH")).and(always_allow_thumb())
                .map(|reply: warp::filters::fs::File| {
                    let mut resp = reply.into_response();
                    if let Some(content_type) = resp.headers().get(warp::http::header::CONTENT_TYPE)
                    {
                        let content_type = content_type.to_str().unwrap();
                        if !is_safe_mimetype(content_type) {
                            println!("Forcing download of file with mimetype {}", content_type);
                            resp.headers_mut().insert(
                                warp::http::header::CONTENT_DISPOSITION,
                                HeaderValue::from_static("attachment"),
                            );
                        }
                    }
                    resp
                })
                .or(valid_token().map(|_| {}).untuple_one().and(
                    warp::fs::dir(env!("DISTRIBUTION_PATH"))
                    .or(warp::fs::file(format!(
                        "{}/index.html",
                        env!("DISTRIBUTION_PATH")
                    ))
                ))),
        );

    let manifest = warp::path!("manifest.json")
        .and(warp::get())
        .and(warp::fs::file(format!(
            "{}/manifest.json",
            env!("DISTRIBUTION_PATH")
        )));

    let icon = warp::path!("res" / "icon-256.png")
        .and(warp::get())
        .and(warp::fs::file(format!(
            "{}/res/icon-256.png",
            env!("DISTRIBUTION_PATH")
        )));

    let unauthorized = warp::path!("unauthorized")
        .and(warp::get())
        .and(warp::fs::file(format!(
            "{}/unauthorized.html",
            env!("DISTRIBUTION_PATH")
        )));

    // let is_scraper = user_agent_is_scraper().and(warp::fs::file(format!(
    //     "{}/scraping.html",
    //     env!("DISTRIBUTION_PATH")
    // ))).map(|reply: warp::filters::fs::File| {
    //     let mut resp = reply.into_response();
    //     let body = resp.body().clone().to_bytes();
    //     println!("body: {:?}", body);
    //     resp
    // });

    let is_scraper = user_agent_is_scraper().and(warp::path::full()).and_then(
        // templating the scraping page by serving up a body with the right headers
        |path: warp::path::FullPath| async move {
            // extract file path from url. it is just everything after the tld
            let conn = &mut crate::POOL.get().await.map_err(|_| {
                warp::reject::reject()
            })?;
            let path = path.as_str();
            let finally = match path.split_once('/').map(|(_, p)| p) {
                Some(p) => {
                    match crate::database::Database::get_file(
                        conn,
                        format!("/{}", p),
                    ).await {
                        Ok(file) => {
                            Some(format!("https://pchan.p51.nl{}", if file.spoiler {
                                crate::database::Database::get_random_spoiler(conn).await.map_err(|_| {
                                    warp::reject::reject()
                                })?
                            } else {
                                file.thumbnail
                            }))
                        }
                        Err(_) => {
                            None
                        }
                    }
                },
                None => None,
            };



            Ok::<_, warp::reject::Rejection>(warp::http::Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(format!(
                    r#"
                    <!DOCTYPE html>

                    <html lang="en">
                        <head>
                            <title>PChan</title>
                            <meta property="og:title" content="PChan" />
                            <meta property="og:type" content="website" />
                            <meta property="og:url" content="https://pchan.p51.nl" />
                            <meta property="og:image" content={} />
                            <meta property="og:description" content="PChan, a simple 4chan-like imageboard" />
                            <meta name="theme-color" content="\#800000">
                        </head>
                    </html>
                    "#,
                    finally.unwrap_or("https://pchan.p51.nl/res/icon-256.png".to_owned())
            )))
        },
    );

    let routes = endpoints::other_endpoints()
        .or(endpoints::api::priveleged_api_endpoints())
        .or(valid_token()
            .map(|_| { /*println!("valid token");*/ })
            .untuple_one()
            .and(endpoints::api::api_endpoints())
            .or(root)
            .or(unauthorized)
            .or(manifest)
            .or(icon)
            .or(warp::any()
                .and(warp::cookie::optional::<String>("token"))
                .then(|token: Option<String>| async move {
                    match token {
                        None => Ok(warp::http::Response::builder()
                            .header("Location", "/login")
                            .status(302)
                            .body("".to_owned())
                            .unwrap()),
                        Some(_) => Ok(warp::http::Response::builder()
                            .header("Location", "/unauthorized")
                            .status(302)
                            .body("".to_owned())
                            .unwrap()),
                    }
                })));

    let (sendkill, kill) = tokio::sync::oneshot::channel::<()>();
    let (killreply, killrecv) = tokio::sync::oneshot::channel::<()>();
    let (_, server) = warp::serve(is_scraper.or(routes)).bind_with_graceful_shutdown(
        (
            [0, 0, 0, 0],
            env!("PORT").parse::<u16>().expect("PORT must be a number"),
        ),
        async {
            let _ = kill.await;
            let _ = killreply.send(());
            println!("Shutting down Warp server");
        },
    );
    tokio::spawn(server);

    let mut trim_files = tokio::time::interval(std::time::Duration::from_secs(*statics::TRIM_TIME));
    let mut delete_old_files = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut auto_delete = tokio::time::Instant::now();
    loop {
        tokio::select! {
            _ = delete_old_files.tick() => {

                if *MANUAL_FILE_TRIM.lock().await || auto_delete.elapsed() >= std::time::Duration::from_secs(*statics::DELETE_TIME) {

                    let lock = FS_LOCK.lock().await;
                    auto_delete = tokio::time::Instant::now();
                    *MANUAL_FILE_TRIM.lock().await = false;
                    let dir = env!("FILE_STORAGE_PATH");
                    let files = get_all_entries(dir).await.unwrap_or_default().iter().flat_map(|x| x.path().to_str().map(|s| s.replace(dir, ""))).collect::<Vec<String>>();
                    let mut db = match POOL.get().await {
                        Ok(x) => x,
                        Err(e) => {
                            println!("Error getting database connection: {e}");
                            continue;
                        }
                    };
                    let files_in_db = match database::Database::get_all_files(&mut db).await {
                        Ok(x) => x,
                        Err(e) => {
                            println!("Error getting files from database: {e}");
                            continue;
                        }
                    };

                    drop(lock);
                    let files_to_delete = files.iter().filter(|x| !files_in_db.iter().any(|v| &&v.path == x || &&v.thumbnail == x)).cloned().collect::<Vec<String>>().iter().map(|x| format!("{dir}{x}")).collect::<Vec<String>>();

                    for file in files_to_delete {
                        if let Err(e) = tokio::fs::remove_file(file.clone()).await {
                            println!("Error deleting file {file}: {e}");
                        }
                    }
                }
            }
            _ = trim_files.tick() => {
                if let Err(e) = UNCLAIMED_FILES.lock().await.trim_files().await {
                    println!("Error trimming files: {e}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("Received SIGINT");
                break;
            }
        }
    }
    println!("Awaiting warp shutdown");
    let _ = sendkill.send(());
    let _ = killrecv.await;
    // println!("Saving data");
    // DATA.lock().await.close().await.unwrap();
}

#[async_recursion::async_recursion]
async fn get_all_entries(dir: &str) -> anyhow::Result<Vec<tokio::fs::DirEntry>> {
    let mut return_files = Vec::new();
    let mut files = tokio::fs::read_dir(dir).await?;
    while let Some(file) = files.next_entry().await? {
        if file.file_type().await?.is_dir() {
            return_files.append(&mut get_all_entries(file.path().to_str().unwrap()).await?);
        } else {
            return_files.push(file);
        }
    }
    Ok(return_files)
}
