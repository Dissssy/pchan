#![feature(async_iterator)]
// Every `.or()` in the route tree nests another layer into one enormous filter type. The
// admin and asset endpoints pushed it past the default limit.
#![recursion_limit = "512"]
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
use std::sync::Arc;

use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use filters::allow_public_shell;
use std::os::linux::fs::MetadataExt;

use tokio::sync::Mutex;
use warp::{http::HeaderValue, Filter, Reply};

mod assets;
mod auth;
mod config;
mod database_bindings;
mod endpoints;
mod filters;
mod client_log;
#[cfg(test)]
mod testing;
mod push;
mod session;
// pub mod schema;
mod statics;
mod unclaimedfiles;
use unclaimedfiles::UnclaimedFiles;
pub mod quotes;
use quotes::Quotes;

use std::collections::HashMap;

// use crate::database_bindings::Users;
use profanity::Profanity;

use crate::filters::{optional_file_sig, optional_viewer, user_agent_is_scraper, FileSig};

lazy_static::lazy_static! {
    pub static ref POOL: deadpool::managed::Pool<diesel_async::pooled_connection::AsyncDieselConnectionManager<diesel_async::AsyncPgConnection>> = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))).build().expect("Database build failed");
    // pub static ref DATA: Arc<Mutex<Users>> = Arc::new(Mutex::new(Users::new().unwrap()));
    pub static ref PROFANITY: Arc<Profanity> = Arc::new(Profanity::load_csv(env!("PROFANITY_PATH")).expect("Failed to load profanity list"));
    pub static ref UNCLAIMED_FILES: Arc<Mutex<UnclaimedFiles>> = Arc::new(Mutex::new(UnclaimedFiles::new(HashMap::new())));
    pub static ref MANUAL_FILE_TRIM: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref FS_LOCK: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    pub static ref QUOTES: Arc<Quotes> = Arc::new(Quotes::load(env!("QUOTES_PATH")).expect("Failed to load quotes"));
    pub static ref RATELIMIT: Arc<Mutex<HashMap<String, tokio::time::Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    /// Errors reported by the web client. Written to `FRONTEND_LOG_PATH`, which defaults to
    /// `logs/frontend-errors.json` relative to the working directory.
    pub static ref CLIENT_LOG: Mutex<client_log::ClientLog> = Mutex::new(client_log::ClientLog::load(
        std::env::var("FRONTEND_LOG_PATH").unwrap_or_else(|_| "logs/frontend-errors.json".to_owned())
    ));
    pub static ref PUSH_NOTIFS: push::SharedHub = Arc::new(std::sync::Mutex::new(push::Hub::new()));
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
    // Before anything reads the environment. POOL is lazily initialised from DATABASE_URL, so
    // this has to happen before the first database access.
    config::load_dotenv();

    // Fail fast and loudly: a missing or reused key is a silent anonymity failure if it is
    // only noticed once requests are being served.
    if let Err(e) = config::init() {
        eprintln!("configuration error: {e:#}");
        std::process::exit(1);
    }

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

    let root =
        warp::get() /*.and(filters::is_beta())*/
            .and(
                warp::fs::dir(env!("FILE_STORAGE_PATH"))
                    .and(warp::path::full())
                    .and(optional_viewer())
                    .and(optional_file_sig())
                    .and_then(
                        |reply: warp::filters::fs::File,
                         path: warp::path::FullPath,
                         viewer: Option<database::Viewer>,
                         file_sig: Option<FileSig>| async move {
                            let path = path.as_str().to_string();

                            let mut resp = reply.into_response();
                            if let Some(content_type) =
                                resp.headers().get(warp::http::header::CONTENT_TYPE)
                            {
                                let content_type = content_type.to_str().unwrap_or("NO MIMETYPE");
                                if !is_safe_mimetype(content_type) {
                                    log::trace!(
                                        "Forcing download of file with mimetype {}",
                                        content_type
                                    );
                                    resp.headers_mut().insert(
                                        warp::http::header::CONTENT_DISPOSITION,
                                        HeaderValue::from_static("attachment"),
                                    );
                                }
                            }

                            if path.contains("/files/") {
                                let mut conn =
                                    POOL.get().await.map_err(|_| warp::reject::reject())?;

                                // This performs the private-board access check. It used to be
                                // skipped entirely for any path ending in -thumb.jpg, which
                                // made every private board's thumbnails world-readable, and
                                // skipped for any authenticated member, which let any member
                                // read files from boards they were never invited to.
                                let file = database::get_file_from_path(
                                    &mut conn,
                                    viewer.as_ref(),
                                    path.trim_end_matches("-thumb.jpg"),
                                )
                                .await
                                .map_err(|e| {
                                    log::error!("Error getting file: {e}");
                                    warp::reject::reject()
                                })?;

                                // A member the check above accepted for this board.
                                if viewer.is_some() {
                                    return Ok(resp);
                                }

                                // Unauthenticated. The lookup already rejected private boards,
                                // so a known file reaching this point is on a public one.
                                // Thumbnails have to stay anonymously readable or link embeds
                                // break: Discord fetches og:image with no cookie, and via its
                                // media proxy rather than the Discordbot user agent, so it
                                // does not take the is_scraper branch.
                                if file.is_some() && path.ends_with("-thumb.jpg") {
                                    return Ok(resp);
                                }

                                if let Some(file_sig) = file_sig {
                                    if file_sig.validates(&path).await {
                                        return Ok(resp);
                                    }
                                }

                                return Err(warp::reject::reject());
                            }

                            if viewer.is_some() {
                                return Ok(resp);
                            }

                            Err(warp::reject::reject())
                        },
                    )
                    .or(allow_public_shell()
                        .map(|_| {})
                        .untuple_one()
                        .and(warp::fs::dir(env!("DISTRIBUTION_PATH")).or(warp::fs::file(
                            format!("{}/index.html", env!("DISTRIBUTION_PATH")),
                        )))),
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

    // The frontend shell is now served to everyone. A logged-out visitor needs to be able to
    // reach the login and request-access pages, so there is no longer a catch-all that
    // redirects anonymous requests to /login. Each API route carries its own viewer() gate
    // instead of one blanket gate around the whole tree.
    // Anything under /api/ that no route accepted is an authentication failure. Without this
    // it falls through to the SPA shell below and a caller expecting JSON gets a 200 with a
    // page of HTML, which is indistinguishable from success until it fails to parse.
    let api_unauthorized = warp::path("api").and(warp::path::tail()).map(|_| {
        warp::reply::with_status(
            warp::reply::json(&"Not authenticated"),
            warp::http::StatusCode::UNAUTHORIZED,
        )
    });

    let routes = endpoints::other_endpoints()
        .or(endpoints::api::retired_bot_endpoints())
        .or(endpoints::api::api_endpoints())
        .or(api_unauthorized)
        .or(root)
        .or(manifest)
        .or(icon);

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

    // Errors the web client reported against an older frontend build are history, not news.
    if let Some(build) = endpoints::served_frontend_build().await {
        if let Err(e) = CLIENT_LOG.lock().await.sync_build(&build).await {
            log::error!("could not tidy the client error log: {e:#}");
        }
    }

    // Rows from before endpoint hashes existed get one, so the first subscribe from a shared
    // browser can find and replace the previous member's row.
    match POOL.get().await {
        Ok(mut conn) => {
            if let Err(e) = database::backfill_push_endpoint_hashes(&mut conn).await {
                log::error!("could not backfill push endpoint hashes: {e:#}");
            }
        }
        Err(e) => log::error!("could not backfill push endpoint hashes: {e}"),
    }

    let mut trim_files = tokio::time::interval(std::time::Duration::from_secs(*statics::TRIM_TIME));
    let mut delete_old_files = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut purge_sessions = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
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
                    // Site assets live under the same storage root but are not rows in
                    // `files`, so the sweep below would move every uploaded banner and
                    // spoiler to trash within minutes of upload.
                    let files_to_delete = files.iter().filter(|x| !x.starts_with("/files/assets/")).filter(|x| !files_in_db.iter().any(|v| {
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
            _ = purge_sessions.tick() => {
                // Expired rows are unreadable but still a record of when people were active.
                match POOL.get().await {
                    Ok(mut conn) => match session::purge_expired(&mut conn).await {
                        Ok(n) if n > 0 => log::info!("purged {n} expired sessions"),
                        Ok(_) => {}
                        Err(e) => log::error!("Error purging sessions: {e}"),
                    },
                    Err(e) => log::error!("Error connecting to purge sessions: {e}"),
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

