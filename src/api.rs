use std::future::Future;

use askama::Template;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
    response::Redirect,
    routing::get,
    Router,
};
use diesel_async::{pooled_connection::deadpool::Pool, AsyncMysqlConnection};
use tower_http::services::ServeDir;

mod conversations;
mod login;

use conversations::MessagesPage;
use login::{LoginPage, Username};

#[derive(Clone)]
pub struct Application {
    pub db: Pool<AsyncMysqlConnection>,
}

pub fn router() -> Router<Application> {
    Router::new()
        .nest_service("/static", ServeDir::new("static/"))
        .nest("/login", login::router())
        .nest("/conversations", conversations::router())
        .route(
            "/conversations-list/:request-type",
            get(conversations::get_conversation_previews),
        )
        .route("/", get(|| async { Redirect::permanent("/conversations") }))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not a valid url on this server!") })
}

enum Content {
    Login(LoginPage),
    Messages(MessagesPage),
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Root {
    content: Content,
}

#[derive(Debug, Clone)]
pub struct HtmxRequest {
    restore: bool,
}

impl<A: Send + Sync> FromRequestParts<A> for HtmxRequest {
    type Rejection = (StatusCode, &'static str);

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 A,
    ) -> ::core::pin::Pin<
        Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async {
            let headers = HeaderMap::from_request_parts(parts, state).await.unwrap();
            if !headers
                .get("HX-Request")
                .is_some_and(|value| value.as_bytes() == "true".as_bytes())
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Expected HTMX request for this endpoint!",
                ));
            };

            return Ok(Self {
                restore: headers
                    .get("HX-History-Restore-Request")
                    .is_some_and(|value| value.as_bytes() == "true".as_bytes()),
            });
        })
    }
}
