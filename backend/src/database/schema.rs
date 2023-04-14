diesel::table! {
    boards (id) {
        id -> BigInt,
        name -> Text,
        descriminator -> Text,
    }
}

diesel::table! {
    threads (id) {
        id -> BigInt,
        board -> BigInt,
        post -> BigInt,
    }
}

diesel::table! {
    posts (id) {
        id -> BigInt,
        post_number -> BigInt,
        image -> Nullable<Array<Text>>,
        board -> BigInt,
        parent -> BigInt,
        author -> Nullable<Text>,
        content -> Text,
        posted -> Time,
        replies_to -> Array<BigInt>,
    }
}
