pub mod schema;

use chrono::NaiveDateTime;
use diesel::backend::Backend;
use diesel::prelude::*;

#[derive(Insertable)]
#[diesel(table_name = schema::messages)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct NewMessage {
    pub sender: String,
    pub receiver: String,
    pub content: String,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = schema::messages)]
#[diesel(check_for_backend(diesel::mysql::Mysql))]
pub struct Message {
    pub id: u64,
    pub sender: String,
    pub receiver: String,
    pub content: String,
    pub sent_at: NaiveDateTime,
    pub read_at: Option<NaiveDateTime>,
}

use diesel::dsl::{And, AsSelect, Eq, Filter, Gt, Or, Order, Select};

type All<DB> = Order<Select<schema::messages::table, AsSelect<Message, DB>>, schema::messages::id>;
type Between<'a, DB> = Filter<All<DB>, Or<And<From<'a>, To<'a>>, And<From<'a>, To<'a>>>>;
type After<'a, DB> = Filter<Between<'a, DB>, Gt<schema::messages::id, u64>>;

impl Message {
    pub fn all<DB: Backend>() -> All<DB> {
        schema::messages::table
            .select(Self::as_select())
            .order_by(schema::messages::id)
    }

    pub fn between<'a, DB: Backend>(peers: (&'a str, &'a str)) -> Between<'a, DB> {
        Self::all().filter(
            from(peers.0)
                .and(to(peers.0))
                .or(from(peers.1).and(to(peers.0))),
        )
    }

    pub fn between_after<'a, DB: Backend>(peers: (&'a str, &'a str), id: u64) -> After<'a, DB> {
        Self::between(peers).filter(schema::messages::id.gt(id))
    }
}

type To<'a> = Eq<schema::messages::sender, &'a str>;
type From<'a> = Eq<schema::messages::receiver, &'a str>;

pub fn from(sender: &str) -> From<'_> {
    schema::messages::receiver.eq(sender)
}

pub fn to(receiver: &str) -> To<'_> {
    schema::messages::sender.eq(receiver)
}
