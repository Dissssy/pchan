use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoardWithThreads {
    pub info: SafeBoard,
    pub threads: Vec<ThreadWithLazyPosts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafeBoard {
    pub name: String,
    pub discriminator: String,
    pub private: bool,
}

impl From<BoardWithThreads> for SafeBoard {
    fn from(b: BoardWithThreads) -> Self {
        // Self {
        //     name: b.name,
        //     discriminator: b.discriminator,
        //     private: b.private,
        // }
        b.info
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithPosts {
    pub board: i64,
    pub thread_post: SafePost,
    pub post_count: i64,
    pub posts: Vec<SafePost>,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadWithLazyPosts {
    pub board: i64,
    pub thread_post: SafePost,
    pub post_count: i64,
    pub posts: Vec<SafePost>,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafePost {
    pub post_number: i64,
    pub file: Option<FileInfo>,
    pub thread_post_number: i64,
    pub board_discriminator: String,
    pub author: User,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub replies: Vec<Reply>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum User {
    Anonymous,
    Named(String),
    Mod(String),
}

impl User {
    pub fn load_from(s: Option<String>, moderator: bool) -> Self {
        match s {
            Some(s) => {
                if moderator {
                    Self::Mod(s)
                } else {
                    Self::Named(s)
                }
            }
            None => Self::Anonymous,
        }
    }
}

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anonymous => write!(f, "Anonymous"),
            Self::Named(s) => write!(f, "{}", s),
            Self::Mod(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileInfo {
    pub claimed: ClaimedFileInfo,
    pub board: MicroBoardInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClaimedFileInfo {
    pub path: String,
    pub thumbnail: String,
    pub spoiler: bool,
}

// `hash` used to be here. It is sha256 of the raw bytes and was sent to every client, so
// anyone holding a copy of a file obtained elsewhere could hash it and find the exact post it
// was uploaded to. The server still stores it for duplicate detection; it is simply no longer
// published.

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MicroBoardInfo {
    pub discriminator: String,
    pub private: bool,
    #[serde(skip)]
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateBoard {
    pub discriminator: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreatePost {
    pub file: Option<CreateFile>,
    pub content: String,
    pub author: Option<String>,
    pub moderator: bool,
    /// Watch the thread this post lands in. Decided by the member's own setting, and done
    /// here rather than in a second request so a failure cannot leave the post unwatched.
    #[serde(default)]
    pub watch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateFile {
    pub id: String,
    pub spoiler: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CreateThread {
    pub post: CreatePost,
    pub topic: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub struct Reply {
    pub post_number: String,
    pub thread_post_number: Option<String>,
    pub board_discriminator: String,
    pub external: bool,
}

impl Reply {
    pub fn from_str(s: &str, board: &str, thread: &str) -> anyhow::Result<Self> {
        let s = s.trim();

        let mut split = s.split('/');
        let first = split.next();
        let second = split.next();
        let third = split.next();
        let fourth = split.next();

        match (first, second, third, fourth) {
            (Some(">>>"), Some(b), Some(n), None) => {
                let board_discriminator = b.to_owned();
                let post_number = n.parse::<i64>()?.to_string();
                Ok(Reply {
                    post_number,
                    thread_post_number: None,
                    board_discriminator,
                    external: true,
                })
            }
            _ => {
                let mut split = s.split(">>");
                let first = split.next();
                let second = split.next();
                let third = split.next();

                match (first, second, third) {
                    (Some(""), Some(n), None) => {
                        let post_number = n.parse::<i64>()?.to_string();
                        Ok(Reply {
                            post_number,
                            thread_post_number: Some(thread.to_owned()),
                            board_discriminator: board.to_owned(),
                            external: false,
                        })
                    }
                    _ => Err(anyhow!("Invalid reply format")),
                }
            }
        }
    }
    pub fn text(&self, this_thread_post_number: String) -> String {
        if self.external {
            format!(
                ">>>/{board_discriminator}/{post_number}",
                board_discriminator = self.board_discriminator,
                post_number = self.post_number
            )
        } else {
            format!(
                ">>{}{}",
                self.post_number,
                if this_thread_post_number == self.post_number {
                    " (OP)"
                } else {
                    ""
                }
            )
        }
    }
    pub fn same_board_reply_text(&self) -> String {
        format!(">>{}", self.post_number)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Banner {
    pub path: String,
    pub href: Option<String>,
}

/// What the live event stream carries. `Open` and `Resync` are control messages; the rest
/// describe something that happened.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PushMessage {
    /// The stream is connected.
    Open,
    NewPost(Arc<SafePost>),
    PostDeleted(Arc<DeletedPost>),
    /// The server cannot replay what this client missed (it was away too long, or the server
    /// restarted). Anything on screen should be reloaded.
    Resync,
    /// This member's unread notifications changed, perhaps from another tab or device.
    InboxChanged,
}

/// A watched thread with posts the member has not read, as the notification inbox lists it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadNotification {
    pub board_discriminator: String,
    pub thread_post_number: i64,
    pub topic: String,
    /// Posts above the member's read mark.
    pub unread: i64,
    /// Number of the oldest unread post, where "jump to it" should land.
    pub first_unread: i64,
    /// The newest unread post, shown as the preview.
    pub latest: SafePost,
}

/// What one of the member's tabs is doing: facts, not conclusions. The server works out from
/// the tabs which one the member is using, and how closely, so the answer keeps changing as
/// time passes without any further report. Sent as things change and repeated while visible,
/// kept only in memory, and it expires on its own if a tab disappears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    /// Random per page load. Tells one tab from another; identifies nobody.
    pub tab: String,
    /// The tab is visible. A hidden tab is not being used.
    pub visible: bool,
    /// `board/number` of the full thread view this tab is showing, if it is showing one.
    pub reading: Option<String>,
    /// Seconds since the member last touched this tab (mouse, keys, scrolling, touch).
    pub quiet_secs: u32,
}

/// How far a member has read in a thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadMark {
    pub through: i64,
}

/// A post, or a whole thread, that no longer exists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeletedPost {
    pub board_discriminator: String,
    pub thread_post_number: i64,
    pub post_number: i64,
    /// The deleted post was the thread's first, so the thread is gone with it.
    pub thread_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionData {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionKeys {
    pub auth: String,
    pub p256dh: String,
}

/// What the frontend is told about the current viewer.
///
/// Carries no identity — not a name, not an email, not an Auth0 subject. Enough to decide
/// which controls to render and nothing more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeInfo {
    /// "member", "site_mod" or "admin".
    pub role: String,
    pub banned: bool,
    /// Whether this viewer moderates at least one board. Site roles imply it. Without this
    /// the UI cannot tell a board moderator from an ordinary member, and they would never be
    /// shown their own report inbox.
    pub moderates_any_board: bool,
    /// Boards this viewer moderates directly, by discriminator. Empty for site roles, which
    /// are not scoped to particular boards — check `can_moderate_site` first.
    pub moderated_boards: Vec<String>,
    /// A label for what this browser may remember about the member's own posts. It changes
    /// when someone else signs in or the member starts a new posting identity, and the browser
    /// drops what it remembered when it does. Meaningless to anyone else, and sent to no one
    /// else.
    pub cache_key: String,
    /// The member asked to be pushed when someone replies to one of their posts.
    pub reply_push: bool,
}

/// A post, named the way a link names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PostRef {
    pub board_discriminator: String,
    pub post_number: i64,
}

/// Replies made since a position, for a browser that was away. The same for everyone (apart
/// from leaving out the asker's own posts), so it needs no idea of who might care about which.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepliesPage {
    /// Posts that reply to other posts, oldest first, on boards the asker can see.
    pub posts: Vec<SafePost>,
    /// Where to ask from next time. Moves forward even when nothing was returned.
    pub cursor: i64,
    /// There are more waiting; ask again from `cursor`.
    pub more: bool,
}

/// Asks which of these posts the member wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoredRequest {
    pub posts: Vec<PostRef>,
}

impl MeInfo {
    pub fn can_moderate_site(&self) -> bool {
        self.role == "site_mod" || self.role == "admin"
    }
    /// Anything moderation-related to show at all.
    pub fn can_moderate_anything(&self) -> bool {
        self.can_moderate_site() || self.moderates_any_board
    }
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// A pending join request as shown to a reviewer.
///
/// Deliberately minimal: the id to act on, whatever the applicant chose to say about
/// themselves, and when. No hash, no subject, nothing that could later be correlated with a
/// member row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRequestSummary {
    pub id: String,
    pub disclosure: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Body of a join request submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessRequest {
    pub disclosure: String,
}

/// Body of a ban request.
///
/// `sitewide` is validated against the actor's own authority server-side, never trusted: a
/// board moderator may only ban from the board they moderate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanRequest {
    pub sitewide: bool,
    pub reason: Option<String>,
}

/// Body of a report submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRequest {
    pub reason: String,
}

/// An open report as shown to a moderator.
///
/// Identifies the post by board and post number — what a moderator can actually navigate to —
/// and carries no reporter, because none is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSummary {
    pub id: String,
    pub board_discriminator: String,
    pub post_number: i64,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A moderator grant, as shown in the panel.
///
/// `label` is free text typed by whoever issued the code, and `redeemed` says only whether
/// somebody has claimed it. There is deliberately no way to go from a grant to the member
/// holding it — the panel manages grants, never a roster of people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeratorGrant {
    pub id: String,
    pub board_discriminator: String,
    pub label: String,
    pub redeemed: bool,
}

/// One entry from the moderation log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub action: String,
    pub board_discriminator: Option<String>,
    pub target: Option<String>,
    pub detail: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Board settings a panel can change. The discriminator is not editable: it appears in every
/// URL and in derived access grants, so changing it would orphan them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardUpdate {
    pub name: Option<String>,
    pub private: Option<bool>,
}

/// A banner or spoiler image as shown in the admin panel.
///
/// `boards` is a list of discriminators. Empty means the asset applies to every board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSummary {
    pub id: i64,
    pub img_path: String,
    /// Banners only. `None` means the banner is not a link.
    pub href: Option<String>,
    pub boards: Vec<String>,
}

/// Full replacement of an asset's settings; a form submits all of it at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetUpdate {
    /// `None` clears the link.
    pub href: Option<String>,
    /// Empty means every board.
    pub boards: Vec<String>,
}

/// Exact dimensions assets must have, enforced on upload so the layout cannot be broken by a
/// mis-sized image.
pub const SPOILER_SIZE: (u32, u32) = (80, 80);
pub const BANNER_SIZE: (u32, u32) = (1200, 200);
