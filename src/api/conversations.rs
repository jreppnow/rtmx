use std::{
    borrow::{BorrowMut, Cow},
    fmt::Display,
};

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Form, Router,
};
use axum_extra::either::Either;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::Deserialize;

use crate::model::{self, schema::messages::dsl, Message as DbMessage};

use super::{Application, Content, HtmxRequest, Root, Username};

const MESSAGE_LIMIT: usize = 10;

pub fn router() -> Router<Application> {
    Router::new()
        .route("/", get(get_conversations))
        .route("/:peer", get(get_conversation))
        .route("/:peer", post(send_message))
        .route("/:peer/poll", get(get_new_messages))
        .route("/:peer/:direction", get(load_more))
}

#[derive(Template, Default)]
#[template(path = "messages.html")]
pub struct MessagesPage {
    conversations: Vec<ConversationPreview>,
    selected: Option<ConversationView>,
}

#[derive(Debug, Clone)]
struct ConversationPreview {
    peer: String,
    date: String,
    preview: String,
}

impl From<(model::Message, &str)> for ConversationPreview {
    fn from((message, username): (model::Message, &str)) -> Self {
        Self {
            peer: if message.sender == username {
                message.receiver
            } else {
                message.sender
            },
            date: message.sent_at.to_string(),
            preview: message.content,
        }
    }
}

pub async fn get_conversations(
    State(Application { db }): State<Application>,
    username: Username,
) -> Root {
    let mut db = db.get().await.unwrap();

    let most_recent_messages = DbMessage::most_recent(&username)
        .load(&mut db)
        .await
        .unwrap();

    Root {
        content: Content::Messages(MessagesPage {
            conversations: most_recent_messages
                .into_iter()
                .map(|message| (message, username.as_str()).into())
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
    last_seen_message_id: Option<u64>,
    peer: String,
}

impl AutoRefreshMessages {
    pub fn new<'a>(messages: Vec<DbMessage>, user: &str, peer: impl Into<Cow<'a, str>>) -> Self {
        let last_seen_message_id = messages.as_slice().first().map(|message| message.id);

        Self {
            peer: peer.into().into_owned(),
            messages: messages
                .into_iter()
                .map(|msg| (msg.sender == user, msg).into())
                .collect(),
            last_seen_message_id,
        }
    }
}

#[derive(Template, Default)]
#[template(path = "conversation.html")]
pub struct ConversationView {
    messages: AutoRefreshMessages,
    lazy_load: Option<LoadMore>,
}

#[derive(Template, Debug, Clone)]
#[template(path = "message.html")]
pub struct Message {
    yours: bool,
    id: u64,
    content: String,
    date: String,
}

impl From<(bool, model::Message)> for Message {
    fn from((yours, msg): (bool, model::Message)) -> Self {
        Self {
            yours,
            id: msg.id,
            content: msg.content,
            date: msg.sent_at.to_string(),
        }
    }
}

pub async fn get_conversation(
    State(Application { db }): State<Application>,
    htmx: Option<HtmxRequest>,
    Path(GetConversation { peer }): Path<GetConversation>,
    username: Username,
) -> Either<Root, ConversationView> {
    let messages_in_convo = DbMessage::limited((&peer, &username), MESSAGE_LIMIT)
        .load(db.get().await.unwrap().as_mut())
        .await
        .unwrap();

    let lazy_load = LoadMore::new(LoadDirection::Earlier, &messages_in_convo, peer.clone());

    let conversation = ConversationView {
        messages: AutoRefreshMessages::new(messages_in_convo, &username, &peer),
        lazy_load,
    };

    if let None | Some(HtmxRequest { restore: true, .. }) = htmx {
        let mut db = db.get().await.unwrap();
        let most_recent_messages = DbMessage::most_recent(&username)
            .load(&mut db)
            .await
            .unwrap();

        Either::E1(Root {
            content: Content::Messages(MessagesPage {
                conversations: most_recent_messages
                    .into_iter()
                    .map(|message| (message, username.as_str()).into())
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
    #[serde(rename = "last-seen-message-id")]
    last_seen_message_id: Option<u64>,
}

pub async fn send_message(
    State(Application { db, .. }): State<Application>,
    Path(SendMessagePath { peer }): Path<SendMessagePath>,
    username: Username,
    Form(SendMessageForm {
        new_message_content,
        last_seen_message_id,
    }): Form<SendMessageForm>,
) -> Result<AutoRefreshMessages, StatusCode> {
    model::NewMessage {
        sender: username.to_owned(),
        receiver: peer.clone(),
        content: new_message_content.clone(),
    }
    .insert_into(dsl::messages)
    .execute(db.get().await.unwrap().borrow_mut())
    .await
    .unwrap();

    let mut db = db.get().await.unwrap();
    let new_messages = if let Some(last_seen_id) = last_seen_message_id {
        DbMessage::after((&peer, &username), last_seen_id).load(db.as_mut())
    } else {
        DbMessage::between((&peer, &username)).load(db.as_mut())
    }
    .await
    .unwrap();

    Ok(AutoRefreshMessages::new(new_messages, &username, &peer))
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetNewMessagesPath {
    peer: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetNewMessagesQuery {
    #[serde(alias = "last-seen-message-id")]
    last_seen_message_id: Option<u64>,
}

pub async fn get_new_messages(
    State(Application { db }): State<Application>,
    Path(GetNewMessagesPath { peer }): Path<GetNewMessagesPath>,
    Query(GetNewMessagesQuery {
        last_seen_message_id,
    }): Query<GetNewMessagesQuery>,
    username: Username,
) -> Result<AutoRefreshMessages, StatusCode> {
    let mut db = db.get().await.unwrap();

    let new_messages = if let Some(last_seen_id) = last_seen_message_id {
        DbMessage::after((&peer, &username), last_seen_id).load(db.as_mut())
    } else {
        DbMessage::between((&peer, &username)).load(db.as_mut())
    }
    .await
    .unwrap();

    if new_messages.is_empty() {
        return Err(StatusCode::NO_CONTENT);
    };

    Ok(AutoRefreshMessages::new(new_messages, &username, &peer))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoadDirection {
    Earlier,
    Later,
}

impl Display for LoadDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LoadDirection::Earlier => "earlier",
                LoadDirection::Later => "later",
            }
        )
    }
}

#[derive(Template, Debug, Clone)]
#[template(path = "load-more.html")]
pub struct LoadMore {
    peer: String,
    id: u64,
    direction: LoadDirection,
}

impl LoadMore {
    pub fn new(direction: LoadDirection, messages: &[DbMessage], peer: String) -> Option<Self> {
        match direction {
            LoadDirection::Earlier => messages.last(),
            LoadDirection::Later => messages.first(),
        }
        .filter(|_| messages.len() == MESSAGE_LIMIT)
        .map(|last_msg| Self {
            peer: peer.clone(),
            id: last_msg.id,
            direction,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoadMorePath {
    peer: String,
    direction: LoadDirection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoadMoreQuery {
    id: u64,
}

#[derive(Template, Debug, Clone)]
#[template(path = "lazy-loaded.html")]
pub struct LazyLoaded {
    messages: Vec<Message>,
    lazy_load: Option<LoadMore>,
}

pub async fn load_more(
    State(Application { db }): State<Application>,
    Path(LoadMorePath { peer, direction }): Path<LoadMorePath>,
    Query(LoadMoreQuery { id }): Query<LoadMoreQuery>,
    username: Username,
) -> LazyLoaded {
    let mut db = db.get().await.unwrap();

    let messages = match direction {
        LoadDirection::Earlier => {
            DbMessage::before_limited((&username, &peer), id, MESSAGE_LIMIT).load(&mut db)
        }
        LoadDirection::Later => {
            DbMessage::after_limited((&username, &peer), id, MESSAGE_LIMIT).load(&mut db)
        }
    }
    .await
    .unwrap();

    let lazy_load = LoadMore::new(direction, &messages, peer.clone());

    LazyLoaded {
        messages: messages
            .into_iter()
            .map(|msg| (msg.sender.as_str() == username.as_str(), msg).into())
            .collect(),
        lazy_load,
    }
}
