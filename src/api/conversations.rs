use askama::Template;
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    routing::{get, post},
    Form, Router,
};
use axum_extra::either::Either;
use chrono::{DateTime, Utc};
use serde::Deserialize;

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
    let updates = changes_since(&username, &peer, timestamp);

    if updates.is_empty() {
        return Err(StatusCode::NO_CONTENT);
    };

    Ok(AutoRefreshMessages {
        messages: updates,
        timestamp: Utc::now(),
        peer,
    })
}
