pub mod schema;

use chrono::NaiveDateTime;
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
