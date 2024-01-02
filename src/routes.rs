use std::{borrow::Cow, future::Future};

use askama::Template;
use axum::{
    extract::{FromRequestParts, Path, Query},
    http::{request::Parts, HeaderMap, StatusCode},
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
use chrono::{DateTime, Utc};
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
    conversations: Vec<ConversationPreview>,
    selected: Option<ConversationView>,
}

#[derive(Debug, Clone)]
struct ConversationPreview {
    peer: String,
    date: String,
    preview: String,
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

pub async fn get_conversations(_username: Username) -> Root {
    Root {
        content: Content::Messages(MessagesPage {
            conversations: (0..30)
                .map(|index| ConversationPreview {
                    peer: format!("user{index:04}"),
                    date: format!("{index} seconds ago.."),
                    preview: "Very important cont...".to_owned(),
                })
                .collect(),
            selected: None,
        }),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetConversation {
    peer: String,
}

#[derive(Template, Default)]
#[template(path = "auto-refresh-messages.html")]
pub struct AutoRefreshMessages {
    messages: Vec<Message>,
    timestamp: DateTime<Utc>,
    peer: String,
}

#[derive(Template, Default)]
#[template(path = "conversation.html")]
pub struct ConversationView {
    messages: AutoRefreshMessages,
}

#[derive(Template, Debug, Clone)]
#[template(path = "message.html")]
pub struct Message {
    yours: bool,
    id: u64,
    content: String,
    date: String,
}

pub async fn get_conversation(
    htmx: Option<HtmxRequest>,
    Path(GetConversation { peer }): Path<GetConversation>,
    _username: Username,
) -> Either<Root, ConversationView> {
    let conversation = ConversationView {
        messages: AutoRefreshMessages {
            peer,
            messages: (0..40)
                .map(|index| Message {
                    yours: index % 2 == 0,
                    id: index as u64,
                    content: if index % 2 == 0 { "Ping!" } else { "Pong!" }.to_owned(),
                    date: format!("{index} seconds ago.."),
                })
                .collect(),
            timestamp: Utc::now(),
        },
    };

    dbg!(&htmx);

    if let None | Some(HtmxRequest { restore: true, .. }) = htmx {
        Either::E1(Root {
            content: Content::Messages(MessagesPage {
                conversations: (0..40)
                    .map(|index| ConversationPreview {
                        peer: format!("user{index:04}"),
                        date: format!("{index} seconds ago.."),
                        preview: "Very important cont...".to_owned(),
                    })
                    .collect(),
                selected: Some(conversation),
            }),
        })
    } else {
        Either::E2(conversation)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendMessagePath {
    peer: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageForm {
    #[serde(rename = "new-message-content")]
    new_message_content: String,
}

pub async fn send_message(
    Path(SendMessagePath { peer }): Path<SendMessagePath>,
    _username: Username,
    Form(SendMessageForm {
        new_message_content,
    }): Form<SendMessageForm>,
) -> AutoRefreshMessages {
    // TODO: check for new messages also DB and such..

    AutoRefreshMessages {
        messages: vec![Message {
            yours: true,
            id: 1337,
            content: new_message_content,
            date: format!("{}", Utc::now().format("%H:%M")),
        }],
        timestamp: Utc::now(),
        peer,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetNewMessagesPath {
    peer: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetNewMessagesQuery {
    timestamp: String,
}

pub async fn get_new_messages(
    Path(GetNewMessagesPath { peer }): Path<GetNewMessagesPath>,
    Query(GetNewMessagesQuery { timestamp }): Query<GetNewMessagesQuery>,
    username: Username,
) -> Result<AutoRefreshMessages, StatusCode> {
    let timestamp = urlencoding::decode(&timestamp).map_err(|_| StatusCode::BAD_REQUEST)?;
    let timestamp: DateTime<Utc> = timestamp.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    fn changes_since(_username: &str, _peer: &str, _timestamp: DateTime<Utc>) -> Vec<Message> {
        // TODO: check if there are any new messages in this conversation in the DB and such..
        vec![Message {
            yours: false,
            id: 123123213,
            content: "Are you there?".to_owned(),
            date: format!("{}", Utc::now().format("%H:%M")),
        }]
    }
    let updates = changes_since(&username.0, &peer, timestamp);

    if updates.is_empty() {
        return Err(StatusCode::NO_CONTENT);
    };

    Ok(AutoRefreshMessages {
        messages: updates,
        timestamp: Utc::now(),
        peer,
    })
}
