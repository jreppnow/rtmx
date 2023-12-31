use std::{borrow::Cow, future::Future};

use askama::Template;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{Html, Redirect},
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
    Messages,
}

#[derive(Template)]
#[template(path = "index.html")]
struct Root {
    content: Content,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    value: String,
    has_error: bool,
}

impl Default for LoginPage {
    fn default() -> Self {
        Self {
            value: String::new(),
            has_error: true,
        }
    }
}

#[derive(Template, Default)]
#[template(path = "messages.html")]
struct MessagesPage {
    selected: Option<()>,
}

const USER_NAME_COOKIE: &str = "MSGX_USERNAME";

pub async fn login(username: Option<Username>) -> Either<Html<String>, Redirect> {
    if username.is_some() {
        return Either::E2(Redirect::to("/messages"));
    };

    Either::E1(Html(
        Root {
            content: Content::Login(Default::default()),
        }
        .to_string(),
    ))
}

#[derive(Deserialize)]
pub struct LoginParameters {
    username: String,
}

pub async fn try_login(
    cookies: CookieJar,
    Form(LoginParameters { username }): Form<LoginParameters>,
) -> Either<Html<String>, (CookieJar, Redirect)> {
    if Username::new(&username).is_some() {
        let mut cookie = Cookie::new(USER_NAME_COOKIE, username);
        cookie.set_http_only(true);
        cookie.set_same_site(SameSite::Lax);
        return Either::E2((cookies.add(cookie), Redirect::to("/messages")));
    };

    Either::E1(Html(
        LoginPage {
            value: username,
            has_error: true,
        }
        .to_string(),
    ))
}

pub async fn validate_username(
    Form(LoginParameters { username }): Form<LoginParameters>,
) -> Html<String> {
    Html(
        LoginPage {
            has_error: Username::new(&username).is_none(),
            value: username,
        }
        .to_string(),
    )
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

pub async fn messages(username: Username) -> Html<String> {
    Html(
        Root {
            content: Content::Messages,
        }
        .to_string(),
    )
}
