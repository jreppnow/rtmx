use askama::Template;
use axum::{routing::get, Router};

use super::{Application, Content, Root, Username};

mod direct;
mod list;

pub fn router() -> Router<Application> {
    Router::new()
        .route("/", get(get_conversations))
        .nest("/list", list::router())
        .nest("/direct", direct::router())
}

#[derive(Template, Default)]
#[template(path = "conversations/index.html")]
pub struct MessagesPage {
    selected: Option<String>,
}

pub async fn get_conversations(_username: Username) -> Root {
    Root {
        content: Content::Messages(MessagesPage { selected: None }),
    }
}
