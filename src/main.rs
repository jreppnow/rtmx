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
        .route("/conversations", get(routes::get_conversations))
        .route("/conversations/:peer", get(routes::get_conversation))
        .route("/conversations/:peer", post(routes::send_message))
        .route("/conversations/:peer/poll", get(routes::get_new_messages))
        .route("/", get(|| async { Redirect::permanent("/conversations") }))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not a valid url on this server!") });

    let listener = TcpListener::bind("[::]:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap()
}
