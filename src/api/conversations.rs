use std::borrow::BorrowMut;

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Form, Router,
};
use axum_extra::either::Either;
use chrono::{DateTime, Utc};
use diesel::{
    alias,
    dsl::{exists, not},
    prelude::*,
};
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::Deserialize;

use crate::model::{
    self,
    schema::{self, messages::dsl},
};

use super::{Application, Content, HtmxRequest, Root, Username};

pub fn router() -> Router<Application> {
    Router::new()
        .route("/", get(get_conversations))
        .route("/:peer", get(get_conversation))
        .route("/:peer", post(send_message))
        .route("/:peer/poll", get(get_new_messages))
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

async fn get_latest_messages(
    mut db: impl AsyncConnection<Backend = diesel::mysql::Mysql>,
    username: &Username,
) -> Vec<model::Message> {
    use crate::model::schema::messages::dsl::*;
    use schema::messages;

    let later_messages = alias!(messages as later_messages);

    messages
        .filter(
            sender.eq(username.as_str()).and(not(exists(
                later_messages
                    .filter(
                        later_messages
                            .field(sender)
                            .eq(username.as_str())
                            .and(later_messages.field(receiver).eq(receiver)),
                    )
                    .or_filter(
                        later_messages
                            .field(receiver)
                            .eq(username.as_str())
                            .and(later_messages.field(sender).eq(receiver)),
                    )
                    .filter(later_messages.field(sent_at).gt(sent_at)),
            ))),
        )
        .or_filter(
            receiver.eq(username.as_str()).and(not(exists(
                later_messages
                    .filter(
                        later_messages
                            .field(sender)
                            .eq(username.as_str())
                            .and(later_messages.field(receiver).eq(sender)),
                    )
                    .or_filter(
                        later_messages
                            .field(receiver)
                            .eq(username.as_str())
                            .and(later_messages.field(sender).eq(sender)),
                    )
                    .filter(later_messages.field(sent_at).gt(sent_at)),
            ))),
        )
        .order_by(sent_at.desc())
        .select(model::Message::as_select())
        .load(&mut db)
        .await
        .unwrap()
}

pub async fn get_conversations(
    State(Application { db }): State<Application>,
    username: Username,
) -> Root {
    let most_recent_messages = get_latest_messages(db.get().await.unwrap(), &username).await;

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
    use crate::model::schema::messages::dsl::*;

    let messages_in_convo = messages
        .filter(sender.eq(username.as_str()).and(receiver.eq(&peer)))
        .or_filter(sender.eq(&peer).and(receiver.eq(username.as_str())))
        .order_by(sent_at.desc())
        .select(model::Message::as_select())
        .load(db.get().await.unwrap().as_mut())
        .await
        .unwrap();

    let conversation = ConversationView {
        messages: AutoRefreshMessages {
            peer,
            messages: messages_in_convo
                .into_iter()
                .map(|msg| (msg.sender == username.as_str(), msg).into())
                .collect(),
            timestamp: Utc::now(),
        },
    };

    dbg!(&htmx);

    if let None | Some(HtmxRequest { restore: true, .. }) = htmx {
        let most_recent_messages = get_latest_messages(db.get().await.unwrap(), &username).await;

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
    timestamp: String,
}

pub async fn send_message(
    State(Application { db, .. }): State<Application>,
    Path(SendMessagePath { peer }): Path<SendMessagePath>,
    username: Username,
    Form(SendMessageForm {
        new_message_content,
        timestamp,
    }): Form<SendMessageForm>,
) -> Result<AutoRefreshMessages, StatusCode> {
    let timestamp = urlencoding::decode(&timestamp).map_err(|_| StatusCode::BAD_REQUEST)?;
    let timestamp: DateTime<Utc> = timestamp.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    model::NewMessage {
        sender: username.to_owned(),
        receiver: peer.clone(),
        content: new_message_content.clone(),
    }
    .insert_into(dsl::messages)
    .execute(db.get().await.unwrap().borrow_mut())
    .await
    .unwrap();

    use crate::model::schema::messages::dsl::*;
    use schema::messages;

    let new_messages = messages
        .filter(sender.eq(username.as_str()).and(receiver.eq(&peer)))
        .or_filter(sender.eq(&peer).and(receiver.eq(username.as_str())))
        .filter(sent_at.ge(timestamp.naive_utc()))
        .select(model::Message::as_select())
        .order_by(messages::sent_at.desc())
        .load(db.get().await.unwrap().as_mut())
        .await
        .unwrap();

    Ok(AutoRefreshMessages {
        messages: new_messages
            .into_iter()
            .map(|msg| (username.as_str() == msg.sender, msg).into())
            .collect(),
        timestamp: Utc::now(),
        peer,
    })
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
    State(Application { db }): State<Application>,
    Path(GetNewMessagesPath { peer }): Path<GetNewMessagesPath>,
    Query(GetNewMessagesQuery { timestamp }): Query<GetNewMessagesQuery>,
    username: Username,
) -> Result<AutoRefreshMessages, StatusCode> {
    let timestamp = urlencoding::decode(&timestamp).map_err(|_| StatusCode::BAD_REQUEST)?;
    let timestamp: DateTime<Utc> = timestamp.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    use crate::model::schema::messages::dsl::*;
    use schema::messages;

    let new_messages = messages
        .filter(sender.eq(username.as_str()).and(receiver.eq(&peer)))
        .or_filter(sender.eq(&peer).and(receiver.eq(username.as_str())))
        .filter(sent_at.ge(timestamp.naive_utc()))
        .select(model::Message::as_select())
        .order_by(messages::sent_at.desc())
        .load(db.get().await.unwrap().as_mut())
        .await
        .unwrap();

    if new_messages.is_empty() {
        return Err(StatusCode::NO_CONTENT);
    };

    Ok(AutoRefreshMessages {
        messages: new_messages
            .into_iter()
            .map(|msg| (username.as_str() == msg.sender, msg).into())
            .collect(),
        timestamp: Utc::now(),
        peer,
    })
}
