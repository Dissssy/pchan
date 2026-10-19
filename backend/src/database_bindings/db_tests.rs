//! Tests against a real database. See `crate::testing` for how to run them.

use std::collections::HashSet;

use common::structs::{CreatePost, PostRef};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use super::Database;
use crate::testing::{self, db, viewer};

fn key(board: &database::Board, thread: &database::Post) -> crate::push::ThreadKey {
    crate::push::ThreadKey {
        board: board.discriminator.clone(),
        number: thread.post_number,
    }
}

fn refs(board: &database::Board, posts: &[&database::Post]) -> Vec<PostRef> {
    posts
        .iter()
        .map(|p| PostRef {
            board_discriminator: board.discriminator.clone(),
            post_number: p.post_number,
        })
        .collect()
}

async fn last_read(conn: &mut testing::Conn, who: &database::Member, thread: &database::Thread) -> Option<i64> {
    use database::member_watching::dsl::*;
    member_watching
        .filter(member_id.eq(who.id))
        .filter(thread_id.eq(thread.id))
        .select(last_read)
        .first::<i64>(conn)
        .await
        .ok()
}

// ---- watching, read marks and the unread list ---------------------------------------------

#[tokio::test]
async fn watching_starts_at_the_end_and_unread_counts_what_comes_after() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (poster, reader) = (testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &poster, "unread test").await;
    testing::post(c, &board, &thread, &poster, "one", vec![]).await;
    testing::post(c, &board, &thread, &poster, "two", vec![]).await;

    let rv = viewer(&reader);
    assert!(!Database::get_watching(c, &board.discriminator, op.post_number, &rv).await.unwrap());
    Database::set_watching(c, &board.discriminator, op.post_number, &rv, true).await.unwrap();
    // Watching twice changes nothing.
    Database::set_watching(c, &board.discriminator, op.post_number, &rv, true).await.unwrap();
    assert!(Database::get_watching(c, &board.discriminator, op.post_number, &rv).await.unwrap());

    // What was already there is not news.
    assert!(Database::get_notifications(c, &rv).await.unwrap().is_empty());

    let p3 = testing::post(c, &board, &thread, &poster, "three", vec![]).await;
    let p4 = testing::post(c, &board, &thread, &poster, "four", vec![]).await;
    let list = Database::get_notifications(c, &rv).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].unread, 2);
    assert_eq!(list[0].first_unread, p3.post_number);
    assert_eq!(list[0].latest.post_number, p4.post_number);
    assert_eq!(list[0].topic, "unread test");
    assert_eq!(list[0].thread_post_number, op.post_number);

    // Reading partway leaves the rest unread.
    Database::mark_thread_read(c, &board.discriminator, op.post_number, &rv, p3.post_number).await.unwrap();
    let list = Database::get_notifications(c, &rv).await.unwrap();
    assert_eq!((list[0].unread, list[0].first_unread), (1, p4.post_number));

    // A late, lower report cannot make things unread again.
    Database::mark_thread_read(c, &board.discriminator, op.post_number, &rv, op.post_number).await.unwrap();
    assert_eq!(Database::get_notifications(c, &rv).await.unwrap()[0].unread, 1);

    Database::mark_thread_read(c, &board.discriminator, op.post_number, &rv, p4.post_number).await.unwrap();
    assert!(Database::get_notifications(c, &rv).await.unwrap().is_empty());

    // Stopping removes it for good.
    testing::post(c, &board, &thread, &poster, "five", vec![]).await;
    Database::set_watching(c, &board.discriminator, op.post_number, &rv, false).await.unwrap();
    assert!(Database::get_notifications(c, &rv).await.unwrap().is_empty());
}

#[tokio::test]
async fn mark_all_read_clears_every_watched_thread_but_only_for_that_member() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (poster, me, other) = (testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (t1, o1) = testing::thread(c, &board, &poster, "first").await;
    let (t2, o2) = testing::thread(c, &board, &poster, "second").await;
    for (viewer_member, who) in [(&me, "me"), (&other, "other")] {
        let v = viewer(viewer_member);
        Database::set_watching(c, &board.discriminator, o1.post_number, &v, true).await.unwrap();
        Database::set_watching(c, &board.discriminator, o2.post_number, &v, true).await.unwrap();
        let _ = who;
    }
    testing::post(c, &board, &t1, &poster, "a", vec![]).await;
    testing::post(c, &board, &t2, &poster, "b", vec![]).await;

    assert_eq!(Database::get_notifications(c, &viewer(&me)).await.unwrap().len(), 2);
    Database::mark_all_read(c, &viewer(&me)).await.unwrap();
    assert!(Database::get_notifications(c, &viewer(&me)).await.unwrap().is_empty());
    assert_eq!(Database::get_notifications(c, &viewer(&other)).await.unwrap().len(), 2, "someone else's list is theirs");
}

#[tokio::test]
async fn a_watch_does_not_outlive_access_to_a_private_board() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (poster, reader) = (testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, true).await;
    testing::grant_access(c, &board, &poster).await;
    testing::grant_access(c, &board, &reader).await;
    let (thread, op) = testing::thread(c, &board, &poster, "secret").await;
    let rv = viewer(&reader);
    Database::set_watching(c, &board.discriminator, op.post_number, &rv, true).await.unwrap();
    testing::post(c, &board, &thread, &poster, "news", vec![]).await;
    assert_eq!(Database::get_notifications(c, &rv).await.unwrap().len(), 1);

    testing::revoke_access(c, &board, &reader).await;
    assert!(
        Database::get_notifications(c, &rv).await.unwrap().is_empty(),
        "what is on a board must not leak to someone who can no longer see it"
    );
    // And the watcher is dropped when the next post goes out.
    let watchers = Database::get_subscribed_users(c, thread.id).await.unwrap();
    assert!(watchers.iter().all(|m| m.id != reader.id));
    assert_eq!(last_read(c, &reader, &thread).await, None);
}

#[tokio::test]
async fn posting_with_watch_on_watches_from_that_post_and_off_does_not() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (starter, on, off) = (testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &starter, "create_post").await;
    let reply = |watch: bool| CreatePost {
        file: None,
        content: format!(">>{}", op.post_number),
        author: None,
        moderator: false,
        watch,
    };

    let watched = Database::create_post(c, board.id, &board.discriminator, thread.id, reply(true), &viewer(&on), false)
        .await
        .unwrap();
    Database::create_post(c, board.id, &board.discriminator, thread.id, reply(false), &viewer(&off), false)
        .await
        .unwrap();

    assert_eq!(last_read(c, &on, &thread).await, Some(watched.post_number), "watching starts read through the post made");
    assert_eq!(last_read(c, &off, &thread).await, None);
    assert_eq!(watched.replies_to, vec![op.id], "the reply was recorded against the post it answers");
}

// ---- who wrote what ------------------------------------------------------------------------

#[tokio::test]
async fn the_authorship_check_answers_only_for_the_asker() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (alice, bob) = (testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &alice, "mine").await;
    let by_alice = testing::post(c, &board, &thread, &alice, "a", vec![]).await;
    let by_bob = testing::post(c, &board, &thread, &bob, "b", vec![]).await;

    let all = refs(&board, &[&op, &by_alice, &by_bob]);
    let mut alices = Database::authored_posts(c, &viewer(&alice), all.clone()).await.unwrap();
    alices.sort_by_key(|r| r.post_number);
    assert_eq!(alices, refs(&board, &[&op, &by_alice]));
    assert_eq!(
        Database::authored_posts(c, &viewer(&bob), all).await.unwrap(),
        refs(&board, &[&by_bob])
    );

    // Posts that are not there, and boards that are not there, are just absent.
    let mut missing = refs(&board, &[&by_alice]);
    missing.push(PostRef { board_discriminator: board.discriminator.clone(), post_number: 999_999 });
    missing.push(PostRef { board_discriminator: "no-such-board".into(), post_number: 1 });
    assert_eq!(Database::authored_posts(c, &viewer(&alice), missing).await.unwrap(), refs(&board, &[&by_alice]));
}

#[tokio::test]
async fn the_authorship_check_skips_private_boards_and_caps_the_batch() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (alice, outsider) = (testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, true).await;
    testing::grant_access(c, &board, &alice).await;
    let (_, op) = testing::thread(c, &board, &alice, "private").await;
    assert_eq!(Database::authored_posts(c, &viewer(&alice), refs(&board, &[&op])).await.unwrap().len(), 1);
    assert!(Database::authored_posts(c, &viewer(&outsider), refs(&board, &[&op])).await.unwrap().is_empty());

    let too_many: Vec<PostRef> = (0..=Database::MAX_AUTHORED_BATCH as i64)
        .map(|n| PostRef { board_discriminator: board.discriminator.clone(), post_number: n })
        .collect();
    assert!(Database::authored_posts(c, &viewer(&alice), too_many).await.is_err());
}

// ---- replies --------------------------------------------------------------------------------

#[tokio::test]
async fn catching_up_on_replies_is_the_same_for_everyone_minus_your_own() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (author, replier, bystander) = (testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &author, "replies").await;

    // A first visit starts from now instead of replaying history.
    let first = Database::replies_since(c, &viewer(&bystander), None).await.unwrap();
    assert!(first.posts.is_empty() && !first.more);

    let plain = testing::post(c, &board, &thread, &replier, "no reference here", vec![]).await;
    let reply = testing::post(c, &board, &thread, &replier, &format!(">>{}", op.post_number), vec![op.id]).await;
    let _ = plain;

    let seen = Database::replies_since(c, &viewer(&bystander), Some(first.cursor)).await.unwrap();
    let ours: Vec<_> = seen.posts.iter().filter(|p| p.board_discriminator == board.discriminator).collect();
    assert_eq!(ours.len(), 1, "only the post that replies to something");
    assert_eq!(ours[0].post_number, reply.post_number);
    assert!(seen.cursor >= reply.id);

    // The replier does not hear about their own post.
    let theirs = Database::replies_since(c, &viewer(&replier), Some(first.cursor)).await.unwrap();
    assert!(theirs.posts.iter().all(|p| p.board_discriminator != board.discriminator));

    // Asking again from where that left off finds nothing new.
    let again = Database::replies_since(c, &viewer(&bystander), Some(seen.cursor)).await.unwrap();
    assert!(again.posts.iter().all(|p| p.board_discriminator != board.discriminator));
}

#[tokio::test]
async fn catching_up_pages_through_a_long_absence_and_hides_private_boards() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (author, replier, reader, outsider) =
        (testing::member(c).await, testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let secret = testing::board(c, true).await;
    testing::grant_access(c, &secret, &author).await;
    testing::grant_access(c, &secret, &replier).await;
    testing::grant_access(c, &secret, &reader).await;
    let (thread, op) = testing::thread(c, &board, &author, "long").await;
    let (sthread, sop) = testing::thread(c, &secret, &author, "hidden").await;

    let start = Database::replies_since(c, &viewer(&reader), None).await.unwrap().cursor;
    let total = Database::MAX_REPLIES_PAGE as usize + 5;
    for _ in 0..total {
        testing::post(c, &board, &thread, &replier, &format!(">>{}", op.post_number), vec![op.id]).await;
    }
    testing::post(c, &secret, &sthread, &replier, &format!(">>{}", sop.post_number), vec![sop.id]).await;

    let page1 = Database::replies_since(c, &viewer(&reader), Some(start)).await.unwrap();
    assert!(page1.more);
    assert_eq!(page1.posts.len(), Database::MAX_REPLIES_PAGE as usize);
    let page2 = Database::replies_since(c, &viewer(&reader), Some(page1.cursor)).await.unwrap();
    assert!(!page2.more);
    let everything: Vec<_> = page1.posts.iter().chain(&page2.posts).collect();
    assert_eq!(everything.iter().filter(|p| p.board_discriminator == board.discriminator).count(), total);
    assert_eq!(
        everything.iter().filter(|p| p.board_discriminator == secret.discriminator).count(),
        1,
        "a member with access sees the private board's reply"
    );

    // Someone without access sees none of the private board, and still gets a sensible cursor.
    let outside = Database::replies_since(c, &viewer(&outsider), Some(start)).await.unwrap();
    assert!(outside.posts.iter().all(|p| p.board_discriminator != secret.discriminator));
}

#[tokio::test]
async fn reply_recipients_are_the_opted_in_authors_of_what_was_replied_to() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (alice, bob, carol, replier) =
        (testing::member(c).await, testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &alice, "recipients").await;
    let alices = op.clone();
    let bobs = testing::post(c, &board, &thread, &bob, "bob's", vec![]).await;
    let carols = testing::post(c, &board, &thread, &carol, "carol's", vec![]).await;

    // Nobody has opted in: nobody is pushed.
    let none = Database::reply_recipients(c, &[alices.id, bobs.id], replier.id, board.id).await.unwrap();
    assert!(none.is_empty());

    database::set_reply_push(c, &alice.id, true).await.unwrap();
    database::set_reply_push(c, &bob.id, true).await.unwrap();
    assert!(database::reply_push_enabled(c, &alice.id).await.unwrap());
    assert!(!database::reply_push_enabled(c, &carol.id).await.unwrap());

    // Replying to Alice's and Bob's posts reaches them, not Carol (who did not opt in, and
    // whose post was not replied to anyway).
    let got = Database::reply_recipients(c, &[alices.id, bobs.id], replier.id, board.id).await.unwrap();
    assert_eq!(got, HashSet::from([alice.id, bob.id]));

    // Only the authors of the posts actually replied to.
    let just_alice = Database::reply_recipients(c, &[alices.id], replier.id, board.id).await.unwrap();
    assert_eq!(just_alice, HashSet::from([alice.id]));

    // Answering your own post is not news.
    let own = Database::reply_recipients(c, &[alices.id], alice.id, board.id).await.unwrap();
    assert!(own.is_empty());

    // Opting out takes effect straight away.
    database::set_reply_push(c, &alice.id, false).await.unwrap();
    assert!(Database::reply_recipients(c, &[alices.id], replier.id, board.id).await.unwrap().is_empty());
    let _ = carols;
}

#[tokio::test]
async fn reply_recipients_must_still_be_able_to_see_the_board() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (alice, replier) = (testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, true).await;
    testing::grant_access(c, &board, &alice).await;
    testing::grant_access(c, &board, &replier).await;
    let (_, op) = testing::thread(c, &board, &alice, "private reply").await;
    database::set_reply_push(c, &alice.id, true).await.unwrap();

    assert_eq!(
        Database::reply_recipients(c, &[op.id], replier.id, board.id).await.unwrap(),
        HashSet::from([alice.id])
    );
    testing::revoke_access(c, &board, &alice).await;
    assert!(
        Database::reply_recipients(c, &[op.id], replier.id, board.id).await.unwrap().is_empty(),
        "a push would carry the post to someone who can no longer see it"
    );
}

// ---- dispatch: who hears about a new post, and how -----------------------------------------

/// Everything a connection has been sent so far, as post numbers of new-post events.
fn heard(rx: &mut tokio::sync::mpsc::Receiver<crate::push::Delivery>) -> Vec<i64> {
    let mut out = Vec::new();
    while let Ok(d) = rx.try_recv() {
        if let common::structs::PushMessage::NewPost(p) = d.message {
            out.push(p.post_number);
        }
    }
    out
}

#[tokio::test]
async fn dispatch_treats_a_member_by_how_closely_they_are_paying_attention() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (poster, reading, active, away) =
        (testing::member(c).await, testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &poster, "tiers").await;
    for m in [&reading, &active, &away] {
        Database::set_watching(c, &board.discriminator, op.post_number, &viewer(m), true).await.unwrap();
    }

    let k = key(&board, &op);
    let (mut rx_reading, mut rx_active, mut rx_away) = {
        let mut hub = crate::push::lock(&crate::PUSH_NOTIFS);
        let now = std::time::Instant::now();
        let none = std::time::Duration::ZERO;
        hub.set_presence(reading.id, "tab".into(), true, Some(k.clone()), none, now);
        hub.set_presence(active.id, "tab".into(), true, None, none, now);
        (
            hub.attach(reading.id, "s1".into(), None, None).2,
            hub.attach(active.id, "s2".into(), None, None).2,
            hub.attach(away.id, "s3".into(), None, None).2,
        )
    };

    let post = testing::post(c, &board, &thread, &poster, "hello", vec![]).await;
    let safe = post.safe(c, &viewer(&poster)).await.unwrap();
    Database::dispatch_push_notifications(safe, thread.id, poster.id, board.id, vec![]).await;

    assert!(heard(&mut rx_reading).is_empty(), "reading it: no notification at all");
    assert_eq!(heard(&mut rx_active), vec![post.post_number]);
    assert_eq!(heard(&mut rx_away), vec![post.post_number]);
    // The one who was reading has it on screen, so it counts as read; the others do not.
    assert_eq!(last_read(c, &reading, &thread).await, Some(post.post_number));
    assert_ne!(last_read(c, &active, &thread).await, Some(post.post_number));
    assert_ne!(last_read(c, &away, &thread).await, Some(post.post_number));

    crate::push::lock(&crate::PUSH_NOTIFS).disconnect_member(&reading.id);
    crate::push::lock(&crate::PUSH_NOTIFS).disconnect_member(&active.id);
    crate::push::lock(&crate::PUSH_NOTIFS).disconnect_member(&away.id);
}

#[tokio::test]
async fn dispatch_sends_every_reply_to_every_connected_member_who_can_see_the_board() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (author, replier, bystander, outsider) =
        (testing::member(c).await, testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let open = testing::board(c, false).await;
    let secret = testing::board(c, true).await;
    for m in [&author, &replier, &bystander] {
        testing::grant_access(c, &secret, m).await;
    }

    // Public board: a bystander who watches nothing still hears the reply.
    let (t, op) = testing::thread(c, &open, &author, "open").await;
    let mut rx_bystander = crate::push::lock(&crate::PUSH_NOTIFS).attach(bystander.id, "s".into(), None, None).2;
    let mut rx_replier = crate::push::lock(&crate::PUSH_NOTIFS).attach(replier.id, "s".into(), None, None).2;
    let reply = testing::post(c, &open, &t, &replier, &format!(">>{}", op.post_number), vec![op.id]).await;
    let safe = reply.safe(c, &viewer(&replier)).await.unwrap();
    Database::dispatch_push_notifications(safe, t.id, replier.id, open.id, vec![op.id]).await;
    assert_eq!(heard(&mut rx_bystander), vec![reply.post_number]);
    assert!(heard(&mut rx_replier).is_empty(), "the author of the reply is not told about it");

    // A post that is not a reply is not broadcast.
    let plain = testing::post(c, &open, &t, &replier, "just chatting", vec![]).await;
    let safe = plain.safe(c, &viewer(&replier)).await.unwrap();
    Database::dispatch_push_notifications(safe, t.id, replier.id, open.id, vec![]).await;
    assert!(heard(&mut rx_bystander).is_empty());

    // Private board: only members with access hear it, and never the outsider.
    let (st, sop) = testing::thread(c, &secret, &author, "secret").await;
    let mut rx_outsider = crate::push::lock(&crate::PUSH_NOTIFS).attach(outsider.id, "s".into(), None, None).2;
    let sreply = testing::post(c, &secret, &st, &replier, &format!(">>{}", sop.post_number), vec![sop.id]).await;
    let safe = sreply.safe(c, &viewer(&replier)).await.unwrap();
    Database::dispatch_push_notifications(safe, st.id, replier.id, secret.id, vec![sop.id]).await;
    assert_eq!(heard(&mut rx_bystander), vec![sreply.post_number]);
    assert!(heard(&mut rx_outsider).is_empty(), "a private board's posts are not for people who cannot see it");

    for m in [&bystander, &replier, &outsider] {
        crate::push::lock(&crate::PUSH_NOTIFS).disconnect_member(&m.id);
    }
}

#[tokio::test]
async fn advancing_a_read_mark_only_moves_it_forward_for_the_members_named() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (poster, a, b) = (testing::member(c).await, testing::member(c).await, testing::member(c).await);
    let board = testing::board(c, false).await;
    let (thread, op) = testing::thread(c, &board, &poster, "advance").await;
    for m in [&a, &b] {
        Database::set_watching(c, &board.discriminator, op.post_number, &viewer(m), true).await.unwrap();
    }
    let start = last_read(c, &a, &thread).await.unwrap();

    Database::advance_read(c, thread.id, &HashSet::from([a.id]), start + 10).await.unwrap();
    assert_eq!(last_read(c, &a, &thread).await, Some(start + 10));
    assert_eq!(last_read(c, &b, &thread).await, Some(start), "the other member was not named");

    Database::advance_read(c, thread.id, &HashSet::from([a.id]), start + 3).await.unwrap();
    assert_eq!(last_read(c, &a, &thread).await, Some(start + 10), "never backwards");
}

// ---- push subscriptions ----------------------------------------------------------------------

fn sub(n: &str) -> common::structs::SubscriptionData {
    common::structs::SubscriptionData {
        endpoint: format!("https://fcm.googleapis.com/fcm/send/{n}"),
        keys: common::structs::SubscriptionKeys { auth: "auth".into(), p256dh: "p256dh".into() },
    }
}

async fn holders(c: &mut testing::Conn, endpoint: &str) -> Vec<uuid::Uuid> {
    use database::member_push::dsl::*;
    member_push
        .filter(endpoint_hash.eq(database::keys::push_endpoint_hash(endpoint)))
        .select(member_id)
        .load::<uuid::Uuid>(c)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_push_subscription_belongs_to_one_member_and_the_latest_one_to_subscribe_has_it() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (alice, bob) = (testing::member(c).await, testing::member(c).await);
    let s = sub(&uuid::Uuid::new_v4().to_string());

    database::add_push_subscription(c, &alice.id, &s).await.unwrap();
    database::add_push_subscription(c, &alice.id, &s).await.unwrap();
    assert_eq!(holders(c, &s.endpoint).await, vec![alice.id], "the same one again is not a second row");
    assert_eq!(database::get_push_subscriptions(c, &alice.id).await.unwrap().len(), 1);

    // Bob signs in on the same browser and subscribes: it moves to him, so Alice stops
    // receiving Bob's notifications there and Bob stops receiving Alice's.
    database::add_push_subscription(c, &bob.id, &s).await.unwrap();
    assert_eq!(holders(c, &s.endpoint).await, vec![bob.id]);
    assert!(database::get_push_subscriptions(c, &alice.id).await.unwrap().is_empty());

    // Alice cannot remove Bob's by asking.
    database::remove_push_endpoint(c, &alice.id, &s.endpoint).await.unwrap();
    assert_eq!(holders(c, &s.endpoint).await, vec![bob.id]);
    database::remove_push_endpoint(c, &bob.id, &s.endpoint).await.unwrap();
    assert!(holders(c, &s.endpoint).await.is_empty());
}

#[tokio::test]
async fn push_subscriptions_are_validated_and_capped() {
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let member = testing::member(c).await;

    let mut bad = sub("x");
    bad.endpoint = "https://localhost/internal".into();
    assert!(database::add_push_subscription(c, &member.id, &bad).await.is_err(), "not a push service");
    let mut empty_keys = sub("y");
    empty_keys.keys.auth.clear();
    assert!(database::add_push_subscription(c, &member.id, &empty_keys).await.is_err());

    for n in 0..20 {
        database::add_push_subscription(c, &member.id, &sub(&format!("{}-{n}", uuid::Uuid::new_v4()))).await.unwrap();
    }
    assert!(
        database::add_push_subscription(c, &member.id, &sub(&uuid::Uuid::new_v4().to_string())).await.is_err(),
        "every post fans out to every device, so the number of them is limited"
    );
    // Re-subscribing a device they already have is not blocked by the limit.
    let existing = database::get_push_subscriptions(c, &member.id).await.unwrap();
    database::add_push_subscription(c, &member.id, &existing[0].1).await.unwrap();
}

#[tokio::test]
async fn old_subscriptions_are_given_hashes_and_shared_ones_are_collapsed() {
    use database::member_push::dsl::*;
    let Some(mut db) = db().await else { return };
    let c = &mut db.conn;
    let (alice, bob) = (testing::member(c).await, testing::member(c).await);
    let shared = sub(&uuid::Uuid::new_v4().to_string());
    let alone = sub(&uuid::Uuid::new_v4().to_string());

    // Rows as they were before endpoint hashes existed: no hash, and one browser under two members.
    for (who, s) in [(&alice, &shared), (&bob, &shared), (&alice, &alone)] {
        let sealed = database::keys::seal_push(&serde_json::to_vec(s).unwrap()).unwrap();
        diesel::insert_into(member_push)
            .values((member_id.eq(who.id), data.eq(sealed)))
            .execute(c)
            .await
            .unwrap();
    }

    database::backfill_push_endpoint_hashes(c).await.unwrap();

    // Anything of ours still unhashed would be a failure; and the shared browser has one owner.
    let unhashed: i64 = member_push
        .filter(member_id.eq_any([alice.id, bob.id]))
        .filter(endpoint_hash.is_null())
        .count()
        .get_result(c)
        .await
        .unwrap();
    assert_eq!(unhashed, 0);
    assert_eq!(holders(c, &shared.endpoint).await.len(), 1);
    assert_eq!(holders(c, &alone.endpoint).await, vec![alice.id]);
    // Running it again is harmless.
    database::backfill_push_endpoint_hashes(c).await.unwrap();
    assert_eq!(holders(c, &shared.endpoint).await.len(), 1);
}
