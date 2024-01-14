use std::{borrow::Cow, collections::HashMap, future::Future, pin::Pin};

use askama::Template;
use axum::{
    extract::FromRequestParts,
    http::{
        header::{InvalidHeaderName, InvalidHeaderValue},
        request::Parts,
        HeaderMap, HeaderName, HeaderValue, StatusCode,
    },
    response::{IntoResponse, IntoResponseParts, Redirect},
    routing::get,
    Router,
};
use diesel_async::{pooled_connection::deadpool::Pool, AsyncMysqlConnection};
use serde::{Serialize, Serializer};
use serde_json::value::Serializer as JsonSerializer;
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
    ) -> Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>>
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

#[allow(unused)]
pub enum HxTrigger<P = ()> {
    NameOnly(Cow<'static, str>),
    WithPayload(Cow<'static, str>, P),
}

pub enum CouldNotCreateHeader {
    FailedToSerialize(<JsonSerializer as Serializer>::Error),
    InvalidHeaderName(InvalidHeaderName),
    InvalidHeaderValue(InvalidHeaderValue),
}

impl IntoResponse for CouldNotCreateHeader {
    fn into_response(self) -> askama_axum::Response {
        match self {
            CouldNotCreateHeader::FailedToSerialize(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("serialization of HX-Trigger header JSON payload failed: {e}"),
            ),
            CouldNotCreateHeader::InvalidHeaderName(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("serialization of HX-Trigger header name failed: {e}"),
            ),
            CouldNotCreateHeader::InvalidHeaderValue(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("serialization of HX-Trigger header value failed: {e}"),
            ),
        }
        .into_response()
    }
}

impl<P: Serialize> IntoResponseParts for HxTrigger<P> {
    type Error = CouldNotCreateHeader;

    fn into_response_parts(
        self,
        mut res: axum::response::ResponseParts,
    ) -> Result<axum::response::ResponseParts, Self::Error> {
        let header_name: HeaderName = "HX-Trigger"
            .parse()
            .map_err(CouldNotCreateHeader::InvalidHeaderName)?;

        let header_value: HeaderValue = match self {
            HxTrigger::NameOnly(name) => name
                .parse()
                .map_err(CouldNotCreateHeader::InvalidHeaderValue)?,
            HxTrigger::WithPayload(name, payload) => {
                let mut values = HashMap::new();
                values.insert(name, payload);

                let header_value = serde_json::to_string(&values)
                    .map_err(CouldNotCreateHeader::FailedToSerialize)?;

                header_value
                    .parse()
                    .map_err(CouldNotCreateHeader::InvalidHeaderValue)?
            }
        };

        res.headers_mut().insert(header_name, header_value);
        Ok(res)
    }
}
