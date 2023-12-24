use axum::{routing::get, Router};
use tokio::{self, net::TcpListener};

mod routes;
mod templates;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| routes::index()));

    let listener = TcpListener::bind("[::]:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap()
}
