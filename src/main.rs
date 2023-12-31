use axum::{
    http::StatusCode,
    response::Redirect,
    routing::{get, post, put},
    Router,
};
use tokio::{self, net::TcpListener};
use tower_http::services::ServeDir;

mod routes;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .nest_service("/static", ServeDir::new("static/"))
        .route("/login", get(routes::login))
        .route("/login", post(routes::try_login))
        .route("/login/validate", put(routes::validate_username))
        .route("/messages", get(routes::messages))
        .route("/", get(|| async { Redirect::permanent("/messages") }))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not a valid url on this server!") });

    let listener = TcpListener::bind("[::]:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap()
}
