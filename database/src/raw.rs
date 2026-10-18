// @generated automatically by Diesel CLI.

diesel::table! {
    banners (id) {
        id -> Int8,
        img_path -> Text,
        href -> Nullable<Text>,
        boards -> Nullable<Array<Int8>>,
    }
}

diesel::table! {
    boards (id) {
        id -> Int8,
        name -> Text,
        discriminator -> Text,
        post_count -> Int8,
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
        id -> Int8,
        filepath -> Text,
        hash -> Text,
        spoiler -> Bool,
    }
}

diesel::table! {
    identities (identity_hash) {
        identity_hash -> Text,
        member_id -> Uuid,
    }
}

diesel::table! {
    join_requests (id) {
        id -> Uuid,
        request_hash -> Text,
        disclosure -> Text,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    member_push (id) {
        id -> Uuid,
        member_id -> Uuid,
        data -> Bytea,
    }
}

diesel::table! {
    member_watching (member_id, thread_id) {
        member_id -> Uuid,
        thread_id -> Int8,
    }
}

diesel::table! {
    members (id) {
        id -> Uuid,
        author_seed -> Text,
        role -> Text,
        banned -> Bool,
    }
}

diesel::table! {
    posts (id) {
        id -> Int8,
        post_number -> Int8,
        thread -> Int8,
        board -> Int8,
        author -> Nullable<Text>,
        content -> Text,
        timestamp -> Timestamp,
        replies_to -> Array<Int8>,
        actual_author -> Text,
        moderator -> Bool,
    }
}

diesel::table! {
    request_blocks (request_hash) {
        request_hash -> Text,
        until -> Timestamptz,
    }
}

diesel::table! {
    sessions (id) {
        id -> Uuid,
        token_hash -> Text,
        member_id -> Uuid,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
        last_seen -> Timestamptz,
    }
}

diesel::table! {
    spoilers (id) {
        id -> Int8,
        img_path -> Text,
        boards -> Array<Int8>,
    }
}

diesel::table! {
    threads (id) {
        id -> Int8,
        board -> Int8,
        post_id -> Int8,
        latest_post -> Int8,
        topic -> Text,
    }
}

diesel::table! {
    user_tags (id) {
        id -> Uuid,
        board_id -> Int8,
        user_name -> Text,
        invite_hash -> Nullable<Text>,
        tag_kind -> Text,
        generated_by -> Text,
    }
}

diesel::joinable!(files -> posts (id));
diesel::joinable!(identities -> members (member_id));
diesel::joinable!(member_push -> members (member_id));
diesel::joinable!(member_watching -> members (member_id));
diesel::joinable!(member_watching -> threads (thread_id));
diesel::joinable!(posts -> boards (board));
diesel::joinable!(posts -> threads (thread));
diesel::joinable!(sessions -> members (member_id));
diesel::joinable!(threads -> boards (board));
diesel::joinable!(user_tags -> boards (board_id));

diesel::allow_tables_to_appear_in_same_query!(
    banners,
    boards,
    config,
    files,
    identities,
    join_requests,
    member_push,
    member_watching,
    members,
    posts,
    request_blocks,
    sessions,
    spoilers,
    threads,
    user_tags,
);
