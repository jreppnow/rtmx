use axum::Router;
use tower_http::services::ServeDir;

mod cpu_load;
mod loading;
mod lorem_ipsum;

#[tokio::main]
async fn main() {
    let router = Router::new()
        .nest("/lorem_ipsum", lorem_ipsum::router())
        .nest("/loading", loading::router())
        .nest("/cpu_load", cpu_load::router())
        .nest_service("/assets", ServeDir::new("./target"));

    axum::serve(
        tokio::net::TcpListener::bind("[::1]:8080").await.unwrap(),
        router,
    )
    .await
    .unwrap()
}
