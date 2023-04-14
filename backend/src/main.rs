#![allow(clippy::box_default)]
use std::sync::Arc;

use tokio::sync::Mutex;
use warp::Filter;

// pub mod datahandlers;
mod database;
mod endpoints;
mod filters;
mod statics;
// pub mod structs;
// pub mod traits;

use common::format_seconds;

// use crate::endpoints::{api::private_endpoints, other_endpoints, priveleged::priveleged_endpoints};

lazy_static::lazy_static! {
    pub static ref DATA: Arc<Mutex<database::DBConnection>> = Arc::new(Mutex::new(database::DBConnection::new("localhost".to_owned(), 5432, "pchan".to_owned(), "pchan".to_owned(), "pchan".to_owned())));
}

#[tokio::main]
async fn main() {
    env_logger::init();
    println!("Starting backend with:");
    // println!("SHARE_TIME: {}", format_seconds(*statics::SHARE_TIME));
    // println!("VALUE_TIME: {}", format_seconds(*statics::VALUE_TIME));
    // println!("TRIM_TIME: {}", format_seconds(*statics::TRIM_TIME));
    // println!("HISTORY_TIME: {}", format_seconds(*statics::HISTORY_TIME));
    // // println!("TOTAL_SHARES: {}", *statics::TOTAL_SHARES);
    // println!("STARTING_CASH: {}", *statics::STARTING_CASH);

    {
        if let Err(e) = DATA.lock().await.open().await {
            println!("Error opening data: {e}");
        }
    }

    let root = warp::get()
        .and(warp::fs::dir("../frontend/dist").or(warp::fs::file("../frontend/dist/index.html")));

    // let routes = priveleged_endpoints()
    //     .or(private_endpoints())
    //     .or(other_endpoints())
    //     .or(root);
    let (sendkill, kill) = tokio::sync::oneshot::channel::<()>();
    let (killreply, killrecv) = tokio::sync::oneshot::channel::<()>();
    let (_, server) = warp::serve(
        filters::valid_token()
            .and(endpoints::api::api_endpoints().or(root))
            .or(endpoints::other_endpoints())
            .or(warp::any().then(|| async move {
                Ok(warp::http::Response::builder()
                    .header("Location", "/login")
                    .status(302)
                    .body("".to_owned())
                    .unwrap())
            })),
    )
    .bind_with_graceful_shutdown(([0, 0, 0, 0], 16835), async {
        let _ = kill.await;
        let _ = killreply.send(());
        println!("Shutting down Warp server");
    });
    tokio::spawn(server);

    let mut trim = tokio::time::interval(std::time::Duration::from_secs(*statics::TRIM_TIME));
    loop {
        tokio::select! {
            _ = trim.tick() => {
                {
                    let mut data = DATA.lock().await;
                    if let Err(e) = data.trim().await {
                        println!("Error: {e}");
                    }
                }
            }
            // _ = history.tick() => {
            //     {
            //         let mut data = DATA.lock().await;
            //         if let Err(e) = data.make_history().await {
            //             println!("Error: {e}");
            //         }
            //     }
            // }
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
