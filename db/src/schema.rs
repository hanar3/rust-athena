table! {
    users (id) {
        id -> BigInt,
        username -> Varchar,
        password_hash -> Varchar,
        email -> Varchar,
        sex -> Unsigned<TinyInt>,
    }
}

table! {
    characters (id) {
        id -> Unsigned<Integer>,
        account_id -> BigInt,
        slot -> Unsigned<TinyInt>,
        name -> Varchar,
        job -> SmallInt,
        hair -> SmallInt,
        hair_color -> SmallInt,
        sex -> Unsigned<TinyInt>,
        map_name -> Varchar,
        last_x -> Nullable<Unsigned<SmallInt>>,
        last_y -> Nullable<Unsigned<SmallInt>>,
    }
}
