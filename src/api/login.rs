use std::{borrow::Cow, future::Future, ops::Deref};

use askama::Template;
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::Redirect,
    routing::{get, post, put},
    Form, Router,
};
use axum_extra::{
    either::Either,
    extract::{
        cookie::{Cookie, SameSite},
        CookieJar,
    },
};
use serde::Deserialize;

use super::{Application, Content, Root};

pub fn router() -> Router<Application> {
    Router::new()
        .route("/", get(login))
        .route("/", post(try_login))
        .route("/validate", put(validate_username))
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

const USER_NAME_COOKIE: &str = "MSGX_USERNAME";

pub async fn login(username: Option<Username>) -> Either<Root, Redirect> {
    if username.is_some() {
        return Either::E2(Redirect::to("/conversations"));
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
        return Either::E2((cookies.add(cookie), Redirect::to("/conversations")));
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
    pub fn new<'a>(s: impl Into<Cow<'a, str>>) -> Option<Self> {
        let s = s.into();
        if s.is_empty()
            || s.len() > 20
            || !s.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_uppercase()
                    || (index > 0 && byte.is_ascii_digit())
            })
        {
            return None;
        }

        Some(Self(s.into_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for Username {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
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
