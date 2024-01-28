pub mod schema;

use chrono::NaiveDateTime;
use diesel::backend::Backend;
use diesel::helper_types::Like;
use diesel::query_source::{Alias, AliasedField};
use diesel::{alias, prelude::*, Expression};

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

use diesel::dsl::{exists, not, And, AsSelect, Desc, Eq, Filter, Gt, Limit, Lt, Or, Order, Select};

type All<DB> =
    Order<Select<schema::messages::table, AsSelect<Message, DB>>, Desc<schema::messages::id>>;
type Between<'a, DB> = Filter<
    All<DB>,
    IsBetween<'a, schema::messages::columns::sender, schema::messages::columns::receiver, &'a str>,
>;

type Containing<'a, DB> =
    Limit<Filter<Between<'a, DB>, Like<schema::messages::columns::content, String>>>;
type ContainingAfter<'a, DB> = Filter<Containing<'a, DB>, Gt<schema::messages::columns::id, u64>>;
type ContainingBefore<'a, DB> = Filter<Containing<'a, DB>, Lt<schema::messages::columns::id, u64>>;

type After<'a, DB> = Filter<Between<'a, DB>, Gt<schema::messages::id, u64>>;
type Before<'a, DB> = Filter<Between<'a, DB>, Lt<schema::messages::id, u64>>;

type AfterLimited<'a, DB> = Limit<After<'a, DB>>;
type BeforeLimited<'a, DB> = Limit<Before<'a, DB>>;

type Limited<'a, DB> = Limit<Between<'a, DB>>;

impl Message {
    pub fn all<DB: Backend>() -> All<DB> {
        schema::messages::table
            .select(Self::as_select())
            .order_by(schema::messages::id.desc())
    }

    pub fn between<'a, DB: Backend>((peer1, peer2): (&'a str, &'a str)) -> Between<'a, DB> {
        Self::all().filter(is_between(
            (peer1, peer2.into_sql::<Text>()),
            (schema::messages::sender, schema::messages::receiver),
        ))
    }

    pub fn limited<'a, DB: Backend>(peers: (&'a str, &'a str), limit: usize) -> Limited<'a, DB> {
        Self::between(peers).limit(limit as i64)
    }

    pub fn after<'a, DB: Backend>(peers: (&'a str, &'a str), id: u64) -> After<'a, DB> {
        Self::between(peers).filter(schema::messages::id.gt(id))
    }

    pub fn before<'a, DB: Backend>(peers: (&'a str, &'a str), id: u64) -> Before<'a, DB> {
        Self::between(peers).filter(schema::messages::id.lt(id))
    }

    pub fn after_limited<'a, DB: Backend>(
        peers: (&'a str, &'a str),
        id: u64,
        limit: usize,
    ) -> AfterLimited<'a, DB> {
        Self::after(peers, id).limit(limit as i64)
    }

    pub fn before_limited<'a, DB: Backend>(
        peers: (&'a str, &'a str),
        id: u64,
        limit: usize,
    ) -> BeforeLimited<'a, DB> {
        Self::before(peers, id).limit(limit as i64)
    }

    pub fn is_between(&self, (peer1, peer2): (&str, &str)) -> bool {
        (self.sender == peer1 && self.receiver == peer2)
            || (self.sender == peer2 && self.receiver == peer1)
    }

    pub fn like<'a, DB: Backend>(peers: (&'a str, &'a str), like: &'a str) -> Containing<'a, DB> {
        Self::between(peers)
            .filter(schema::messages::content.like(format!("%{like}%")))
            .limit(1)
    }

    pub fn like_before<'a, DB: Backend>(
        peers: (&'a str, &'a str),
        like: &'a str,
        id: u64,
    ) -> ContainingBefore<'a, DB> {
        Self::like(peers, like).filter(schema::messages::id.lt(id))
    }
}

type IsBetween<'a, C1, C2, E> =
    Or<And<Eq<C1, &'a str>, Eq<C2, E>>, And<Eq<C1, E>, Eq<C2, &'a str>>>;

use diesel::expression_methods::ExpressionMethods;
use diesel::sql_types::Text;

fn is_between<C1, C2, E>(peers: (&str, E), columns: (C1, C2)) -> IsBetween<'_, C1, C2, E>
where
    C1: Expression<SqlType = Text> + Clone,
    C2: Expression<SqlType = Text> + Clone,
    E: Expression<SqlType = Text> + Clone,
{
    columns
        .0
        .clone()
        .eq(peers.0)
        .and(columns.1.clone().eq(peers.1.clone()))
        .or(columns.0.eq(peers.1).and(columns.1.eq(peers.0)))
}

alias! {
    pub const LATER_MESSAGES: Alias<LaterMessages> = schema::messages as later_messages;
}

pub type LaterMessagesWithSender<'a> = Filter<
    Alias<LaterMessages>,
    And<
        IsBetween<
            'a,
            AliasedField<LaterMessages, schema::messages::columns::sender>,
            AliasedField<LaterMessages, schema::messages::columns::receiver>,
            schema::messages::columns::sender,
        >,
        Gt<AliasedField<LaterMessages, schema::messages::columns::id>, schema::messages::id>,
    >,
>;

fn later_messages_with_sender(sender: &str) -> LaterMessagesWithSender<'_> {
    LATER_MESSAGES.filter(
        is_between(
            (sender, schema::messages::sender),
            (
                LATER_MESSAGES.field(schema::messages::sender),
                LATER_MESSAGES.field(schema::messages::receiver),
            ),
        )
        .and(
            LATER_MESSAGES
                .field(schema::messages::id)
                .gt(schema::messages::id),
        ),
    )
}

pub type LaterMessagesWithReceiver<'a> = Filter<
    Alias<LaterMessages>,
    And<
        IsBetween<
            'a,
            AliasedField<LaterMessages, schema::messages::columns::sender>,
            AliasedField<LaterMessages, schema::messages::columns::receiver>,
            schema::messages::columns::receiver,
        >,
        Gt<AliasedField<LaterMessages, schema::messages::columns::id>, schema::messages::id>,
    >,
>;

fn later_messages_with_receiver(sender: &str) -> LaterMessagesWithReceiver<'_> {
    LATER_MESSAGES.filter(
        is_between(
            (sender, schema::messages::receiver),
            (
                LATER_MESSAGES.field(schema::messages::sender),
                LATER_MESSAGES.field(schema::messages::receiver),
            ),
        )
        .and(
            LATER_MESSAGES
                .field(schema::messages::id)
                .gt(schema::messages::id),
        ),
    )
}

pub type SenderAndNoLaterMessagesWithReceiver<'a> =
    And<Eq<schema::messages::columns::sender, &'a str>, not<exists<LaterMessagesWithReceiver<'a>>>>;

pub type ReceiverAndNoLaterMessagesWithSender<'a> =
    And<Eq<schema::messages::columns::receiver, &'a str>, not<exists<LaterMessagesWithSender<'a>>>>;

pub type MostRecentMessages<'a, DB> = Filter<
    All<DB>,
    Or<SenderAndNoLaterMessagesWithReceiver<'a>, ReceiverAndNoLaterMessagesWithSender<'a>>,
>;

impl Message {
    pub fn most_recent<DB: Backend>(user: &str) -> MostRecentMessages<'_, DB> {
        Self::all().filter(
            schema::messages::sender
                .eq(user)
                .and(not(exists(later_messages_with_receiver(user))))
                .or(schema::messages::receiver
                    .eq(user)
                    .and(not(exists(later_messages_with_sender(user))))),
        )
    }
}
