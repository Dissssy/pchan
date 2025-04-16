// @generated automatically by Diesel CLI.

diesel::table! {
    banners (id) {
        id -> BigInt,
        img_path -> Text,
        href -> Nullable<Text>,
        boards -> Nullable<Array<BigInt>>,
    }
}

diesel::table! {
    boards (id) {
        id -> BigInt,
        name -> Text,
        discriminator -> Text,
        post_count -> BigInt,
        private -> Bool,
    }
}

diesel::table! {
    config (key) {
        key -> Text,
        value -> Text,
    }
}

diesel::table! {
    files (id) {
        id -> BigInt,
        filepath -> Text,
        hash -> Text,
        spoiler -> Bool,
    }
}

diesel::table! {
    members (id) {
        id -> BigInt,
        token_hash -> Text,
        push_data -> Jsonb,
        watching -> Array<BigInt>,
        admin -> Bool,
    }
}

diesel::table! {
    posts (id) {
        id -> BigInt,
        moderator -> Bool,
        post_number -> BigInt,
        thread -> BigInt,
        board -> BigInt,
        author -> Nullable<Text>,
        actual_author -> Text,
        content -> Text,
        timestamp -> Timestamp,
        replies_to -> Array<BigInt>,
    }
}

diesel::table! {
    spoilers (id) {
        id -> BigInt,
        img_path -> Text,
        boards -> Array<BigInt>,
    }
}

diesel::table! {
    threads (id) {
        id -> BigInt,
        board -> BigInt,
        post_id -> BigInt,
        latest_post -> BigInt,
        topic -> Text,
    }
}

diesel::table! {
    user_tags (id) {
        id -> Uuid,
        board_id -> BigInt,
        user_name -> Text,
        invite_hash -> Nullable<Text>,
        tag_kind -> Text,
        generated_by -> Text,
    }
}

diesel::joinable!(files -> posts (id));
diesel::joinable!(posts -> boards (board));
diesel::joinable!(posts -> threads (thread));
diesel::joinable!(threads -> boards (board));

diesel::allow_tables_to_appear_in_same_query!(
    banners, boards, config, files, members, posts, spoilers, threads,
);
