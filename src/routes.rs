use std::{array, borrow::Cow, future::Future};

use askama::Template;
use axum::{
    extract::{FromRequestParts, Path},
    http::{request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::Redirect,
    Form,
};
use axum_extra::{
    either::Either,
    extract::{
        cookie::{Cookie, SameSite},
        CookieJar,
    },
};
use serde::Deserialize;

enum Content {
    Login(LoginPage),
    Messages(MessagesPage),
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Root {
    content: Content,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginPage {
    value: String,
    has_error: bool,
    from_validation: bool,
}

impl Default for LoginPage {
    fn default() -> Self {
        Self {
            value: String::new(),
            has_error: true,
            from_validation: false,
        }
    }
}

#[derive(Template, Default)]
#[template(path = "messages.html")]
struct MessagesPage {
    selected: Option<()>,
}

const USER_NAME_COOKIE: &str = "MSGX_USERNAME";

pub async fn login(username: Option<Username>) -> Either<Root, Redirect> {
    if username.is_some() {
        return Either::E2(Redirect::to("/messages"));
    };

    Either::E1(Root {
        content: Content::Login(Default::default()),
    })
}

#[derive(Deserialize)]
pub struct LoginParameters {
    username: String,
}

pub async fn try_login(
    cookies: CookieJar,
    Form(LoginParameters { username }): Form<LoginParameters>,
) -> Either<LoginPage, (CookieJar, Redirect)> {
    if Username::new(&username).is_some() {
        let mut cookie = Cookie::new(USER_NAME_COOKIE, username);
        cookie.set_http_only(true);
        cookie.set_same_site(SameSite::Lax);
        return Either::E2((cookies.add(cookie), Redirect::to("/messages")));
    };

    Either::E1(LoginPage {
        value: username,
        has_error: true,
        from_validation: true,
    })
}

pub async fn validate_username(
    Form(LoginParameters { username }): Form<LoginParameters>,
) -> LoginPage {
    LoginPage {
        has_error: Username::new(&username).is_none(),
        value: username,
        from_validation: true,
    }
}

pub struct Username(String);

impl Username {
    fn new<'a>(s: impl Into<Cow<'a, str>>) -> Option<Self> {
        let s = s.into();
        if s.is_empty() || "test" == s {
            return None;
        }
        // TODO: validation!
        Some(Self(s.into_owned()))
    }
}

impl<A: Send + Sync> FromRequestParts<A> for Username {
    type Rejection = Redirect;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        a: &'life1 A,
    ) -> ::core::pin::Pin<
        Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async {
            let cookies = CookieJar::from_request_parts(parts, a).await.unwrap();

            if let Some(username) = cookies
                .get(USER_NAME_COOKIE)
                .and_then(|cookie| Username::new(cookie.value()))
            {
                println!("User logged in as {}.", &username.0);
                Ok(username)
            } else {
                Err(Redirect::to("/login"))
            }
        })
    }
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
            if headers
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

pub async fn messages(username: Username) -> Root {
    Root {
        content: Content::Messages(MessagesPage {
            conversations: array::from_fn::<_, 10, _>(|index| Conversation {
                name: format!("Message #{index}"),
                date: format!("{index} seconds ago.."),
                preview: "Very important cont...".to_owned(),
            })
            .to_vec(),
            selected: None,
        }),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetMessage {
    message: String,
}

pub async fn message(
    htmx: Option<HtmxRequest>,
    Path(GetMessage { message }): Path<GetMessage>,
    username: Username,
) -> Root {
    Root {
        content: Content::Messages(MessagesPage {
            conversations: array::from_fn::<_, 10, _>(|index| Conversation {
                name: format!("Message #{index}"),
                date: format!("{index} seconds ago.."),
                preview: "Very important cont...".to_owned(),
            })
            .to_vec(),
            selected: None,
        }),
    }
}
