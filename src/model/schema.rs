// @generated automatically by Diesel CLI.

diesel::table! {
    messages (id) {
        id -> Unsigned<Bigint>,
        #[max_length = 64]
        sender -> Varchar,
        #[max_length = 64]
        receiver -> Varchar,
        #[max_length = 1024]
        content -> Varchar,
        sent_at -> Timestamp,
        read_at -> Nullable<Timestamp>,
    }
}
