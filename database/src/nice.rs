use anyhow::Result;
use chrono::TimeZone;
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::count;
use diesel::prelude::*;
use rand::prelude::*;
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
        viewer: &Viewer,
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
            bthreads.push(thread.with_lazy_posts(conn, viewer).await?);
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
        viewer: &Viewer,
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
            safeposts.push(post.safe(conn, viewer).await?);
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
            thread_post: tpost.safe(conn, viewer).await?,
            posts: safeposts,
        })
    }
    pub async fn with_lazy_posts(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &Viewer,
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
            safeposts.push(post.safe(conn, viewer).await?);
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
            thread_post: tpost.safe(conn, viewer).await?,
            posts: safeposts,
        })
    }
}

#[derive(Queryable, Selectable, Debug, Clone, PartialEq, Eq, Hash)]
#[diesel(table_name = crate::posts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
// Field order must match the column order in the generated schema: Queryable maps
// positionally, not by name.
pub struct Post {
    pub id: i64,
    pub post_number: i64,
    pub thread: i64,
    pub board: i64,
    pub author: Option<String>,
    pub content: String,
    pub timestamp: chrono::NaiveDateTime,
    pub replies_to: Vec<i64>,
    pub actual_author: String,
    pub moderator: bool,
}

impl Post {
    pub async fn safe(
        &self,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &Viewer,
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

        let t = get_file(conn, Some(viewer), self.id).await;

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
    viewer: Option<&Viewer>,
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
        Some(f) => Some(f.info(conn, viewer).await?),
        None => None,
    };
    Ok(file)
}

pub async fn get_file_from_path(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    viewer: Option<&Viewer>,
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
        Some(f) => Some(f.info(conn, viewer).await?),
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
        viewer: Option<&Viewer>,
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
            let permitted = match viewer {
                Some(v) => check_access(conn, v, board.id).await?,
                None => false,
            };
            if !permitted {
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

/// A member row. Carries nothing identifying by design — no name, no email, no created_at.
#[derive(Queryable, Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: Uuid,
    /// Preimage for post authorship. Rotatable: changing it severs the member from everything
    /// they have already posted.
    pub author_seed: String,
    pub role: String,
}

impl Member {
    pub fn role(&self) -> Role {
        Role::from_str(&self.role)
    }

    /// Bans live in their own table, so they have to be resolved separately.
    pub fn viewer_with(&self, ban: Ban) -> Viewer {
        Viewer {
            id: self.id,
            author_seed: self.author_seed.clone(),
            role: self.role(),
            ban,
        }
    }
}

/// How far a ban reaches.
///
/// Deliberately not constructible from a bare `Vec`: an accidentally empty list must not become
/// a sitewide ban, so [`Ban::boards`] rejects that case and `Sitewide` has to be named out loud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ban {
    None,
    Sitewide,
    Boards(Vec<i64>),
}

impl Ban {
    /// Board-scoped ban. An empty list is refused rather than silently escalating.
    pub fn boards(ids: Vec<i64>) -> Result<Self> {
        if ids.is_empty() {
            anyhow::bail!("a board-scoped ban needs at least one board");
        }
        Ok(Self::Boards(ids))
    }

    pub fn is_sitewide(&self) -> bool {
        matches!(self, Self::Sitewide)
    }

    pub fn covers(&self, board: i64) -> bool {
        match self {
            Self::None => false,
            Self::Sitewide => true,
            Self::Boards(ids) => ids.contains(&board),
        }
    }
}

/// Active bans for a member, collapsed into a scope. Expired rows are ignored.
pub async fn resolve_ban(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    member: &Uuid,
) -> Result<Ban> {
    use crate::bans::dsl::*;

    let rows = bans
        .filter(member_id.eq(member))
        .filter(expires_at.is_null().or(expires_at.gt(chrono::Utc::now())))
        .select(board_id)
        .load::<Option<i64>>(conn)
        .await?;

    // A single sitewide row outranks everything else.
    if rows.iter().any(|b| b.is_none()) {
        return Ok(Ban::Sitewide);
    }
    let boards: Vec<i64> = rows.into_iter().flatten().collect();
    if boards.is_empty() {
        Ok(Ban::None)
    } else {
        Ok(Ban::Boards(boards))
    }
}

/// Site-wide role. Per-board moderation is separate — see [`PermissionLevel`].
///
/// Deliberately stored here rather than in Auth0: a mod with the Auth0 dashboard could
/// correlate login timestamps against post timestamps and deanonymise a small population.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Member,
    SiteMod,
    Admin,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::SiteMod => "site_mod",
            Self::Admin => "admin",
        }
    }
    /// Unknown values degrade to the least privileged role rather than failing a request.
    pub fn from_str(s: &str) -> Self {
        match s {
            "admin" => Self::Admin,
            "site_mod" => Self::SiteMod,
            _ => Self::Member,
        }
    }
    pub fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
    /// May review the join queue and act on reported posts.
    pub fn can_moderate_site(self) -> bool {
        matches!(self, Self::SiteMod | Self::Admin)
    }
}

/// The authenticated member behind a request, resolved once per request from the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    pub id: Uuid,
    pub author_seed: String,
    pub role: Role,
    pub ban: Ban,
}

impl Viewer {
    /// An internal caller that is not a real member — file trimming, scheduled maintenance,
    /// link-embed rendering. Sees every board, the way an admin does.
    ///
    /// This replaces the old `SUPER_SECRET_CODE` constant, which did the same job by
    /// comparing an attacker-supplied string against a value compiled into the binary.
    /// Construct it only in internal call paths, never from anything in a request.
    pub fn system() -> Self {
        Self {
            id: Uuid::nil(),
            author_seed: String::new(),
            role: Role::Admin,
            ban: Ban::None,
        }
    }

    /// Authorship tag for one of this member's posts.
    pub fn post_author_hash(&self, post_id: i64) -> String {
        crate::keys::post_author_hash(&self.author_seed, post_id)
    }
    /// This member's access-grant hash for one board.
    pub fn invite_hash(&self, board_id: i64) -> String {
        crate::keys::invite_hash(&self.id, board_id)
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

    spoiler
        .choose(&mut rand::rng())
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

/// Whether `viewer` may see `board`.
///
/// The previous version began with `if token == env!("SUPER_SECRET_CODE")`, a compile-time
/// constant that granted read access to every private board. That is deliberately gone.
pub async fn check_access(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    viewer: &Viewer,
    board: i64,
) -> Result<bool> {
    let board = {
        use crate::boards::dsl::*;
        boards.filter(id.eq(board)).first::<Board>(conn).await?
    };

    // A sitewide ban blocks everything; a board ban blocks only that board.
    if viewer.ban.covers(board.id) {
        return Ok(false);
    }

    if !board.private {
        return Ok(true);
    }

    if !permission_level(conn, board.id, viewer).await?.is_none() {
        return Ok(true);
    }

    use crate::user_tags::dsl::*;
    use diesel::dsl::{exists, select};

    let hash = viewer.invite_hash(board.id);

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
    viewer: &Viewer,
    board: i64,
) -> Result<String> {
    let permission_level = permission_level(conn, board, viewer).await?;
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
    viewer: &Viewer,
    board: i64,
) -> Result<String> {
    // Site-level only. A board moderator cannot mint more moderators for their own board,
    // which would let a single grant quietly multiply itself.
    if !viewer.role.can_moderate_site() {
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
    viewer: &Viewer,
) -> Result<PermissionLevel> {
    if viewer.ban.covers(board) {
        return Ok(PermissionLevel::None);
    }
    // The role is already on the viewer, so this no longer costs a query per call.
    if viewer.role.is_admin() {
        return Ok(PermissionLevel::Admin);
    }

    use crate::user_tags::dsl::*;
    use diesel::dsl::{exists, select};

    let hash = viewer.invite_hash(board);

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

/// One stored push subscription, still encrypted.
#[derive(Queryable, Debug, Clone)]
pub struct StoredPush {
    pub id: Uuid,
    pub member_id: Uuid,
    pub data: Vec<u8>,
}

/// A member's push subscriptions, decrypted. The row id comes back too so a subscription that
/// the push service rejects can be deleted.
///
/// Rows that fail to decrypt are skipped rather than failing the whole call: a single corrupt
/// row should not stop notifications for every other device.
pub async fn get_push_subscriptions(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    member: &Uuid,
) -> Result<Vec<(Uuid, SubscriptionData)>> {
    use crate::member_push::dsl::*;

    let rows = member_push
        .filter(member_id.eq(member))
        .load::<StoredPush>(conn)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let plain = match crate::keys::open_push(&row.data) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("skipping undecryptable push subscription {}: {e}", row.id);
                continue;
            }
        };
        match serde_json::from_slice::<SubscriptionData>(&plain) {
            Ok(sub) => out.push((row.id, sub)),
            Err(e) => log::warn!("skipping malformed push subscription {}: {e}", row.id),
        }
    }
    Ok(out)
}

/// Store a push subscription, ignoring one this member already has.
pub async fn add_push_subscription(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    member: &Uuid,
    sub: &SubscriptionData,
) -> Result<()> {
    // Ciphertext differs every time (fresh nonce), so duplicates have to be detected by
    // comparing decrypted values rather than by a unique index.
    if get_push_subscriptions(conn, member)
        .await?
        .iter()
        .any(|(_, existing)| existing == sub)
    {
        return Ok(());
    }

    use crate::member_push::dsl::*;
    let sealed = crate::keys::seal_push(&serde_json::to_vec(sub)?)?;
    diesel::insert_into(member_push)
        .values((member_id.eq(member), data.eq(sealed)))
        .execute(conn)
        .await?;
    Ok(())
}

/// Drop a subscription the push service has rejected.
pub async fn remove_push_subscription(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    row_id: &Uuid,
) -> Result<()> {
    use crate::member_push::dsl::*;
    diesel::delete(member_push.filter(id.eq(row_id)))
        .execute(conn)
        .await?;
    Ok(())
}

/// Look up a member by id. Returns `None` if the row is gone.
pub async fn get_member(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    member_id: &Uuid,
) -> Result<Option<Member>> {
    use crate::members::dsl::*;
    use diesel::result::OptionalExtension as _;

    Ok(members
        .filter(id.eq(member_id))
        .first::<Member>(conn)
        .await
        .optional()?)
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
    viewer: &Viewer,
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

    let hash = viewer.invite_hash(conf.board);

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

/// A report row.
#[derive(Queryable, Debug, Clone)]
pub struct Report {
    pub id: Uuid,
    pub post_id: i64,
    pub board_id: i64,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved_by: Option<Uuid>,
}

/// File a report against a post.
///
/// No reporter is recorded. Re-reporting a post that already has an open report is a no-op
/// rather than an error, so a pile-on neither fails loudly nor accumulates rows.
pub async fn create_report(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    post: i64,
    board: i64,
    text: &str,
) -> Result<()> {
    use crate::reports::dsl::*;
    diesel::insert_into(reports)
        .values((post_id.eq(post), board_id.eq(board), reason.eq(text)))
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;
    Ok(())
}

/// Open reports, oldest first. `boards` limits the inbox to a moderator's own boards; an empty
/// slice means no restriction (site-level).
pub async fn open_reports(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    boards: Option<&[i64]>,
) -> Result<Vec<(Report, String, i64)>> {
    use crate::reports::dsl as r;

    let mut query = r::reports.filter(r::resolved_at.is_null()).into_boxed();
    if let Some(ids) = boards {
        query = query.filter(r::board_id.eq_any(ids.to_vec()));
    }
    let rows = query.order(r::created_at.asc()).load::<Report>(conn).await?;

    // Resolve each post to something a moderator can navigate to. Reports are few, so a
    // lookup per row is fine; if the inbox ever grows this wants a join.
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let post = {
            use crate::posts::dsl as p;
            p::posts
                .filter(p::id.eq(row.post_id))
                .select(p::post_number)
                .first::<i64>(conn)
                .await
                .optional()?
        };
        let discriminator = get_board_discrim(row.board_id, conn).await?;
        if let Some(number) = post {
            out.push((row, discriminator, number));
        }
    }
    Ok(out)
}

/// Mark a report handled.
pub async fn resolve_report(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    report_id: &Uuid,
    by: &Uuid,
) -> Result<Option<i64>> {
    use crate::reports::dsl::*;
    let board = reports
        .filter(id.eq(report_id))
        .filter(resolved_at.is_null())
        .select(board_id)
        .first::<i64>(conn)
        .await
        .optional()?;

    if board.is_some() {
        diesel::update(reports.filter(id.eq(report_id)))
            .set((resolved_at.eq(chrono::Utc::now()), resolved_by.eq(by)))
            .execute(conn)
            .await?;
    }
    Ok(board)
}

/// Board ids this viewer moderates. Site-level roles return `None`, meaning "all boards".
///
/// Two queries regardless of how many boards exist: the per-board grant hashes are computed
/// locally and matched in one `IN`, rather than asking the database once per board.
pub async fn moderated_boards(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    viewer: &Viewer,
) -> Result<Option<Vec<i64>>> {
    if viewer.role.can_moderate_site() {
        return Ok(None);
    }

    let all = {
        use crate::boards::dsl as b;
        b::boards.select(b::id).load::<i64>(conn).await?
    };

    // Each board has a distinct grant hash for this member, so the lookup is by value.
    let hashes: Vec<String> = all.iter().map(|b| viewer.invite_hash(*b)).collect();

    let found = {
        use crate::user_tags::dsl as t;
        t::user_tags
            .filter(t::tag_kind.eq(UserTag::Moderator.to_string()))
            .filter(t::invite_hash.eq_any(&hashes))
            .select((t::board_id, t::invite_hash))
            .load::<(i64, Option<String>)>(conn)
            .await?
    };

    // Match the grant back to the board it was computed for, so a hash cannot be credited to
    // the wrong board.
    let mine = found
        .into_iter()
        .filter(|(board, hash)| hash.as_deref() == Some(viewer.invite_hash(*board).as_str()))
        .map(|(board, _)| board)
        .collect();

    Ok(Some(mine))
}

/// Change a board's display name and privacy.
///
/// The discriminator is deliberately not editable: it is in every URL and in
/// `user_tags.board_id`-derived grants, so renaming it would orphan links rather than move
/// them.
pub async fn update_board(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    board: i64,
    new_name: Option<&str>,
    new_private: Option<bool>,
) -> Result<()> {
    use crate::boards::dsl::*;
    if let Some(n) = new_name {
        if n.trim().is_empty() {
            anyhow::bail!("a board needs a name");
        }
        diesel::update(boards.filter(id.eq(board)))
            .set(name.eq(n.trim()))
            .execute(conn)
            .await?;
    }
    if let Some(p) = new_private {
        diesel::update(boards.filter(id.eq(board)))
            .set(private.eq(p))
            .execute(conn)
            .await?;
    }
    Ok(())
}

/// Delete a board and everything on it.
pub async fn delete_board(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    board: i64,
) -> Result<()> {
    use crate::boards::dsl::*;
    diesel::delete(boards.filter(id.eq(board)))
        .execute(conn)
        .await?;
    Ok(())
}

/// Moderator grants, per board.
///
/// Returns the grant rows, never a member roster: `user_name` is a label typed by whoever
/// issued the code and `invite_hash` is only ever shown as redeemed-or-not. There is
/// deliberately no way to go from a grant to the member holding it.
pub async fn moderator_grants(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
) -> Result<Vec<(Uuid, String, String, bool)>> {
    use crate::user_tags::dsl as t;

    let rows = t::user_tags
        .filter(t::tag_kind.eq(UserTag::Moderator.to_string()))
        .select((t::id, t::board_id, t::user_name, t::invite_hash))
        .load::<(Uuid, i64, String, Option<String>)>(conn)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (grant_id, board, label, hash) in rows {
        let discriminator = get_board_discrim(board, conn).await?;
        out.push((grant_id, discriminator, label, hash.is_some()));
    }
    Ok(out)
}

/// Revoke a moderator grant. The cascade trigger removes any invites descended from it.
pub async fn revoke_grant(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    grant_id: &Uuid,
) -> Result<Option<i64>> {
    use crate::user_tags::dsl::*;
    let board = user_tags
        .filter(id.eq(grant_id))
        .select(board_id)
        .first::<i64>(conn)
        .await
        .optional()?;

    if board.is_some() {
        diesel::delete(user_tags.filter(id.eq(grant_id)))
            .execute(conn)
            .await?;
    }
    Ok(board)
}

/// Recent moderation actions. `actor_filter` limits the view to one moderator's own entries.
pub async fn moderation_entries(
    conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    actor_filter: Option<&Uuid>,
    limit: i64,
) -> Result<Vec<(String, Option<String>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>)>>
{
    use crate::moderation_log::dsl as l;

    let mut query = l::moderation_log.into_boxed();
    if let Some(a) = actor_filter {
        query = query.filter(l::actor.eq(a));
    }
    let rows = query
        .order(l::created_at.desc())
        .limit(limit)
        .select((l::action, l::board_id, l::target, l::detail, l::created_at))
        .load::<(String, Option<i64>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>)>(conn)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (action, board, target, detail, at) in rows {
        let discriminator = match board {
            Some(b) => get_board_discrim(b, conn).await.ok(),
            None => None,
        };
        out.push((action, discriminator, target, detail, at));
    }
    Ok(out)
}
