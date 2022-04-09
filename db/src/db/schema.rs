table! {
    users (id) {
        id -> Binary,
        username -> Varchar,
        password_hash -> Varchar,
        email -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Datetime>,
    }
}
