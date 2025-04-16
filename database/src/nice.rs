use anyhow::Result;
use chrono::TimeZone;
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::count;
use diesel::prelude::*;
use uuid::Uuid;
// use diesel::prelude::QueryableByName;
// use diesel::query_dsl::methods::SelectDsl;
// use diesel::Selectable;
// use diesel::{
//     query_dsl::methods::{FilterDsl, LimitDsl, OrderDsl},
//     ExpressionMethods, Queryable,
// };
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};

use common::structs::SubscriptionData;

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Board {
    pub id: i64,
    pub name: String,
    pub discriminator: String,
    pub post_count: i64,
    pub private: bool,
}

impl Board {
    pub async fn with_threads(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: &str,
    ) -> Result<BoardWithThreads> {
        use crate::threads::dsl::*;
        let mut bthreads = Vec::new();
        for thread in threads
            .filter(board.eq(self.id))
            .order(latest_post.desc())
            .load::<Thread>(conn)
            .await?
            .iter()
        {
            bthreads.push(thread.with_lazy_posts(conn, token).await?);
        }
        Ok(BoardWithThreads {
            info: SafeBoard {
                name: self.name.clone(),
                discriminator: self.discriminator.clone(),
                private: self.private,
            },
            threads: bthreads,
        })
    }

    pub fn safe(&self) -> SafeBoard {
        SafeBoard {
            name: self.name.clone(),
            discriminator: self.discriminator.clone(),
            private: self.private,
        }
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Thread {
    pub id: i64,
    pub board: i64,
    pub post_id: i64,
    pub latest_post: i64,
    pub topic: String,
}

impl Thread {
    pub async fn with_posts(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: &str,
    ) -> Result<ThreadWithPosts> {
        use crate::posts::dsl::*;
        let tposts = posts
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .order(post_number.asc())
            .load::<Post>(conn)
            .await?;
        let mut safeposts = Vec::new();
        for post in tposts.iter() {
            safeposts.push(post.safe(conn, token).await?);
        }
        let post_count = posts
            .select(count(id))
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .first::<i64>(conn)
            .await?;
        let tpost = posts
            .filter(id.eq(self.post_id))
            .first::<Post>(conn)
            .await?;
        Ok(ThreadWithPosts {
            board: self.board,
            post_count,
            topic: self.topic.clone(),
            thread_post: tpost.safe(conn, token).await?,
            posts: safeposts,
        })
    }
    pub async fn with_lazy_posts(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: &str,
    ) -> Result<ThreadWithLazyPosts> {
        use crate::posts::dsl::*;
        let tposts: Vec<Post> = posts
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .order(post_number.desc())
            .limit(5)
            .load::<Post>(conn)
            .await?;
        let post_count = posts
            .select(count(id))
            .filter(thread.eq(self.id))
            .filter(id.ne(self.post_id))
            .first::<i64>(conn)
            .await?;
        let mut safeposts = Vec::new();
        for post in tposts.iter() {
            safeposts.push(post.safe(conn, token).await?);
        }
        safeposts.reverse();
        let tpost = posts
            .filter(id.eq(self.post_id))
            .first::<Post>(conn)
            .await?;
        Ok(ThreadWithLazyPosts {
            board: self.board,
            post_count,
            topic: self.topic.clone(),
            thread_post: tpost.safe(conn, token).await?,
            posts: safeposts,
        })
    }
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq, Eq, Hash)]
#[diesel(table_name = crate::posts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Post {
    pub id: i64,
    pub moderator: bool,
    pub post_number: i64,
    pub thread: i64,
    pub board: i64,
    pub author: Option<String>,
    pub actual_author: String,
    pub content: String,
    pub timestamp: chrono::NaiveDateTime,
    pub replies_to: Vec<i64>,
}

impl Post {
    pub async fn safe(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: &str,
    ) -> Result<SafePost> {
        use crate::posts::dsl::*;
        let replies = posts
            .select(Self::as_select())
            .load::<Self>(conn)
            .await?
            .iter()
            .flat_map(|p| p.replies_to.contains(&self.id).then_some(p.id))
            .collect::<Vec<i64>>();

        let mut newreplies = Vec::new();
        for reply in replies {
            newreplies.push(get_reply_info(reply, self.board, conn).await?);
        }

        let t = get_file(conn, Some(token.to_owned()), self.id).await;

        let board_discrim = get_board_discrim(self.board, conn).await?;

        Ok(SafePost {
            post_number: self.post_number,
            file: t?,
            thread_post_number: thread_post_number(self.thread, conn).await?,
            board_discriminator: board_discrim,
            author: User::load_from(self.author.clone(), self.moderator),
            content: self.content.clone(),
            timestamp: TimeZone::from_utc_datetime(&chrono::Utc, &self.timestamp),
            replies: newreplies,
        })
    }
}

pub async fn get_reply_info(
    tid: i64,
    og_board: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<Reply> {
    use crate::posts::dsl::*;
    let post = posts.filter(id.eq(tid)).first::<Post>(conn).await?;
    let external = post.board != og_board;

    let thisboard = get_board_discrim(post.board, conn).await?;

    let post_thread = thread_post_number(post.thread, conn).await?;

    Ok(Reply {
        post_number: post.post_number.to_string(),
        thread_post_number: Some(post_thread.to_string()),
        board_discriminator: thisboard,
        external,
    })
}

pub async fn get_board_discrim(
    board: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<String> {
    use crate::boards::dsl::*;
    let board = boards.filter(id.eq(board)).first::<Board>(conn).await?;
    Ok(board.discriminator)
}

pub async fn thread_post_number(
    thread: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<i64> {
    use crate::threads::dsl::*;
    let thread = threads.filter(id.eq(thread)).first::<Thread>(conn).await?;
    let post = post_number(thread.post_id, conn).await?;
    Ok(post)
}

pub async fn post_number(
    post: i64,
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<i64> {
    use crate::posts::dsl::*;
    let post = posts.filter(id.eq(post)).first::<Post>(conn).await?;
    Ok(post.post_number)
}

pub async fn get_file(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    token: Option<String>,
    tid: i64,
) -> Result<Option<FileInfo>> {
    use crate::files::dsl::*;
    use diesel::result::OptionalExtension;

    let file = match files
        .filter(id.eq(tid))
        .first::<File>(conn)
        .await
        .optional()?
    {
        Some(f) => Some(f.info(conn, token).await?),
        None => None,
    };
    Ok(file)
}

pub async fn get_file_from_path(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    token: Option<String>,
    path: &str,
) -> Result<Option<FileInfo>> {
    use crate::files::dsl::*;
    use diesel::result::OptionalExtension;

    let file = match files
        .filter(filepath.eq(path))
        .first::<File>(conn)
        .await
        .optional()?
    {
        Some(f) => Some(f.info(conn, token).await?),
        None => None,
    };
    Ok(file)
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct File {
    pub id: i64,
    pub filepath: String,
    pub hash: String,
    pub spoiler: bool,
}

impl File {
    pub async fn info(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: Option<String>,
    ) -> Result<FileInfo> {
        // file id is shared with the post id, so we can work our way up the chain to find whether the board is private
        let post = crate::posts::dsl::posts
            .filter(crate::posts::dsl::id.eq(self.id))
            .first::<Post>(conn)
            .await?;

        let board = crate::boards::dsl::boards
            .filter(crate::boards::dsl::id.eq(post.board))
            .first::<Board>(conn)
            .await?;

        if board.private {
            // ensure the user has access to this board
            if !match token {
                Some(t) => check_access(conn, &t, board.id).await?,
                None => {
                    log::trace!("No token provided");
                    false
                }
            } {
                return Err(anyhow::anyhow!("Not authorized to view this file"));
            }
        }

        let thumbnail = if self.spoiler {
            get_random_spoiler(conn, board.id).await?
        } else {
            format!("{}-thumb.jpg", self.filepath)
        };

        Ok(FileInfo {
            claimed: ClaimedFileInfo {
                path: self.filepath.clone(),
                thumbnail,
                hash: self.hash.clone(),
                spoiler: self.spoiler,
            },
            board: MicroBoardInfo {
                discriminator: board.discriminator,
                private: board.private,
                id: board.id,
            },
        })
    }
    // pub fn raw_info(&self) -> FileInfo {
    //     let thumbnail = format!("{}-thumb.jpg", self.filepath);
    //     FileInfo {
    //         path: self.filepath.clone(),
    //         thumbnail,
    //         hash: self.hash.clone(),
    //         spoiler: self.spoiler,
    //         board: None,
    //     }
    // }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Banner {
    pub id: i64,
    pub img_path: String,
    pub href: Option<String>,
    pub boards: Option<Vec<i64>>,
}

impl Banner {
    pub fn safe(&self) -> common::structs::Banner {
        common::structs::Banner {
            path: self.img_path.clone(),
            href: self.href.clone(),
        }
    }
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Spoiler {
    pub id: i64,
    pub img_path: String,
    pub boards: Vec<i64>,
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: i64,
    pub token_hash: String,
    pub push_data: serde_json::Value,
    pub watching: Vec<i64>,
    pub admin: bool,
}

impl Member {
    pub fn parse_push_data(&self) -> Result<Vec<SubscriptionData>> {
        let data = serde_json::from_value(self.push_data.clone())?;
        Ok(data)
    }
}

pub async fn get_random_spoiler(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    board: i64,
) -> Result<String> {
    use crate::spoilers::dsl::*;

    // let spoiler = spoilers.select(img).load::<String>(conn).await?;

    let spoiler = spoilers
        .or_filter(boards.contains(vec![board]))
        .or_filter(boards.is_null())
        .select(img_path)
        .load::<String>(conn)
        .await?;

    use rand::seq::SliceRandom;
    spoiler
        .choose(&mut rand::thread_rng())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No spoilers found for board {}!", board))

    // spoiler
    //     .choose(&mut rand::thread_rng())
    //     .cloned()
    //     .ok_or_else(|| anyhow::anyhow!("No spoilers found!"))
}

#[derive(Queryable, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoardAccess {
    id: Uuid,
    board_id: i64,
    invite_name: String,
    invite_hash: Option<String>,
    tag_kind: String,
    generated_by: String,
}

pub async fn check_access(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    token: &str,
    board: i64,
) -> Result<bool> {
    if token == env!("SUPER_SECRET_CODE") {
        return Ok(true);
    }
    log::trace!("{}, {}", board, token);

    let board = {
        use crate::boards::dsl::*;
        boards.filter(id.eq(board)).first::<Board>(conn).await?
    };

    use crate::user_tags::dsl::*;
    if !board.private {
        return Ok(true);
    }

    // let permission_level = Self::is_admin(conn, token.clone(), board.id).await?;
    let permission_level = permission_level(conn, board.id, token).await?;
    if !permission_level.is_none() {
        return Ok(true);
    }

    let hash = common::hash_invitation(token, board.id);

    use diesel::dsl::{exists, select};

    let exists = select(exists(
        user_tags.filter(
            invite_hash
                .eq(Some(hash))
                .and(board_id.eq(board.id))
                .and(tag_kind.eq(UserTag::BoardAccess.to_string())),
        ),
    ))
    .get_result(conn)
    .await?;

    Ok(exists)
}

pub async fn create_access(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    name: &str,
    token: &str,
    board: i64,
) -> Result<String> {
    let permission_level = permission_level(conn, board, token).await?;
    let generated_by_val = match permission_level {
        PermissionLevel::None => {
            return Err(anyhow::anyhow!("Not authorized to generate access code"));
        }
        PermissionLevel::Moderator(generated_by_val) => generated_by_val,
        PermissionLevel::Admin => "ADMIN".to_string(),
    };

    {
        use crate::boards::dsl::*;
        use diesel::dsl::{exists, select};
        if !select(exists(boards.filter(id.eq(board)).filter(private.eq(true))))
            .get_result(conn)
            .await?
        {
            return Err(anyhow::anyhow!("Board not found or isn't private"));
        }
    }

    use crate::user_tags::dsl::*;

    if name.is_empty() {
        return Err(anyhow::anyhow!("Name cannot be empty"));
    }

    let r_id = diesel::insert_into(user_tags)
        .values((
            user_name.eq(name),
            board_id.eq(board),
            tag_kind.eq(UserTag::BoardAccess.to_string()),
            generated_by.eq(generated_by_val),
        ))
        .returning(id)
        .get_result::<Uuid>(conn)
        .await?;

    let conf = BoardAccessConfirmation { id: r_id, board };

    Ok(conf.into_code())
}

pub async fn create_moderation(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    name: &str,
    token: &str,
    board: i64,
) -> Result<String> {
    let permission_level = permission_level(conn, board, token).await?;
    if !permission_level.is_admin() {
        return Err(anyhow::anyhow!("Not authorized to generate moderator code"));
    }

    {
        use crate::boards::dsl::*;
        use diesel::dsl::{exists, select};
        if !select(exists(boards.filter(id.eq(board))))
            .get_result(conn)
            .await?
        {
            return Err(anyhow::anyhow!("Board not found"));
        }
    }

    use crate::user_tags::dsl::*;

    if name.is_empty() {
        return Err(anyhow::anyhow!("Name cannot be empty"));
    }

    let r_id = diesel::insert_into(user_tags)
        .values((
            user_name.eq(name),
            board_id.eq(board),
            tag_kind.eq(UserTag::Moderator.to_string()),
            generated_by.eq("ADMIN"),
        ))
        .returning(id)
        .get_result::<Uuid>(conn)
        .await?;

    let conf = BoardAccessConfirmation { id: r_id, board };

    Ok(conf.into_code())
}

pub async fn permission_level(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    board: i64,
    token: &str,
) -> Result<PermissionLevel> {
    if check_admin(conn, token).await? {
        return Ok(PermissionLevel::Admin);
    }

    use crate::user_tags::dsl::*;
    use diesel::dsl::{exists, select};

    let hash = common::hash_invitation(token, board);

    let exists = select(exists(
        user_tags.filter(
            invite_hash
                .eq(Some(&hash))
                .and(board_id.eq(board))
                .and(tag_kind.eq(UserTag::Moderator.to_string())),
        ),
    ))
    .get_result(conn)
    .await?;

    if exists {
        return Ok(PermissionLevel::Moderator(hash));
    }

    Ok(PermissionLevel::None)
}

pub enum PermissionLevel {
    None,
    Moderator(String),
    Admin,
}

impl PermissionLevel {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    pub fn is_moderator(&self) -> bool {
        matches!(self, Self::Moderator(_))
    }
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

pub async fn check_admin(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    token: &str,
) -> Result<bool> {
    use crate::members::dsl::*;
    use diesel::dsl::{exists, select};

    let res = select(exists(
        members.filter(token_hash.eq(token)).filter(admin.eq(true)),
    ))
    .get_result(conn)
    .await?;

    Ok(res)
}

pub enum UserTag {
    BoardAccess,
    Moderator,
}

impl UserTag {
    pub fn to_string(&self) -> &'static str {
        match self {
            Self::BoardAccess => "INVITE",
            Self::Moderator => "MODERATOR",
        }
    }
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "INVITE" => Some(Self::BoardAccess),
            "MODERATOR" => Some(Self::Moderator),
            _ => None,
        }
    }
}

pub async fn grant_access(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    code: &str,
    token: &str,
) -> Result<()> {
    use crate::user_tags::dsl::*;
    use diesel::dsl::{exists, select};

    let conf = BoardAccessConfirmation::from_code(code)?;

    // ensure the invite exists
    if !select(exists(user_tags.filter(id.eq(conf.id))))
        .get_result(conn)
        .await?
    {
        return Err(anyhow::anyhow!("Invite not found"));
    }

    // get this invite record, filter out ones where invite_hash is null
    let this = match user_tags
        .filter(id.eq(conf.id))
        .filter(invite_hash.is_null())
        .first::<BoardAccess>(conn)
        .await
        .optional()?
    {
        Some(t) => t,
        None => {
            return Err(anyhow::anyhow!("Invite already accepted"));
        }
    };
    // if select(exists(
    //     user_tags
    //         .filter(id.eq(conf.id))
    //         .filter(invite_hash.is_not_null()),
    // ))
    // .get_result(conn)
    // .await?
    // {
    //     return Err(anyhow::anyhow!("Invite already accepted"));
    // }

    let hash = common::hash_invitation(token, conf.board);

    // get an existing user_tag with the same invite_hash
    let existing = user_tags
        .filter(invite_hash.eq(Some(&hash)))
        .first::<BoardAccess>(conn)
        .await
        .optional()?;

    if let Some(existing) = existing {
        if existing.tag_kind == UserTag::BoardAccess.to_string()
            && this.tag_kind == UserTag::Moderator.to_string()
        {
            // promote the existing tag to moderator and delete this one
            diesel::update(user_tags.filter(id.eq(existing.id)))
                .set(tag_kind.eq(UserTag::Moderator.to_string()))
                .execute(conn)
                .await?;

            diesel::delete(user_tags.filter(id.eq(conf.id)))
                .execute(conn)
                .await?;
        } else {
            return Err(anyhow::anyhow!(
                "User is already at or above this permission level"
            ));
        }
    } else {
        diesel::update(user_tags.filter(id.eq(conf.id)))
            .set(invite_hash.eq(Some(hash)))
            .execute(conn)
            .await?;
    }

    Ok(())
}

pub struct BoardAccessConfirmation {
    pub id: Uuid,
    pub board: i64,
}

// const ENCRYPTION_KEY: &[u8] = Box::leak(env!("INVITE_ENCRYPTION_KEY").as_bytes().into());
lazy_static::lazy_static! {
    static ref ENCRYPTION_KEY: Vec<u8> = env!("INVITE_ENCRYPTION_KEY").as_bytes().to_vec();
    static ref BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
}

use base64::engine::Engine;

impl BoardAccessConfirmation {
    pub fn into_code(self) -> String {
        // basically just salt and encrypt the id returning a base64 string
        let salt = "boardaccess";
        let full_string = format!("{}|{}|{}", self.id, self.board, salt);

        // using the encryption key as the key, do basic xor encryption on the string
        let mut encrypted = Vec::new();
        let mut cycled_key = ENCRYPTION_KEY.iter().cycle();
        for byte in full_string.as_bytes() {
            let key_byte = cycled_key.next().unwrap();
            encrypted.push(byte ^ key_byte);
        }

        Engine::encode(&*BASE64_ENGINE, &encrypted)
    }
    pub fn from_code(code: &str) -> Result<Self> {
        let encrypted = Engine::decode(&*BASE64_ENGINE, code.as_bytes())?;
        let mut decrypted = Vec::new();
        let mut cycled_key = ENCRYPTION_KEY.iter().cycle();
        for byte in encrypted {
            let key_byte = cycled_key.next().unwrap();
            decrypted.push(byte ^ key_byte);
        }
        let decrypted = String::from_utf8(decrypted)?;
        let mut parts = decrypted.split('|');
        let id = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("No id found"))?
            .parse()?;
        let board = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("No board_id found"))?
            .parse()?;
        let salt = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("No salt found"))?;
        if salt != "boardaccess" {
            return Err(anyhow::anyhow!("Invalid salt"));
        }
        Ok(Self { id, board })
    }
}
