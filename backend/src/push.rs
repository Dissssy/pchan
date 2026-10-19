//! The live event hub: one server-sent-event stream per browser tab.
//!
//! Every event gets an id and is kept in a short in-memory log, so a client that drops and
//! reconnects can be told what it missed instead of silently losing it. Delivery to each
//! connection goes through a bounded queue; a connection that cannot keep up is closed and
//! recovers the same way a dropped one does.
//!
//! Nothing here is persisted. The log holds member ids only for as long as the process runs.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use async_stream::stream;
use common::structs::PushMessage;
use futures::Stream;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::sse::Event;

/// Events kept for replay. A reconnect older than this is told to reload instead.
const LOG_CAPACITY: usize = 500;
/// Events one connection may have waiting before it is considered stuck.
const QUEUE_CAPACITY: usize = 64;

/// A thread, as the URL names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadKey {
    pub board: String,
    pub number: i64,
}

impl ThreadKey {
    /// Parse `board/number`.
    pub fn parse(s: &str) -> Option<Self> {
        let (board, number) = s.rsplit_once('/')?;
        if board.is_empty() {
            return None;
        }
        Some(Self {
            board: board.to_owned(),
            number: number.parse().ok()?,
        })
    }
}

/// One thing to send down a connection.
#[derive(Debug, Clone)]
pub struct Delivery {
    /// `None` for control messages, which must not move the client's resume point.
    pub id: Option<String>,
    pub message: PushMessage,
}

struct Connection {
    member: Uuid,
    /// Hash of the session cookie that opened this stream, so logging that session out can
    /// close exactly its streams.
    session: String,
    thread: Option<ThreadKey>,
    tx: mpsc::Sender<Delivery>,
}

struct Logged {
    id: u64,
    message: PushMessage,
    thread: Option<ThreadKey>,
    members: HashSet<Uuid>,
}

impl Connection {
    /// Whether this connection is meant to see an event about `thread` that is also addressed
    /// to `members`. A connection viewing the thread, or belonging to a watcher, gets it once.
    fn wants(&self, thread: &Option<ThreadKey>, members: &HashSet<Uuid>) -> bool {
        (thread.is_some() && self.thread == *thread) || members.contains(&self.member)
    }
}

/// A tab's last report of what the member is doing in it.
struct Viewing {
    /// The full thread view the tab is showing, if any.
    reading: Option<ThreadKey>,
    /// When the member last touched this tab, worked out from the report.
    last_interaction: Instant,
    /// When the report stops counting if no newer one arrives.
    until: Instant,
}

/// How much attention a member is paying, which decides how a new post reaches them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// Reading that very thread: it is on their screen, so nothing more is needed. No push, no
    /// notification, and the post counts as read.
    Reading,
    /// Using the site somewhere else: they will see a notification in the page, which is
    /// enough, so no push.
    Active,
    /// Not on the site at all, or idle everywhere: push.
    Away,
}

/// Longest a report counts for. Tabs repeat it more often than this, so a tab that vanishes
/// without saying goodbye stops mattering quickly.
pub const PRESENCE_TTL: Duration = Duration::from_secs(45);
/// Without touching the tab this long, a thread on screen stops counting as being read.
pub const READING_IDLE: Duration = Duration::from_secs(2 * 60);
/// Without touching any tab this long, the member stops counting as using the site.
pub const ACTIVE_IDLE: Duration = Duration::from_secs(10 * 60);
/// Reports kept per member; a client cannot grow the map by inventing tabs.
const MAX_TABS_PER_MEMBER: usize = 20;

pub struct Hub {
    presence: HashMap<(Uuid, String), Viewing>,
    boot: String,
    /// Id of the next event.
    next: u64,
    next_connection: u64,
    connections: HashMap<u64, Connection>,
    log: VecDeque<Logged>,
}

pub type SharedHub = Arc<Mutex<Hub>>;

/// The lock is only ever held for in-memory bookkeeping, never across an await, so a
/// poisoned lock means a panic mid-update of a plain data structure: keep serving.
pub fn lock(hub: &SharedHub) -> MutexGuard<'_, Hub> {
    hub.lock().unwrap_or_else(|e| e.into_inner())
}

impl Hub {
    pub fn new() -> Self {
        Self {
            presence: HashMap::new(),
            // A fresh value per process: ids from before a restart must not be mistaken for
            // ids in this one.
            boot: Uuid::new_v4().simple().to_string()[..8].to_owned(),
            next: 1,
            next_connection: 0,
            connections: HashMap::new(),
            log: VecDeque::new(),
        }
    }

    fn event_id(&self, n: u64) -> String {
        format!("{}-{}", self.boot, n)
    }

    /// Record `message` and hand it to every connection that should see it.
    pub fn publish(
        &mut self,
        message: PushMessage,
        thread: Option<ThreadKey>,
        members: HashSet<Uuid>,
    ) {
        let id = self.next;
        self.next += 1;
        let delivery = Delivery {
            id: Some(self.event_id(id)),
            message: message.clone(),
        };

        let mut stuck = Vec::new();
        for (conn_id, conn) in &self.connections {
            if conn.wants(&thread, &members) && conn.tx.try_send(delivery.clone()).is_err() {
                stuck.push(*conn_id);
            }
        }
        // Dropping the sender ends that stream; the client reconnects and replays.
        for conn_id in stuck {
            self.connections.remove(&conn_id);
        }

        self.log.push_back(Logged {
            id,
            message,
            thread,
            members,
        });
        while self.log.len() > LOG_CAPACITY {
            self.log.pop_front();
        }
    }

    /// Register a connection. Returns its id, whatever it missed since `last`, and the queue
    /// that live events arrive on. Replay and registration happen under one lock, so nothing
    /// can slip between them.
    pub fn attach(
        &mut self,
        member: Uuid,
        session: String,
        thread: Option<ThreadKey>,
        last: Option<&str>,
    ) -> (u64, Vec<Delivery>, mpsc::Receiver<Delivery>) {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
        let conn = Connection {
            member,
            session,
            thread,
            tx,
        };

        let mut replay = Vec::new();
        if let Some(last) = last {
            let resume = last
                .rsplit_once('-')
                .filter(|(boot, _)| *boot == self.boot)
                .and_then(|(_, n)| n.parse::<u64>().ok());
            // The oldest event we still hold; if the client needs something older it is gone.
            let oldest = self.log.front().map(|l| l.id).unwrap_or(self.next);
            match resume {
                Some(n) if n + 1 >= oldest && n < self.next => {
                    for logged in self.log.iter().filter(|l| l.id > n) {
                        if conn.wants(&logged.thread, &logged.members) {
                            replay.push(Delivery {
                                id: Some(self.event_id(logged.id)),
                                message: logged.message.clone(),
                            });
                        }
                    }
                }
                _ => replay.push(Delivery {
                    // Carries the current head, so the client's resume point moves forward
                    // and the next reconnect is not told to reload again.
                    id: Some(self.event_id(self.next - 1)),
                    message: PushMessage::Resync,
                }),
            }
        }

        let conn_id = self.next_connection;
        self.next_connection += 1;
        self.connections.insert(conn_id, conn);
        (conn_id, replay, rx)
    }

    pub fn detach(&mut self, conn_id: u64) {
        self.connections.remove(&conn_id);
    }

    /// Record what one of a member's tabs is doing. A tab that is not visible is forgotten.
    pub fn set_presence(
        &mut self,
        member: Uuid,
        tab: String,
        visible: bool,
        reading: Option<ThreadKey>,
        quiet: Duration,
        now: Instant,
    ) {
        self.presence.retain(|_, v| v.until > now);
        if !visible {
            self.presence.remove(&(member, tab));
            return;
        }
        let known = self.presence.contains_key(&(member, tab.clone()));
        let held = self.presence.keys().filter(|(m, _)| *m == member).count();
        if known || held < MAX_TABS_PER_MEMBER {
            self.presence.insert(
                (member, tab),
                Viewing {
                    reading,
                    // Before the clock started, if it must be; the tier only cares how long ago.
                    last_interaction: now.checked_sub(quiet).unwrap_or(now),
                    until: now + PRESENCE_TTL,
                },
            );
        }
    }

    /// How closely `member` is paying attention, as far as `thread` is concerned.
    ///
    /// There is one answer per member, taken from the tab they touched most recently: a tab
    /// they have stopped using does not keep claiming their attention while they work in
    /// another. Because it is worked out from when they last touched it, it also degrades on
    /// its own as time passes, with no report needed to say they have gone quiet.
    pub fn attention(&self, member: &Uuid, thread: &ThreadKey, now: Instant) -> Attention {
        let in_use = self
            .presence
            .iter()
            .filter(|((m, _), v)| m == member && v.until > now)
            .map(|(_, v)| v)
            .max_by_key(|v| v.last_interaction);
        let Some(tab) = in_use else {
            return Attention::Away;
        };
        let quiet = now.saturating_duration_since(tab.last_interaction);
        if quiet > ACTIVE_IDLE {
            Attention::Away
        } else if quiet <= READING_IDLE && tab.reading.as_ref() == Some(thread) {
            Attention::Reading
        } else {
            Attention::Active
        }
    }

    /// Members with at least one stream open right now.
    pub fn connected_members(&self) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = self.connections.values().map(|c| c.member).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Close every stream a member has open. They reconnect on their own, and the reconnect
    /// is checked again, so a banned or logged-out member is simply refused.
    pub fn disconnect_member(&mut self, member: &Uuid) {
        self.connections.retain(|_, c| &c.member != member);
        self.presence.retain(|(m, _), _| m != member);
    }

    /// Close the streams opened by one session.
    pub fn disconnect_session(&mut self, session: &str) {
        self.connections.retain(|_, c| c.session != session);
    }

    #[cfg(test)]
    fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

/// Removes the connection when the response stream is dropped, however it ends.
struct Detach {
    hub: SharedHub,
    id: u64,
}

impl Drop for Detach {
    fn drop(&mut self) {
        lock(&self.hub).detach(self.id);
    }
}

fn into_event(delivery: Delivery) -> Event {
    let event = match &delivery.message {
        PushMessage::Open => Ok(Event::default().event("open")),
        // An event with no data is never dispatched by the browser, so control messages that
        // carry nothing still need a body.
        PushMessage::Resync => Ok(Event::default().event("resync").data("resync")),
        PushMessage::InboxChanged => Ok(Event::default().event("inbox_changed").data("inbox")),
        PushMessage::NewPost(post) => Event::default().event("new_post").json_data(post.as_ref()),
        PushMessage::PostDeleted(gone) => Event::default()
            .event("post_deleted")
            .json_data(gone.as_ref()),
    };
    let event = match event {
        Ok(e) => e,
        Err(e) => Event::default().event("error").data(format!("{e:?}")),
    };
    match delivery.id {
        Some(id) => event.id(id),
        None => event,
    }
}

/// Open a stream for `member`.
pub fn connect(
    hub: &SharedHub,
    member: Uuid,
    session: String,
    thread: Option<ThreadKey>,
    last: Option<&str>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let (id, replay, mut rx) = lock(hub).attach(member, session, thread, last);
    let guard = Detach {
        hub: hub.clone(),
        id,
    };
    stream! {
        let _guard = guard;
        yield Ok(into_event(Delivery { id: None, message: PushMessage::Open }));
        for delivery in replay {
            yield Ok(into_event(delivery));
        }
        while let Some(delivery) = rx.recv().await {
            yield Ok(into_event(delivery));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::structs::DeletedPost;

    fn key(n: i64) -> ThreadKey {
        ThreadKey {
            board: "b".into(),
            number: n,
        }
    }

    fn gone(n: i64) -> PushMessage {
        PushMessage::PostDeleted(Arc::new(DeletedPost {
            board_discriminator: "b".into(),
            thread_post_number: n,
            post_number: n,
            thread_deleted: false,
        }))
    }

    fn drain(rx: &mut mpsc::Receiver<Delivery>) -> Vec<Delivery> {
        let mut out = Vec::new();
        while let Ok(d) = rx.try_recv() {
            out.push(d);
        }
        out
    }

    #[test]
    fn parses_thread_keys() {
        assert_eq!(ThreadKey::parse("b/12"), Some(key(12)));
        assert_eq!(ThreadKey::parse("a/b/3").unwrap().board, "a/b");
        assert_eq!(ThreadKey::parse("b/x"), None);
        assert_eq!(ThreadKey::parse("/3"), None);
        assert_eq!(ThreadKey::parse("nope"), None);
    }

    #[test]
    fn delivers_to_viewers_and_watchers_once() {
        let mut hub = Hub::new();
        let (watcher, viewer, both, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let (_, _, mut w) = hub.attach(watcher, "s1".into(), None, None);
        let (_, _, mut v) = hub.attach(viewer, "s2".into(), Some(key(1)), None);
        let (_, _, mut b) = hub.attach(both, "s3".into(), Some(key(1)), None);
        let (_, _, mut o) = hub.attach(other, "s4".into(), Some(key(2)), None);

        hub.publish(gone(1), Some(key(1)), [watcher, both].into());

        assert_eq!(drain(&mut w).len(), 1);
        assert_eq!(drain(&mut v).len(), 1);
        assert_eq!(drain(&mut b).len(), 1, "watching and viewing is still one delivery");
        assert_eq!(drain(&mut o).len(), 0);
    }

    #[test]
    fn replays_only_what_the_connection_may_see() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        hub.publish(gone(1), Some(key(1)), HashSet::new());
        hub.publish(gone(2), Some(key(2)), [me].into());
        hub.publish(gone(3), Some(key(3)), HashSet::new());

        let last = hub.event_id(1);
        let (_, replay, _) = hub.attach(me, "s".into(), Some(key(1)), Some(&last));
        let ids: Vec<_> = replay.iter().map(|d| d.id.clone().unwrap()).collect();
        assert_eq!(ids, vec![hub.event_id(2)], "event 2 is addressed to me; 3 is not");
        assert!(replay.iter().all(|d| d.message != PushMessage::Resync));
    }

    #[test]
    fn first_connection_replays_nothing() {
        let mut hub = Hub::new();
        hub.publish(gone(1), Some(key(1)), HashSet::new());
        let (_, replay, _) = hub.attach(Uuid::new_v4(), "s".into(), Some(key(1)), None);
        assert!(replay.is_empty());
    }

    #[test]
    fn unreplayable_gaps_ask_for_a_reload() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();

        // Another boot, garbage, and an id from the future.
        for bad in ["deadbeef-3", "nonsense", "x-y-z", &format!("{}-999", hub.boot)] {
            let (_, replay, _) = hub.attach(me, "s".into(), None, Some(bad));
            assert_eq!(replay.len(), 1, "{bad}");
            assert_eq!(replay[0].message, PushMessage::Resync, "{bad}");
        }

        // Older than the log reaches.
        for i in 0..(LOG_CAPACITY as i64 + 10) {
            hub.publish(gone(i), Some(key(i)), HashSet::new());
        }
        let ancient = hub.event_id(2);
        let (_, replay, _) = hub.attach(me, "s".into(), None, Some(&ancient));
        assert_eq!(replay[0].message, PushMessage::Resync);
        // And the reload moves the resume point, so it is not asked again.
        let head = replay[0].id.clone().unwrap();
        let (_, again, _) = hub.attach(me, "s".into(), None, Some(&head));
        assert!(again.is_empty());
    }

    #[test]
    fn a_stuck_connection_is_dropped_not_buffered_forever() {
        let mut hub = Hub::new();
        let (_, _, mut rx) = hub.attach(Uuid::new_v4(), "s".into(), Some(key(1)), None);
        for _ in 0..(QUEUE_CAPACITY + 1) {
            hub.publish(gone(1), Some(key(1)), HashSet::new());
        }
        assert_eq!(hub.connection_count(), 0);
        assert_eq!(drain(&mut rx).len(), QUEUE_CAPACITY);
    }

    #[test]
    fn logout_and_bans_close_streams() {
        let mut hub = Hub::new();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        hub.attach(a, "phone".into(), None, None);
        hub.attach(a, "laptop".into(), None, None);
        hub.attach(b, "other".into(), None, None);

        hub.disconnect_session("phone");
        assert_eq!(hub.connection_count(), 2);
        hub.disconnect_member(&a);
        assert_eq!(hub.connection_count(), 1);
    }

    #[test]
    fn connected_members_lists_each_member_once() {
        let mut hub = Hub::new();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        hub.attach(a, "1".into(), None, None);
        hub.attach(a, "2".into(), Some(key(1)), None);
        hub.attach(b, "3".into(), None, None);
        let mut expected = vec![a, b];
        expected.sort();
        assert_eq!(hub.connected_members(), expected);
        hub.disconnect_member(&a);
        assert_eq!(hub.connected_members(), vec![b]);
    }

    #[test]
    fn dropping_the_stream_detaches() {
        let hub: SharedHub = Arc::new(Mutex::new(Hub::new()));
        let stream = connect(&hub, Uuid::new_v4(), "s".into(), None, None);
        assert_eq!(lock(&hub).connection_count(), 1);
        drop(stream);
        // The guard lives inside the generator, which has not started: dropping it must
        // still release the registration.
        assert_eq!(lock(&hub).connection_count(), 0);
    }

    #[test]
    fn member_only_events_reach_every_tab_of_that_member() {
        let mut hub = Hub::new();
        let (me, other) = (Uuid::new_v4(), Uuid::new_v4());
        let (_, _, mut a) = hub.attach(me, "s1".into(), None, None);
        let (_, _, mut b) = hub.attach(me, "s2".into(), Some(key(1)), None);
        let (_, _, mut c) = hub.attach(other, "s3".into(), Some(key(1)), None);

        hub.publish(PushMessage::InboxChanged, None, [me].into());

        assert_eq!(drain(&mut a).len(), 1);
        assert_eq!(drain(&mut b).len(), 1);
        assert_eq!(drain(&mut c).len(), 0, "viewing a thread is not being the member");
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn attention_has_three_tiers() {
        let mut hub = Hub::new();
        let (me, other) = (Uuid::new_v4(), Uuid::new_v4());
        let t0 = Instant::now();

        // Nothing reported: away, so pushes go out.
        assert_eq!(hub.attention(&me, &key(1), t0), Attention::Away);

        // Using the site, but not on this thread.
        hub.set_presence(me, "tab".into(), true, None, secs(0), t0);
        assert_eq!(hub.attention(&me, &key(1), t0), Attention::Active);

        // Reading this thread.
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(0), t0);
        assert_eq!(hub.attention(&me, &key(1), t0), Attention::Reading);
        assert_eq!(hub.attention(&me, &key(2), t0), Attention::Active, "reading another thread");
        assert_eq!(hub.attention(&other, &key(1), t0), Attention::Away, "a different member");
    }

    #[test]
    fn going_quiet_lowers_attention_with_no_further_report() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        let t0 = Instant::now();
        // The tab keeps reporting (heartbeats), each saying how long it has been quiet.
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(0), t0);
        assert_eq!(hub.attention(&me, &key(1), t0 + secs(30)), Attention::Reading);

        // A later heartbeat from the same tab, still untouched for two and a half minutes.
        let later = t0 + secs(150);
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(150), later);
        assert_eq!(hub.attention(&me, &key(1), later), Attention::Active, "too quiet to be reading");

        let much_later = t0 + secs(700);
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(700), much_later);
        assert_eq!(hub.attention(&me, &key(1), much_later), Attention::Away, "too quiet to be using the site");
    }

    #[test]
    fn the_tab_used_most_recently_decides() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        let t0 = Instant::now();

        // Reading in one window...
        hub.set_presence(me, "left".into(), true, Some(key(1)), secs(5), t0);
        assert_eq!(hub.attention(&me, &key(1), t0), Attention::Reading);

        // ...then the member picks up the other window, on a different page. The left one is
        // still visible and still inside its own idle window, but no longer where they are.
        hub.set_presence(me, "right".into(), true, None, secs(0), t0 + secs(10));
        assert_eq!(hub.attention(&me, &key(1), t0 + secs(10)), Attention::Active);

        // And back again.
        hub.set_presence(me, "left".into(), true, Some(key(1)), secs(0), t0 + secs(20));
        assert_eq!(hub.attention(&me, &key(1), t0 + secs(20)), Attention::Reading);
    }

    #[test]
    fn a_hidden_tab_is_not_being_used() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        let now = Instant::now();
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(0), now);
        hub.set_presence(me, "tab".into(), false, None, secs(0), now);
        assert_eq!(hub.attention(&me, &key(1), now), Attention::Away);
    }

    #[test]
    fn a_vanished_tab_stops_counting_by_itself() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        let t0 = Instant::now();
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(0), t0);
        assert_eq!(hub.attention(&me, &key(1), t0 + PRESENCE_TTL - secs(1)), Attention::Reading);
        assert_eq!(hub.attention(&me, &key(1), t0 + PRESENCE_TTL + secs(1)), Attention::Away);
    }

    #[test]
    fn presence_cannot_be_used_to_grow_memory() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        let now = Instant::now();
        for i in 0..(MAX_TABS_PER_MEMBER * 3) {
            hub.set_presence(me, format!("tab{i}"), true, Some(key(1)), secs(0), now);
        }
        assert_eq!(hub.presence.len(), MAX_TABS_PER_MEMBER);
        // A tab already counted can still update.
        hub.set_presence(me, "tab0".into(), true, Some(key(2)), secs(0), now);
        assert!(hub.presence.contains_key(&(me, "tab0".to_owned())));
    }

    #[test]
    fn closing_a_members_streams_forgets_where_they_were() {
        let mut hub = Hub::new();
        let me = Uuid::new_v4();
        let now = Instant::now();
        hub.set_presence(me, "tab".into(), true, Some(key(1)), secs(0), now);
        hub.disconnect_member(&me);
        assert_eq!(hub.attention(&me, &key(1), now), Attention::Away);
    }
}
