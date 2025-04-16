#![feature(async_iterator)]
#![warn(
    clippy::map_unwrap_or,
    clippy::unwrap_used,
    clippy::clone_on_ref_ptr,
    clippy::dbg_macro,
    clippy::todo,
    clippy::unimplemented,
    clippy::panic,
    clippy::unwrap_in_result,
    clippy::use_self,
    clippy::unnecessary_to_owned,
    clippy::ptr_arg,
    clippy::if_then_some_else_none,
    clippy::implicit_clone,
    clippy::manual_string_new
)]
#![allow(clippy::needless_return)]
use std::{
    io::{Read as _, Write as _},
    sync::Arc,
};

use base64::Engine;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use filters::valid_token_always_allow_res;
use std::os::linux::fs::MetadataExt;

use tokio::sync::Mutex;
use warp::{http::HeaderValue, Filter, Reply};

mod database_bindings;
mod endpoints;
mod filters;
mod push;
// pub mod schema;
mod statics;
mod unclaimedfiles;
use unclaimedfiles::UnclaimedFiles;
pub mod quotes;
use quotes::Quotes;

use std::collections::HashMap;

// use crate::database_bindings::Users;
use profanity::Profanity;

use crate::filters::{
    optional_file_sig, optional_token, user_agent_is_scraper, valid_token, FileSig, Token,
};

lazy_static::lazy_static! {
    pub static ref POOL: deadpool::managed::Pool<diesel_async::pooled_connection::AsyncDieselConnectionManager<diesel_async::AsyncPgConnection>> = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))).build().expect("Database build failed");
    // pub static ref DATA: Arc<Mutex<Users>> = Arc::new(Mutex::new(Users::new().unwrap()));
    pub static ref PROFANITY: Arc<Profanity> = Arc::new(Profanity::load_csv(env!("PROFANITY_PATH")).expect("Failed to load profanity list"));
    pub static ref UNCLAIMED_FILES: Arc<Mutex<UnclaimedFiles>> = Arc::new(Mutex::new(UnclaimedFiles::new(HashMap::new())));
    pub static ref MANUAL_FILE_TRIM: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref FS_LOCK: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    pub static ref QUOTES: Arc<Quotes> = Arc::new(Quotes::load(env!("QUOTES_PATH")).expect("Failed to load quotes"));
    pub static ref RATELIMIT: Arc<Mutex<HashMap<String, tokio::time::Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    pub static ref PUSH_NOTIFS: Arc<Mutex<push::PushHolder>> = Arc::new(Mutex::new(push::PushHolder::new()));
}

fn is_safe_mimetype(mimetype: &str) -> bool {
    let mimetype = mimetype.to_lowercase();
    log::trace!("mimetype: {}", mimetype);
    // if the Content-Type header is not video/* audio/* or image/*, then force download. also check for svg because those can contain javascript
    let can_contain = ["video/", "audio/", "image/", "NO MIMETYPE"];
    let overrides = ["svg"];

    // if the mimetype contains any of the can_contain strings, then it's not bad UNLESS it also contains any of the overrides
    can_contain.iter().any(|s| {
        let f = mimetype.contains(s);
        log::trace!("checking if {} contains {} (result: {})", mimetype, s, f);
        f
    }) && !overrides.iter().any(|s| {
        let f = mimetype.contains(s);
        log::trace!("checking if {} contains {} (result: {})", mimetype, s, f);
        f
    })
}

#[tokio::main]
async fn main() {
    // env_logger::init();
    // match api_key {
    //     Ok(api_key) => {
    //         if let Ok(api) = std::env::var("SEQ_API_URL") {
    //             if let Err(e) = datalust_logger::init_remote(
    //                 "PChan",
    //                 &api,
    //                 &api_key,
    //                 log_level.parse::<log::Level>().unwrap_or(log::Level::Info),
    //             ).await {
    //                 env_logger::init();
    //                 log::error!("Failed to initialize datalust logger: {e}");
    //             }
    //         } else if let Err(e) = datalust_logger::init_local(
    //             "PChan",
    //             &api_key,
    //             log_level.parse::<log::Level>().unwrap_or(log::Level::Info),
    //         ).await {
    //             env_logger::init();
    //             log::error!("Failed to initialize datalust logger: {e}");
    //         }
    //     }
    //     _ => {
    //         env_logger::init();
    //         log::warn!("SEQ_API_KEY not set, defaulting to env_logger");
    //     }
    // }
    if let Err(e) = datalust_logger::init("PChan") {
        env_logger::init();
        log::error!("Failed to initialize datalust logger: {e}");
    }
    log::info!("Starting PChan");
    // if let Some(lvl) = match ("SEQ_LOG_LEVEL").to_lowercase().as_str() {
    //     "debug" => Some(log::Level::Debug),
    //     "trace" => Some(log::Level::Trace),
    //     "info" => Some(log::Level::Info),
    //     "warn" => Some(log::Level::Warn),
    //     "error" => Some(log::Level::Error),
    //     _ => None,
    // } {
    //     if let Err(e) = datalust_logger::init_local(
    //         "PChan",
    //         env!("SEQ_API_KEY"),
    //         lvl,
    //     ).await {
    //         env_logger::init();
    //         log::error!("Failed to initialize datalust logger: {e}");
    //     }
    // } else {
    //     env_logger::init();
    //     log::warn!("Invalid log level set");
    // }
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
            warp::fs::dir(env!("FILE_STORAGE_PATH"))
                .and(warp::path::full())
                .and(optional_token())
                .and(optional_file_sig())
                .and_then(
                    |reply: warp::filters::fs::File,
                     path: warp::path::FullPath,
                     mut token: Option<Token>,
                     file_sig: Option<FileSig>| async move {
                        let path = path.as_str().to_string();

                        let mut resp = reply.into_response();
                        if let Some(content_type) =
                            resp.headers().get(warp::http::header::CONTENT_TYPE)
                        {
                            let content_type = content_type.to_str().unwrap_or("NO MIMETYPE");
                            if !is_safe_mimetype(content_type) {
                                log::trace!("Forcing download of file with mimetype {}", content_type);
                                resp.headers_mut().insert(
                                    warp::http::header::CONTENT_DISPOSITION,
                                    HeaderValue::from_static("attachment"),
                                );
                            }
                        }

                        if path.ends_with("-thumb.jpg") || token.is_some() {
                            return Ok(resp);
                        }

                        if path.contains("/files/") {
                            let mut conn = POOL.get().await.map_err(|_| warp::reject::reject())?;

                            let _file = database::get_file_from_path(
                                &mut conn,
                                token
                                    .as_mut()
                                    .map(|x| x.member_hash().database_hash().to_string()),
                                path.trim_end_matches("-thumb.jpg"),
                            )
                            .await
                            .map_err(|e| {
                                log::error!("Error getting file: {e}");
                                warp::reject::reject()
                            })?;

                            if let Some(file_sig) = file_sig {
                                if file_sig.validates(&path).await {
                                    return Ok(resp);
                                }
                            }
                        }

                        Err(warp::reject::reject())
                    },
                )
                .or(valid_token_always_allow_res()
                    .map(|_| {})
                    .untuple_one()
                    .and(
                        warp::fs::dir(env!("DISTRIBUTION_PATH")).or(warp::fs::file(format!(
                            "{}/index.html",
                            env!("DISTRIBUTION_PATH")
                        ))),
                    )),
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

    let is_scraper = user_agent_is_scraper().and(warp::path::full()).and(crate::filters::optional_file_sig()).and(
        crate::filters::optional_file()
    ).and_then(
        // templating the scraping page by serving up a body with the right headers
        |path: warp::path::FullPath, sig: Option<FileSig>, raw_file: Option<warp::fs::File>| async move {
            // extract file path from url. it is just everything after the tld
            let conn = &mut crate::POOL.get().await.map_err(|_| {
                warp::reject::reject()
            })?;
            let path = path.as_str();

            if path.contains("/res/") {
                if let Some(raw_file) = raw_file {
                    return Ok(raw_file.into_response());
                }
            }

            let finally = match path.split_once('/').map(|(_, p)| format!("/{}", p)) {
                Some(p) => {
                    match crate::database_bindings::Database::get_file(
                        &p,
                        conn,
                    ).await.map(|f| (f.board.private, f)) {
                        Ok((false, file)) => {
                            Some(format!("https://pchan.p51.nl{}", if let Some(sig) = sig {
                                log::trace!("found sig");
                                if sig.validates(&p).await {
                                    log::trace!("{:?}", raw_file);
                                    if let Some(raw_file) = raw_file {
                                        log::trace!("valid file");
                                        // determine file size
                                        match std::fs::metadata(raw_file.path()) {
                                            Ok(meta) => {
                                                if meta.st_size() > 10 * 1024 * 1024 || !p.contains("/image/") {
                                                    file.claimed.thumbnail
                                                } else {
                                                    return Ok(raw_file.into_response());
                                                }
                                            }
                                            Err(e) => {
                                                log::error!("Error getting file metadata: {e}");
                                                file.claimed.thumbnail
                                            }
                                        }
                                    } else {
                                        log::trace!("no raw file? for path {}", p);
                                        file.claimed.thumbnail
                                    }
                                } else {
                                    log::trace!("no valid sig");
                                    file.claimed.thumbnail
                                }
                            } else {
                                log::trace!("no sig");
                                file.claimed.thumbnail
                            }))
                        }
                        _ => {
                            log::trace!("no file found");
                            None
                        }
                    }
                },
                _ => None,
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
            )).into_response())
        },
    );

    let routes = endpoints::other_endpoints()
        .or(endpoints::api::priveleged_api_endpoints())
        .or(valid_token()
            .map(|_| { log::trace!("valid token"); })
            .untuple_one()
            .and(endpoints::api::api_endpoints())
            .or(root)
            .or(unauthorized)
            .or(manifest)
            .or(icon)
            .or(warp::any()
                .and(warp::cookie::optional::<String>("token"))
                .and(warp::path::full())
                .then(
                    |token: Option<String>, path: warp::filters::path::FullPath| async move {
                        match token {
                            None => warp::http::Response::builder()
                                .header(
                                    "Location",
                                    format!(
                                        "/login?redirect={}",
                                        match encode_checksum_str(path.as_str()) {
                                            Ok(x) => x,
                                            Err(e) => {
                                                log::error!("Error encoding checksum: {e}");
                                                return warp::http::Response::builder()
                                                    .header("Location", "/unauthorized")
                                                    .status(302)
                                                    .body(String::new());
                                            }
                                        }
                                    ),
                                )
                                .status(302)
                                .body(String::new()),
                            Some(_) => warp::http::Response::builder()
                                .header("Location", "/unauthorized")
                                .status(302)
                                .body(String::new()),
                        }
                    },
                )));

    let (sendkill, kill) = tokio::sync::oneshot::channel::<()>();
    let (killreply, killrecv) = tokio::sync::oneshot::channel::<()>();
    let (_, server) = warp::serve(is_scraper.or(routes)).bind_with_graceful_shutdown(
        (
            [0, 0, 0, 0],
            env!("PORT").parse::<u16>().unwrap_or_else(|_| {
                log::warn!("Failed to parse port, defaulting to 8118");
                8118
            }),
        ),
        async {
            log::info!("Welcome to PChan");
            let _ = kill.await;
            let _ = killreply.send(());
            log::info!("Shutting down Warp server");
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
                            log::error!("Error getting database connection: {e}");
                            continue;
                        }
                    };
                    let files_in_db = match database_bindings::Database::get_all_files(&mut db).await {
                        Ok(x) => x,
                        Err(e) => {
                            log::error!("Error getting files from database: {e}");
                            continue;
                        }
                    };

                    drop(lock);
                    let files_to_delete = files.iter().filter(|x| !files_in_db.iter().any(|v| {
                        // if &&v.claimed.path == x || &&v.claimed.thumbnail == x {
                        //     true
                        // } else {
                        //     println!("{x} is not in {} or {}", v.claimed.path, v.claimed.thumbnail);
                        //     false
                        // }
                        if v.claimed.path.replace(*x, "").is_empty() || v.claimed.path.replace(x.trim_end_matches("-thumb.jpg"), "").is_empty() {
                            log::trace!("`{x}` is `{}` or `{}`", v.claimed.path.replace(*x, ""), v.claimed.path.replace(x.trim_end_matches("-thumb.jpg"), ""));
                            true
                        } else {
                            false
                        }
                    })).cloned().collect::<Vec<String>>().iter().map(|x| format!("{dir}{x}")).collect::<Vec<String>>();

                    for file in files_to_delete {
                        // until i'm confident in the file deletion code, i'm gonna move them to a trash folder instead of deleting them
                        // if let Err(e) = tokio::fs::remove_file(file.clone()).await {
                        //     println!("Error deleting file {file}: {e}");
                        // }
                        log::trace!("Deleting: {file}");
                        let trash_path = format!("{}{}", env!("TRASH_STORAGE_PATH"), file.replace(dir, ""));
                        // remove the last element after the last slash
                        let trash = {
                            let trash_vec = trash_path.split('/').collect::<Vec<&str>>();
                            trash_vec[..trash_vec.len() - 1].join("/")
                        };

                        if let Err(e) = tokio::fs::create_dir_all(trash).await {
                            log::error!("Error creating trash folder: {e}");
                            continue;
                        }

                        if let Err(e) = tokio::fs::rename(file.clone(), trash_path).await {
                            log::error!("Error moving file to trash: {e}");
                        }
                    }
                }
            }
            _ = trim_files.tick() => {
                if let Err(e) = UNCLAIMED_FILES.lock().await.trim_files().await {
                    log::error!("Error trimming files: {e}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                log::trace!("Received SIGINT");
                break;
            }
        }
    }
    log::info!("Awaiting warp shutdown");
    let _ = sendkill.send(());
    let _ = killrecv.await;
    log::trace!("Saving data");
    // DATA.lock().await.close().await.unwrap();
}

#[async_recursion::async_recursion]
async fn get_all_entries(dir: &str) -> anyhow::Result<Vec<tokio::fs::DirEntry>> {
    let mut return_files = Vec::new();
    let mut files = tokio::fs::read_dir(dir).await?;
    while let Some(file) = files.next_entry().await? {
        if file.file_type().await?.is_dir() {
            return_files
                .append(&mut get_all_entries(file.path().to_str().unwrap_or_default()).await?);
        } else {
            return_files.push(file);
        }
    }
    Ok(return_files)
}

fn encode_checksum_str(s: &str) -> anyhow::Result<String> {
    // encode the string with a checksum and encrypt it using our secret key (generated at startup, does not persist)
    // random key is at crate::statics::RANDOM_KEY;

    // checksum is crc32

    let checksum = {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(s.as_bytes());
        hasher.finalize()
    };
    let mut s = s.as_bytes().to_vec();
    s.extend_from_slice(&checksum.to_be_bytes());
    let mut key = statics::RANDOM_KEY.iter().cycle();
    // simple xor encryption
    let s = s
        .iter()
        .map(|x| x ^ key.next().expect("KEY RAN OUT???"))
        .collect::<Vec<u8>>();
    let mut compressor = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    compressor.write_all(&s)?;
    let s = compressor.finish()?;
    Ok(statics::BASE64_ENGINE.encode(s))
}

fn decode_checksum_str(s: &str) -> anyhow::Result<String> {
    // decrypt the string and check the checksum
    let s = statics::BASE64_ENGINE.decode(s.as_bytes())?;
    let mut decompressor = flate2::read::GzDecoder::new(&s[..]);
    let mut s = Vec::new();
    decompressor.read_to_end(&mut s)?;
    let mut key = statics::RANDOM_KEY.iter().cycle();
    let s = s
        .iter()
        .map(|x| x ^ key.next().expect("KEY RAN OUT???"))
        .collect::<Vec<u8>>();
    let (s, checksum) = s.split_at(s.len() - 4);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(s);
    if hasher.finalize() == u32::from_be_bytes([checksum[0], checksum[1], checksum[2], checksum[3]])
    {
        Ok(String::from_utf8(s.to_vec())?)
    } else {
        Err(anyhow::anyhow!("Checksum failed"))
    }
}
