use std::{env::var, error::Error};

use diesel_async::pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager};
use tokio::{self, net::TcpListener};

mod api;
mod model;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + 'static>> {
    dotenv::dotenv()?;

    let config = AsyncDieselConnectionManager::<diesel_async::AsyncMysqlConnection>::new(var(
        "DATABASE_URL",
    )?);

    let app = api::router().with_state(api::Application {
        db: Pool::builder(config).build()?,
    });

    let listener = TcpListener::bind("[::]:3000").await.unwrap();

    axum::serve(listener, app).await.map_err(Into::into)
}
