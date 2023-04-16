use std::sync::Arc;

use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;

use tokio::sync::Mutex;
use warp::Filter;

mod database;
mod endpoints;
mod filters;
pub mod schema;
mod statics;
mod unclaimedfiles;
use unclaimedfiles::UnclaimedFiles;

use std::collections::HashMap;

use crate::database::Users;
use profanity::Profanity;

lazy_static::lazy_static! {
    pub static ref POOL: deadpool::managed::Pool<diesel_async::pooled_connection::AsyncDieselConnectionManager<diesel_async::AsyncPgConnection>> = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))).build().expect("Database build failed");
    pub static ref DATA: Arc<Mutex<Users>> = Arc::new(Mutex::new(Users::new().unwrap()));
    pub static ref PROFANITY: Arc<Profanity> = Arc::new(Profanity::load_csv("./profanity_en.csv").expect("Failed to load profanity list"));
    pub static ref UNCLAIMED_FILES: Arc<Mutex<UnclaimedFiles>> = Arc::new(Mutex::new(UnclaimedFiles::new(HashMap::new())));
    pub static ref MANUAL_FILE_TRIM: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

#[tokio::main]
async fn main() {
    env_logger::init();
    println!("Starting backend with:");

    println!("PROFANITY CHECK: {:#?}", PROFANITY.check_profanity("FUCK"));

    {
        if let Err(e) = DATA.lock().await.open().await {
            println!("Error opening data: {e}");
        }
    }

    let root = warp::get().and(
        warp::fs::dir(env!("FILE_STORAGE_PATH"))
            .or(warp::fs::dir("/git/pchan/frontend/tempdist"))
            .or(warp::fs::file("/git/pchan/frontend/tempdist/index.html")),
    );

    let unauthorized = warp::path!("unauthorized")
        .and(warp::get())
        .and(warp::fs::file(
            "/git/pchan/frontend/tempdist/unauthorized.html",
        ));

    let routes = endpoints::api::priveleged_api_endpoints().or(filters::valid_token()
        .and(endpoints::api::api_endpoints().or(root))
        .or(endpoints::other_endpoints())
        .or(unauthorized)
        .or(warp::any()
            .and(warp::cookie::optional::<String>("token"))
            .then(|token: Option<String>| async move {
                match token {
                    None => Ok(warp::http::Response::builder()
                        .status(401)
                        .body("Invalid token. navigate to /login to log in.".to_owned())
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
    let (_, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], 16835), async {
            let _ = kill.await;
            let _ = killreply.send(());
            println!("Shutting down Warp server");
        });
    tokio::spawn(server);

    let mut trim_files = tokio::time::interval(std::time::Duration::from_secs(*statics::TRIM_TIME));
    let mut delete_old_files = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut auto_delete = tokio::time::Instant::now();
    // let mut last_trim = tokio::time::Instant::now();
    loop {
        tokio::select! {
            _ = delete_old_files.tick() => {

                // if manual trim is set to true OR auto_delete has elapsed
                if *MANUAL_FILE_TRIM.lock().await || auto_delete.elapsed() >= std::time::Duration::from_secs(*statics::DELETE_TIME) {

                    // println!("Trimming files, last trim was {last_trim}s ago", last_trim = last_trim.elapsed().as_secs());
                    // last_trim = tokio::time::Instant::now();

                    // reset auto_delete
                    auto_delete = tokio::time::Instant::now();
                    // set manual trim to false
                    *MANUAL_FILE_TRIM.lock().await = false;
                    // get the file storage path
                    let dir = env!("FILE_STORAGE_PATH");
                    // get a list of all files in all directories in the dir
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
                    }.iter().flatten().cloned().collect::<Vec<String>>();

                    // get a list of all files that are not in the database.
                    let files_to_delete = files.iter().filter(|x| !files_in_db.contains(x)).cloned().collect::<Vec<String>>().iter().map(|x| format!("{dir}{x}")).collect::<Vec<String>>();

                    // delete all files that are not in the database.
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
    println!("Saving data");
    DATA.lock().await.close().await.unwrap();
}

#[async_recursion::async_recursion]
async fn get_all_entries(dir: &str) -> anyhow::Result<Vec<tokio::fs::DirEntry>> {
    // get a list of all files in all directories in the dir, if the file is a directory, recurse
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
