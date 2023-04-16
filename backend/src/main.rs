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

use crate::database::Users;
use profanity::Profanity;

lazy_static::lazy_static! {
    pub static ref POOL: deadpool::managed::Pool<diesel_async::pooled_connection::AsyncDieselConnectionManager<diesel_async::AsyncPgConnection>> = Pool::builder(AsyncDieselConnectionManager::<AsyncPgConnection>::new(std::env::var("DATABASE_URL").expect("DATABASE_URL not set"))).build().expect("Database build failed");
    pub static ref DATA: Arc<Mutex<Users>> = Arc::new(Mutex::new(Users::new().unwrap()));
    pub static ref PROFANITY: Arc<Profanity> = Arc::new(Profanity::load_csv("./profanity_en.csv").expect("Failed to load profanity list"));
}

#[tokio::main]
async fn main() {
    env_logger::init();
    println!("Starting backend with:");

    println!("PROFANITY CHECK: {:?}", PROFANITY.check_profanity("FUCK"));

    {
        if let Err(e) = DATA.lock().await.open().await {
            println!("Error opening data: {e}");
        }
    }

    let root = warp::get().and(
        warp::fs::dir("/git/pchan/frontend/tempdist")
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
                        .body("Invalid token, navigate to /login to login again.".to_owned())
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

    loop {
        tokio::select! {
















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
