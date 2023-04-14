use warp::Filter;

pub fn api_endpoints() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
{
    warp::path!("api").and(warp::get()).and_then(|| async move {
        Ok::<warp::reply::Json, warp::reject::Rejection>(warp::reply::json(
            &"Hello, world!".to_owned(),
        ))
    })
}
