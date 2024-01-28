use std::cmp::max;

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Router,
};
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};

use crate::{
    api::{login::Username, Application},
    model::Message as DbMessage,
};

pub fn router() -> Router<Application> {
    Router::new().route("/:request-type", get(get_conversation_previews))
}

#[derive(Debug, Clone)]
struct ConversationPreview {
    peer: String,
    date: String,
    preview: String,
    selected: bool,
}

impl ConversationPreview {
    fn new(message: DbMessage, username: &str, selected: bool) -> Self {
        Self {
            peer: if message.sender == username {
                message.receiver
            } else {
                message.sender
            },
            date: message.sent_at.to_string(),
            preview: message.content,
            selected,
        }
    }
}

#[derive(Template, Debug, Clone, Default)]
#[template(path = "conversations/list/conversation-dynamic-bits.html")]
pub struct ConversationItems {
    conversations: Vec<ConversationPreview>,
    hidden_selected: Option<String>,
    start_new: Option<String>,
    last_seen_id: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RequestType {
    Poll,
    Search,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Ordering {
    Alphabetically,
    MostRecent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GetConversationPreviewsQuery {
    last_seen_id: Option<u64>,
    selected_conversation: Option<String>,
    search_needle: String,
    ordering: Ordering,
}

pub async fn get_conversation_previews(
    State(Application { db }): State<Application>,
    Path(request_type): Path<RequestType>,
    Query(GetConversationPreviewsQuery {
        last_seen_id,
        selected_conversation,
        search_needle,
        ordering,
    }): Query<GetConversationPreviewsQuery>,
    username: Username,
) -> Result<ConversationItems, StatusCode> {
    let mut db = db.get().await.unwrap();

    let mut most_recent_messages = DbMessage::most_recent(&username)
        .load(&mut db)
        .await
        .unwrap();

    // TODO: Include in query!
    if !search_needle.is_empty() {
        most_recent_messages.retain(|msg| {
            (msg.sender == username.as_str() && msg.receiver.contains(&search_needle))
                || (msg.receiver == username.as_str() && msg.sender.contains(&search_needle))
        });
    }

    let newest_id = most_recent_messages.as_slice().first().map(|msg| msg.id);
    match (request_type, newest_id, last_seen_id) {
        (RequestType::Poll, None, _) => return Err(StatusCode::NO_CONTENT),
        (RequestType::Poll, Some(newest_id), Some(last_seen_id)) if last_seen_id >= newest_id => {
            return Err(StatusCode::NO_CONTENT);
        }
        _ => {}
    }

    // TODO: Include in query!
    if let Ordering::Alphabetically = ordering {
        most_recent_messages.sort_by(|left, right| {
            macro_rules! get_peer {
                ($id:ident) => {
                    if $id.sender == username.as_str() {
                        &$id.receiver
                    } else {
                        &$id.sender
                    }
                };
            }

            get_peer!(left).cmp(get_peer!(right))
        });
    }

    let last_seen_id = match (last_seen_id, newest_id) {
        (Some(last_seen_id), Some(newest_id)) => Some(max(last_seen_id, newest_id)),
        (id @ Some(_), None) | (None, id @ Some(_)) => id,
        (None, None) => None,
    };

    let hidden_selected = selected_conversation.as_ref().and_then(|peer| {
        if most_recent_messages
            .iter()
            .any(|msg| msg.is_between((peer, &username)))
        {
            None
        } else {
            Some(peer.to_owned())
        }
    });

    let start_new = Username::new(&search_needle)
        .filter(|peer| {
            !most_recent_messages
                .iter()
                .any(|msg| msg.is_between((&username, &peer)))
        })
        .map(Username::into_inner);

    Ok(ConversationItems {
        conversations: most_recent_messages
            .into_iter()
            .map(|message| {
                let selected = selected_conversation
                    .as_ref()
                    .is_some_and(|peer| message.is_between((peer, &username)));
                ConversationPreview::new(message, username.as_str(), selected)
            })
            .collect(),
        start_new,
        hidden_selected,
        last_seen_id,
    })
}
