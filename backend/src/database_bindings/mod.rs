// use std::io::{Read, Write};

use std::sync::Arc;

use anyhow::{anyhow, Result};
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::now;
use diesel::insert_into;
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::methods::OrderDsl;
use diesel::query_dsl::methods::SelectDsl;
use diesel::ExpressionMethods;
use diesel::PgArrayExpressionMethods;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};
use profanity::replace_possible_profanity;
use rand::seq::SliceRandom as _;
use futures::StreamExt as _;
use web_push::WebPushClient as _;
// use serde_json::json;
// use web_push::SubscriptionInfo;
// use web_push::VapidSignatureBuilder;
// use web_push::WebPushClient;
// use web_push::WebPushMessageBuilder;

use common::structs::SubscriptionData;
// use crate::endpoints::api::SubscriptionData;
use database::thread_post_number;
use database::Banner;
use database::Post;

// pub struct Users {
//     valid_users: Vec<&str>,
// }

// impl Users {
//     pub fn new() -> Result<Self> {
//         Ok(Self {
//             valid_users: vec![],
//         })
//     }
//     pub async fn open(&mut self) -> Result<()> {
//         if let Ok(auth) = tokio::fs::read("./auth.bin.gz").await {
//             let mut bytes = vec![];
//             {
//                 let mut auth = flate2::read::GzDecoder::new(auth.as_slice());
//                 let _ = auth.read_to_end(&mut bytes);
//             }
//             let auth: Vec<&str> = postcard::from_bytes(&bytes).unwrap_or_default();
//             self.valid_users = auth;
//         }

//         Ok(())
//     }
//     pub async fn close(&mut self) -> Result<()> {
//         let mut auth = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
//         auth.write_all(&postcard::to_allocvec(&self.valid_users)?)?;
//         tokio::fs::write("./auth.bin.gz", auth.finish()?).await?;
//         Ok(())
//     }

//     pub async fn is_auth(&mut self, token: MemberToken) -> Result<bool> {
//         Ok(self.valid_users.contains(&token))
//     }
//     pub async fn add_auth(&mut self, token: MemberToken) -> Result<()> {
//         self.valid_users.push(token);
//         Ok(())
//     }
//     pub async fn sync_auth(&mut self, tokens: Vec<&str>) -> Result<()> {
//         self.valid_users = tokens;
//         Ok(())
//     }
//     pub async fn remove_auth(&mut self, token: MemberToken) -> Result<()> {
//         self.valid_users.retain(|x| x != &token);
//         Ok(())
//     }
// }

pub struct Database;

impl Database {
    pub async fn create_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        new_discriminator: &str,
        new_name: &str,
    ) -> Result<()> {
        use database::boards::dsl::*;

        insert_into(boards)
            .values((discriminator.eq(new_discriminator), name.eq(new_name)))
            .execute(conn)
            .await?;
        Ok(())
    }
    pub async fn get_boards(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
    ) -> Result<Vec<database::Board>> {
        use database::boards::dsl::*;

        let results: Vec<database::Board> = boards
            .order(name.desc())
            .load::<database::Board>(&mut *conn)
            .await?;

        let mut final_results = vec![];
        for board in results {
            if database::check_access(conn, viewer, board.id)
                .await
                .inspect_err(|e| log::error!("Error checking access: {}", e))
                .unwrap_or(false)
            {
                final_results.push(board);
            }
        }

        Ok(final_results)
    }

    // pub async fn is_allowed_access<'a>(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     token: MemberToken,
    //     board: BoardOrDiscriminator<'a>,
    // ) -> Result<bool> {
    //     use database::user_tags::dsl::*;

    //     let board = match board {
    //         BoardOrDiscriminator::Discriminator(disc) => Self::get_raw_board(conn, disc).await?,
    //         BoardOrDiscriminator::Board(board) => board,
    //     };

    //     if !board.private {
    //         return Ok(true);
    //     }

    //     // let permission_level = Self::is_admin(conn, viewer, board.id).await?;
    //     let permission_level =
    //         database::permission_level(conn, board.id, viewer).await?;
    //     if !permission_level.is_none() {
    //         return Ok(true);
    //     }

    //     let hash = common::hash_invitation(viewer, board.id);

    //     use diesel::dsl::{exists, select};

    //     let exists = select(exists(
    //         user_tags.filter(
    //             invite_hash
    //                 .eq(Some(hash))
    //                 .and(board_id.eq(board.id))
    //                 .and(tag_kind.eq(UserTag::BoardAccess.to_string())),
    //         ),
    //     ))
    //     .get_result(conn)
    //     .await?;

    //     // match results {
    //     //     Ok(_) => Ok(true),
    //     //     Err(diesel::NotFound) => Ok(false),
    //     //     Err(e) => Err(e.into()),
    //     // }
    //     Ok(exists)
    // }

    pub async fn generate_board_access_code(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
        board_id: i64,
        invite_name: String,
    ) -> Result<String> {
        database::create_access(conn, invite_name.as_str(), viewer, board_id).await
    }

    pub async fn generate_board_moderator_code(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
        board_id: i64,
        invite_name: String,
    ) -> Result<String> {
        database::create_moderation(conn, invite_name.as_str(), viewer, board_id)
            .await
    }

    pub async fn consume_code(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
        invite_name: String,
    ) -> Result<()> {
        database::grant_access(conn, &invite_name, viewer).await
    }

    pub async fn get_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discrim: &str,
        viewer: &database::Viewer,
    ) -> Result<database::Board> {
        use database::boards::dsl::*;

        let result = boards
            .filter(discriminator.eq(discrim))
            .first::<database::Board>(&mut *conn)
            .await?;

        database::check_access(conn, viewer, result.id)
            .await
            .inspect_err(|e| log::error!("Error checking access: {}", e))
            .and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(anyhow!("Not authorized to view board"))
                }
            })?;

        Ok(result)
    }

    pub async fn get_raw_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discim: &str,
    ) -> Result<database::Board> {
        use database::boards::dsl::*;

        let results = boards
            .filter(discriminator.eq(discim))
            .first::<database::Board>(&mut *conn)
            .await?;

        Ok(results)
    }

    pub async fn get_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: &str,
        number: i64,
        viewer: &database::Viewer,
    ) -> Result<SafePost> {
        let board = Self::get_board(conn, discriminator, viewer).await?;

        database::check_access(conn, viewer, board.id)
            .await
            .inspect_err(|e| log::error!("Error checking access: {}", e))
            .and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(anyhow!("Not authorized to view post"))
                }
            })?;

        Self::get_raw_post(conn, discriminator, number)
            .await?
            .safe(conn, viewer)
            .await
    }

    pub async fn get_raw_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: &str,
        number: i64,
    ) -> Result<database::Thread> {
        use database::threads::dsl::*;
        let tpost = Self::get_raw_post(conn, discriminator, number).await?;
        let this_board = Self::get_raw_board(conn, discriminator).await?;
        Ok(threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(tpost.id))
            .first::<database::Thread>(&mut *conn)
            .await?)
    }

    // pub async fn get_post_from_id(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     id: i64,
    // ) -> Result<SafePost> {
    //     use database::posts::dsl::*;

    //     let post = posts
    //         .filter(id.eq(id))
    //         .first::<database::Post>(&mut *conn)
    //         .await?;
    //     post.safe(conn).await
    // }

    async fn get_raw_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: &str,
        number: i64,
    ) -> Result<Post> {
        use database::posts::dsl::*;

        let this_board = Self::get_raw_board(conn, discriminator).await?;
        Ok(posts
            .filter(board.eq(this_board.id))
            .filter(post_number.eq(number))
            .first::<database::Post>(&mut *conn)
            .await?)
    }

    pub async fn delete_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: &str,
        number: i64,
        viewer: &database::Viewer,
    ) -> Result<(i64, bool)> {
        let board = Self::get_board(conn, discriminator, viewer).await?;

        database::check_access(conn, viewer, board.id)
            .await
            .inspect_err(|e| log::error!("Error checking access: {}", e))
            .and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(anyhow!("Not authorized to delete post"))
                }
            })?;
        let tpost = Self::get_raw_post(conn, discriminator, number).await?;
        let tauthor = tpost.actual_author == *viewer.post_author_hash(tpost.id);
        // let tadmin = Self::is_admin(conn, token, tpost.board).await?;
        let at_least_mod = !database::permission_level(conn, tpost.board, viewer)
            .await?
            .is_none();

        if !(at_least_mod || tauthor) {
            return Err(anyhow!("Not authorized to delete post"));
        }

        let tthread = Self::get_raw_thread(conn, discriminator, number).await;
        let id = tpost.id;
        // Read before the row is gone: once it is deleted there is nothing to look this up in.
        let owning_thread = thread_post_number(tpost.thread, conn).await.unwrap_or(number);
        let thread_deleted = tthread.is_ok();
        match (at_least_mod, tauthor, tthread, tpost) {
            // if the user is an admin they can delete a post
            (true, _, Err(_), post) => {
                Self::raw_delete_post(conn, post.id).await?;
            }
            // if the user is an admin they can delete a thread
            (true, _, Ok(thrd), _) => {
                Self::raw_delete_thread(conn, thrd.id).await?;
            }
            // if the user is the author of the post and it is not a thread they can delete it
            (false, true, Err(_), post) => {
                Self::raw_delete_post(conn, post.id).await?;
            }
            // otherwise they are not authorized to delete the post
            _ => {
                return Err(anyhow::anyhow!("Not authorized to delete post"));
            }
        }
        crate::push::lock(&crate::PUSH_NOTIFS).publish(
            common::structs::PushMessage::PostDeleted(Arc::new(common::structs::DeletedPost {
                board_discriminator: discriminator.to_owned(),
                thread_post_number: owning_thread,
                post_number: number,
                thread_deleted,
            })),
            Some(crate::push::ThreadKey {
                board: discriminator.to_owned(),
                number: owning_thread,
            }),
            Default::default(),
        );
        // Whether this was an act of moderation rather than someone removing their own post.
        // Deleting your own post is deliberately NOT moderation even if you happen to be a
        // moderator: an audit entry naming the actor and the post would link that member to
        // that post, which is exactly the correlation the schema avoids.
        Ok((id, at_least_mod && !tauthor))
    }

    async fn raw_delete_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tid: i64,
    ) -> Result<()> {
        use database::posts::dsl::*;

        diesel::delete(posts.filter(id.eq(tid)))
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn raw_delete_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tid: i64,
    ) -> Result<()> {
        use database::threads::dsl::*;

        diesel::delete(threads.filter(id.eq(tid)))
            .execute(conn)
            .await?;
        Ok(())
    }

    // async fn is_admin(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     token: MemberToken,
    //     board_id: i64,
    // ) -> Result<bool> {
    //     // use database::members::dsl::*;
    //     // use diesel::query_dsl::methods::SelectDsl;

    //     // // get this members moderates Vec<i64>
    //     // let status = members
    //     //     .select(moderates)
    //     //     .filter(token_hash.eq(&*token.member_hash()))
    //     //     .first::<Option<Vec<i64>>>(&mut *conn)
    //     //     .await?;

    //     // Ok(match status {
    //     //     Some(status) => status.is_empty() || status.contains(&board_id),
    //     //     None => false,
    //     // })
    //     database::check_moderation(conn, board, token)
    // }

    pub async fn create_file(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        file: crate::unclaimedfiles::ClaimedFile,
        post_id: i64,
    ) -> Result<database::File> {
        use database::files::dsl::*;

        let tf = insert_into(files)
            .values((
                filepath.eq(file.info.path),
                hash.eq(file.hash),
                id.eq(post_id),
                spoiler.eq(file.info.spoiler),
            ))
            .get_result::<database::File>(conn)
            .await?;

        Ok(tf)
    }
    pub async fn get_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: &str,
        number: i64,
        viewer: &database::Viewer,
    ) -> Result<ThreadWithPosts> {
        use database::threads::dsl::*;
        let this_board = Self::get_board(conn, discriminator, viewer).await?;
        let this_post = Self::get_raw_post(conn, discriminator, number).await?;
        let results = threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(this_post.id))
            .first::<database::Thread>(&mut *conn)
            .await?;
        results.with_posts(conn, viewer).await
    }

    // pub async fn get_thread_from_post_number(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     bboard: i64,
    //     number: i64,
    // ) -> Result<ThreadWithPosts> {
    //     use database::threads::dsl::*;
    //     let this_post = Self::get_post_from_post_number(conn, bboard, number).await?;
    //     let results = threads
    //         .filter(board.eq(bboard))
    //         .filter(post_id.eq(this_post.id))
    //         .first::<database::Thread>(&mut *conn)
    //         .await?;
    //     results.with_posts(conn).await
    // }

    // pub async fn get_post_from_post_number(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     bboard: i64,
    //     number: i64,
    // ) -> Result<database::Post> {
    //     use database::posts::dsl::*;
    //     let results = posts
    //         .filter(board.eq(bboard))
    //         .filter(post_number.eq(number))
    //         .first::<database::Post>(&mut *conn)
    //         .await?;
    //     Ok(results)
    // }

    pub async fn create_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: &str,
        mut thread: CreateThread,
        viewer: &database::Viewer,
    ) -> Result<ThreadWithPosts> {
        use database::threads::dsl::*;

        if thread.topic.is_empty() {
            return Err(anyhow::anyhow!("No topic provided"));
        }

        thread.topic = replace_possible_profanity(thread.topic, &crate::PROFANITY, || {
            crate::QUOTES.random_quote()
        });

        if thread.post.file.is_none() {
            return Err(anyhow::anyhow!("No file provided"));
        }

        let this_board = Self::get_board(conn, tboard, viewer).await?;

        let mut t = insert_into(threads)
            .values((
                board.eq(this_board.id),
                post_id.eq(0),
                latest_post.eq(0),
                topic.eq(thread.topic),
            ))
            .get_result::<database::Thread>(conn)
            .await?;

        let p = match Self::create_post(
            conn,
            this_board.id,
            tboard,
            t.id,
            thread.post,
            viewer,
            false,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                diesel::delete(threads.filter(id.eq(t.id)))
                    .execute(conn)
                    .await?;
                return Err(e);
            }
        };
        t.post_id = p.id;
        t.with_posts(conn, viewer).await
    }

    pub async fn create_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: i64,
        discriminator: &str,
        tthread: i64,
        mut post: CreatePost,
        viewer: &database::Viewer,
        check_duplicates_in_thread: bool,
    ) -> Result<database::Post> {
        use database::posts::dsl::*;

        post.content = post.content.trim().to_string();
        if post.content.is_empty() && post.file.is_none() {
            return Err(anyhow::anyhow!(
                "Either content or an file must be provided for any post"
            ));
        }

        post.content = replace_possible_profanity(post.content, &crate::PROFANITY, || {
            crate::QUOTES.random_quote()
        });
        post.author = post.author.map(|string| {
            replace_possible_profanity(string, &crate::PROFANITY, || crate::QUOTES.random_quote())
        });
        let this_post_number = Self::get_board(conn, discriminator, viewer)
            .await?
            .post_count
            + 1;
        // THIS LINE, THE THREAD DOESNT EXIST LOOOL
        let thread_post_number = match thread_post_number(tthread, conn).await {
            Ok(v) => v,
            Err(_) => this_post_number,
        };

        let replies = post
            .content
            .split_whitespace()
            .flat_map(|x| {
                common::structs::Reply::from_str(x, discriminator, &thread_post_number.to_string())
            })
            .collect::<Vec<common::structs::Reply>>();
        let mut replieses = Vec::new();

        for reply in replies {
            let this_post = Self::get_raw_post(
                conn,
                &reply.board_discriminator,
                reply.post_number.parse::<i64>()?,
            )
            .await
            .map(|x| x.id);
            if let Ok(this_post) = this_post {
                replieses.push(this_post);
            }
        }
        let lock = crate::FS_LOCK.lock().await;

        let pending_file = if let Some(file) = post.file.clone() {
            let f = crate::UNCLAIMED_FILES
                .lock()
                .await
                .claim_file(&file, viewer, thread_post_number == this_post_number)
                .await?;

            if check_duplicates_in_thread {
                // files.id is the post id, so this needs a subquery rather than a join.
                use database::files::dsl as fd;
                use database::posts::dsl as pd;
                use diesel::dsl::{exists, select};
                let duplicate: bool = select(exists(
                    fd::files.filter(fd::hash.eq(&f.hash)).filter(
                        fd::id.eq_any(pd::posts.filter(pd::thread.eq(tthread)).select(pd::id)),
                    ),
                ))
                .get_result(conn)
                .await?;
                if duplicate {
                    return Err(anyhow::anyhow!("File already exists"));
                }
            }

            Some(f)
        } else {
            None
        };

        let at_least_mod = !database::permission_level(conn, tboard, viewer)
            .await?
            .is_none();

        // if !Self::is_admin(conn, viewer, board).await? {
        //     post.moderator = false;
        // }

        // Reserve the post id up front so actual_author can be written correctly in a single
        // statement. The previous version inserted the member's raw identifier and then
        // UPDATEd it to the per-post hash, which left a window — and a permanent record if
        // the update ever failed — where posts.actual_author was the exact value needed to
        // enumerate every other post by that member.
        let new_id: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
            "nextval('posts_id_seq')",
        ))
        .get_result(conn)
        .await?;

        let t = insert_into(posts).values((
            id.eq(new_id),
            post_number.eq(this_post_number),
            moderator.eq(post.moderator && at_least_mod),
            thread.eq(tthread),
            board.eq(tboard),
            author.eq(&post.author),
            content.eq(&post.content),
            replies_to.eq(replieses),
            timestamp.eq(now),
            actual_author.eq(viewer.post_author_hash(new_id)),
        ));
        let p = t.get_result::<database::Post>(conn).await?;

        if let Some(f) = pending_file {
            Self::create_file(conn, f, p.id).await?;
        }

        drop(lock);

        if post.watch {
            use database::member_watching::dsl as w;
            // Part of posting rather than a second request. A failure here must not fail a
            // post that already exists.
            // Starts read through this post: it is the newest one, and they just wrote it.
            if let Err(e) = insert_into(w::member_watching)
                .values((
                    w::member_id.eq(viewer.id),
                    w::thread_id.eq(tthread),
                    w::last_read.eq(this_post_number),
                ))
                .on_conflict_do_nothing()
                .execute(conn)
                .await
            {
                log::warn!("could not watch the thread after posting: {e}");
            }
        }

        let safe = p.safe(conn, viewer).await?;
        let author_id = viewer.id;
        let board_id = p.board;
        // Ids of the posts this one replies to.
        let reply_targets = p.replies_to.clone();
        tokio::spawn(async move {
            Self::dispatch_push_notifications(safe, tthread, author_id, board_id, reply_targets).await;
        });

        Ok(p)
    }

    pub async fn dispatch_push_notifications(
        safe: common::structs::SafePost,
        thread: i64,
        this_author: uuid::Uuid,
        board_id: i64,
        reply_targets: Vec<i64>,
    ) {
        use std::collections::HashSet;

        let mut conn = match crate::POOL.get().await {
            Ok(v) => v,
            Err(e) => {
                log::error!("Error getting connection: {}", e);
                return;
            }
        };
        // A failed lookup must not hide the post from people who have the thread open, so
        // it degrades to "no watchers" and still publishes.
        let watchers = match Self::get_subscribed_users(&mut conn, thread).await {
            Ok(v) => v,
            Err(e) => {
                log::error!("Could not load watchers of thread {}: {}", thread, e);
                vec![]
            }
        };
        let watchers = watchers
            .iter()
            .filter(|x| x.id != this_author)
            .collect::<Vec<_>>();

        let key = crate::push::ThreadKey {
            board: safe.board_discriminator.clone(),
            number: safe.thread_post_number,
        };

        // A reply is also sent to everyone who is connected and can see the board, and each
        // browser decides for itself whether it replies to one of its member's posts. The
        // server sends the same thing to everyone, so this part never needs to know who wrote
        // what. Pushes are the exception (a browser must show something for every push), so
        // for members who opted in the server works out, in memory, whose post it was. See
        // "Design constraint" in CLAUDE.md for what that may and may not do.
        let is_reply = !reply_targets.is_empty();
        let repliers: HashSet<uuid::Uuid> = if is_reply {
            Self::reply_recipients(&mut conn, &reply_targets, this_author, board_id)
                .await
                .unwrap_or_else(|e| {
                    log::error!("Could not work out reply recipients: {e:#}");
                    HashSet::new()
                })
        } else {
            HashSet::new()
        };
        let everyone: Vec<uuid::Uuid> = if is_reply {
            let connected = crate::push::lock(&crate::PUSH_NOTIFS).connected_members();
            Self::members_with_access(&mut conn, connected, board_id)
                .await
                .into_iter()
                .filter(|id| *id != this_author)
                .collect()
        } else {
            vec![]
        };

        // How each member should hear about this post depends on how closely they are paying
        // attention, read in the same breath as the publish so the answer matches the moment
        // the post went out:
        //
        //   reading this thread  -> nothing at all: no notification, no push, and it counts as
        //                           read (they have it on screen)
        //   active on the site   -> a notification in the page, no push
        //   away                 -> a notification in the page for when they return, and a push
        //
        // Anyone with the thread open gets the post live regardless; that is the thread
        // itself updating, not a notification, and it travels by the thread audience.
        let (reading, away) = {
            let mut hub = crate::push::lock(&crate::PUSH_NOTIFS);
            let at = std::time::Instant::now();
            let mut reading = HashSet::new();
            let mut away = HashSet::new();
            for id in watchers.iter().map(|w| w.id).chain(repliers.iter().copied()) {
                match hub.attention(&id, &key, at) {
                    crate::push::Attention::Reading => {
                        reading.insert(id);
                    }
                    crate::push::Attention::Active => {}
                    crate::push::Attention::Away => {
                        away.insert(id);
                    }
                }
            }
            hub.publish(
                common::structs::PushMessage::NewPost(Arc::new(safe.clone())),
                Some(key.clone()),
                watchers
                    .iter()
                    .map(|w| w.id)
                    .chain(everyone.iter().copied())
                    .filter(|id| !reading.contains(id))
                    .collect(),
            );
            (reading, away)
        };

        if !reading.is_empty() {
            if let Err(e) =
                Self::advance_read(&mut conn, thread, &reading, safe.post_number).await
            {
                log::warn!("could not mark a post read for those reading it: {e:#}");
            }
        }

        let thread_topic = Self::get_raw_thread(
            &mut conn,
            &safe.board_discriminator,
            safe.thread_post_number,
        )
        .await
        .map(|x| x.topic)
        .unwrap_or_default();
        let (watch_payload, reply_payload) = match (
            build_push_payload(&safe, &thread_topic, false),
            build_push_payload(&safe, &thread_topic, true),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(e), _) | (_, Err(e)) => {
                log::error!("Error serializing push payload: {}", e);
                return;
            }
        };

        // Someone reading the thread on any of their devices does not need a push on all the
        // others for a post they are looking at. A member who is both watching and being
        // replied to gets the reply version, once.
        let mut targets = Vec::new();
        for id in &away {
            let reply = repliers.contains(id);
            match database::get_push_subscriptions(&mut conn, id).await {
                Ok(subs) => targets.extend(subs.into_iter().map(|(row, sub)| (row, sub, reply))),
                Err(e) => log::error!("Error loading push subscriptions: {}", e),
            }
        }

        // Devices are independent, so send to them together instead of one after another.
        let (watch_payload, reply_payload) = (&watch_payload, &reply_payload);
        let outcomes: Vec<_> = futures::stream::iter(targets)
            .map(|(row_id, sub, reply)| async move {
                let payload = if reply { reply_payload } else { watch_payload };
                (row_id, Self::push(&sub, payload.as_bytes()).await)
            })
            .buffer_unordered(PUSH_CONCURRENCY)
            .collect()
            .await;

        let mut delivered = 0;
        for (row_id, outcome) in outcomes {
            match outcome {
                Ok(()) => delivered += 1,
                // Only a verdict that the endpoint is gone justifies forgetting it. A timeout
                // or a 5xx says nothing about the device, and deleting on those silently
                // ended notifications for everyone whenever a push service had a bad minute.
                Err(e) if e.gone => {
                    log::info!("removing a push subscription the push service rejected: {}", e);
                    if let Err(e) = database::remove_push_subscription(&mut conn, &row_id).await {
                        log::error!("Error removing dead push subscription: {}", e);
                    }
                }
                Err(e) => log::warn!("push notification not delivered, keeping the subscription: {}", e),
            }
        }
        // Counts only: which members these were is deliberately not logged.
        log::trace!(
            "Push dispatched on thread {}: {} watchers, {} reply recipients, {} reading it, {} away, {} devices reached",
            thread,
            watchers.len(),
            repliers.len(),
            reading.len(),
            away.len(),
            delivered
        );
    }

    /// Members who opted in to reply pushes and wrote one of the posts that was replied to.
    ///
    /// This is the one place the server works out authorship for someone other than the person
    /// asking, so it follows the rules in CLAUDE.md: the answer is used to decide who to push
    /// and then dropped. It is never stored, logged or returned, and nothing exposes it.
    async fn reply_recipients(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        targets: &[i64],
        this_author: uuid::Uuid,
        board_id: i64,
    ) -> Result<std::collections::HashSet<uuid::Uuid>> {
        use database::posts::dsl as p;

        let tags: Vec<(i64, String)> = p::posts
            .filter(p::id.eq_any(targets))
            .select((p::id, p::actual_author))
            .load(conn)
            .await?;
        let mut out = std::collections::HashSet::new();
        if tags.is_empty() {
            return Ok(out);
        }

        for member in database::reply_push_members(conn).await? {
            if member.id == this_author {
                continue; // answering your own post is not news
            }
            let wrote_one = tags
                .iter()
                .any(|(id, tag)| *tag == database::keys::post_author_hash(&member.author_seed, *id));
            if !wrote_one {
                continue;
            }
            // They may have lost access, or be banned, since they opted in.
            let ban = database::resolve_ban(conn, &member.id)
                .await
                .unwrap_or(database::Ban::None);
            let viewer = member.viewer_with(ban);
            if database::check_access(conn, &viewer, board_id)
                .await
                .unwrap_or(false)
            {
                out.insert(member.id);
            }
        }
        Ok(out)
    }

    /// Of `ids`, the members who may see board `board_id`. A public board is open to everyone.
    async fn members_with_access(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        ids: Vec<uuid::Uuid>,
        board_id: i64,
    ) -> Vec<uuid::Uuid> {
        let private = {
            use database::boards::dsl::*;
            boards
                .filter(id.eq(board_id))
                .select(private)
                .first::<bool>(conn)
                .await
                .unwrap_or(true) // if it cannot be read, assume the stricter case
        };
        if !private {
            return ids;
        }
        let mut allowed = Vec::new();
        for id in ids {
            let Ok(Some(member)) = database::get_member(conn, &id).await else {
                continue;
            };
            let ban = database::resolve_ban(conn, &id)
                .await
                .unwrap_or(database::Ban::None);
            if database::check_access(conn, &member.viewer_with(ban), board_id)
                .await
                .unwrap_or(false)
            {
                allowed.push(id);
            }
        }
        allowed
    }

    pub async fn get_all_files(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<FileInfo>> {
        use database::files::dsl::*;
        let filelist = files.load::<database::File>(conn).await?;
        let mut newlist = vec![];
        for file in filelist {
            newlist.push(
                file.info(conn, Some(&database::Viewer::system()))
                    .await?,
            );
        }
        Ok(newlist)
    }

    pub async fn get_file(
        path: &str,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<FileInfo> {
        use database::files::dsl::*;
        let file = files
            .filter(filepath.eq(path))
            .first::<database::File>(conn)
            .await
            .map_err(|_| anyhow!("File not found"))?;
        file.info(conn, Some(&database::Viewer::system()))
            .await
    }

    pub async fn get_random_banner(
        board_discriminator: &str,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
    ) -> Result<common::structs::Banner> {
        use database::banners::dsl::*;
        use diesel::query_dsl::methods::OrFilterDsl;
        let board = Self::get_raw_board(conn, board_discriminator).await?;

        database::check_access(conn, viewer, board.id)
            .await
            .inspect_err(|e| log::error!("Error checking access: {}", e))
            .and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(anyhow!("Not authorized to view board"))
                }
            })?;

        // An empty board list means every board. Rows created before assets were manageable
        // express that as NULL, so both are accepted.
        let banner = banners
            .or_filter(boards.contains(vec![board.id]))
            .or_filter(boards.is_null())
            .or_filter(boards.eq(Some(Vec::<i64>::new())))
            .load::<Banner>(conn)
            .await?;

        Ok(banner
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| anyhow!("No banners found for board {}!", board_discriminator))?
            .safe())
    }

    // is_valid_token is gone. A credential no longer resolves to a member by hashing it;
    // the session table does that. See crate::session.

    /// Members watching `thread_id` who may actually see the board it is on.
    pub async fn get_subscribed_users(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        thread_id: i64,
    ) -> Result<Vec<database::Member>> {
        let board = {
            let bid = {
                use database::threads::dsl::*;
                threads
                    .filter(id.eq(thread_id))
                    .first::<database::Thread>(conn)
                    .await?
                    .board
            };
            {
                use database::boards::dsl::*;
                boards
                    .filter(id.eq(bid))
                    .first::<database::Board>(conn)
                    .await?
            }
        };

        let watcher_ids: Vec<uuid::Uuid> = {
            use database::member_watching::dsl as w;
            w::member_watching
                .filter(w::thread_id.eq(thread_id))
                .select(w::member_id)
                .load(conn)
                .await?
        };

        let mut users = {
            use database::members::dsl::*;
            members
                .filter(id.eq_any(&watcher_ids))
                .load::<database::Member>(conn)
                .await?
        };

        if board.private {
            let mut permitted = vec![];
            for user in users {
                let ban = match database::resolve_ban(conn, &user.id).await {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("Error resolving ban: {}", e);
                        database::Ban::None
                    }
                };
                let viewer = user.viewer_with(ban);
                let has_access = database::check_access(conn, &viewer, board.id)
                    .await
                    .inspect_err(|e| log::error!("Error checking access: {}", e))
                    .unwrap_or(false);

                if has_access {
                    permitted.push(user);
                } else {
                    // They lost access to the board, so stop watching on their behalf. Logged
                    // without naming the member: an id here plus a board would be a record of
                    // who reads what.
                    log::info!(
                        "dropping a watch on thread {} in board {}: watcher no longer has access",
                        thread_id,
                        board.discriminator
                    );
                    use database::member_watching::dsl as w;
                    diesel::delete(
                        w::member_watching
                            .filter(w::member_id.eq(user.id))
                            .filter(w::thread_id.eq(thread_id)),
                    )
                    .execute(conn)
                    .await?;
                }
            }
            users = permitted;
        }
        Ok(users)
    }

    /// Most replies one catch-up request returns.
    pub const MAX_REPLIES_PAGE: i64 = 100;

    /// Replies posted after `since` (a post id), for a browser that was away.
    ///
    /// Everyone is told the same thing, so the server never has to know who might care; the
    /// browser works that out. The asker's own posts are left out, which is the same check as
    /// deleting a post and needs nothing the asker could not already prove.
    pub async fn replies_since(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
        since: Option<i64>,
    ) -> Result<common::structs::RepliesPage> {
        use database::posts::dsl as p;
        use diesel::query_dsl::methods::LimitDsl;

        let head: Option<i64> = p::posts.select(diesel::dsl::max(p::id)).first(conn).await?;
        let head = head.unwrap_or(0);
        // A first visit has no position. Start from now rather than replaying history.
        let Some(since) = since else {
            return Ok(common::structs::RepliesPage {
                posts: vec![],
                cursor: head,
                more: false,
            });
        };

        let rows: Vec<database::Post> = p::posts
            .filter(p::id.gt(since))
            .filter(p::replies_to.ne(Vec::<i64>::new()))
            .order(p::id.asc())
            .limit(Self::MAX_REPLIES_PAGE + 1)
            .load(conn)
            .await?;
        let more = rows.len() as i64 > Self::MAX_REPLIES_PAGE;

        let mut access: std::collections::HashMap<i64, bool> = Default::default();
        let mut posts = Vec::new();
        let mut cursor = since;
        for row in rows.into_iter().take(Self::MAX_REPLIES_PAGE as usize) {
            cursor = row.id;
            if row.actual_author == *viewer.post_author_hash(row.id) {
                continue;
            }
            let allowed = match access.get(&row.board) {
                Some(a) => *a,
                None => {
                    let a = database::check_access(conn, viewer, row.board)
                        .await
                        .unwrap_or(false);
                    access.insert(row.board, a);
                    a
                }
            };
            if allowed {
                posts.push(row.safe(conn, viewer).await?);
            }
        }
        // Nothing left to page through: start from the newest post next time.
        if !more {
            cursor = cursor.max(head);
        }
        Ok(common::structs::RepliesPage { posts, cursor, more })
    }

    /// Most posts one authorship check may ask about.
    pub const MAX_AUTHORED_BATCH: usize = 200;

    /// Which of `refs` the viewer wrote. Posts that do not exist, or are on boards the viewer
    /// cannot see, are simply not in the answer.
    pub async fn authored_posts(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
        refs: Vec<PostRef>,
    ) -> Result<Vec<PostRef>> {
        use database::posts::dsl as p;

        if refs.len() > Self::MAX_AUTHORED_BATCH {
            return Err(anyhow!("too many posts in one request"));
        }

        let mut by_board: std::collections::BTreeMap<String, Vec<i64>> = Default::default();
        for r in refs {
            by_board.entry(r.board_discriminator).or_default().push(r.post_number);
        }

        let mut mine = Vec::new();
        for (discriminator, numbers) in by_board {
            let Ok(board) = Self::get_raw_board(conn, &discriminator).await else {
                continue;
            };
            if !database::check_access(conn, viewer, board.id)
                .await
                .unwrap_or(false)
            {
                continue;
            }
            let rows: Vec<(i64, i64, String)> = p::posts
                .filter(p::board.eq(board.id))
                .filter(p::post_number.eq_any(&numbers))
                .select((p::id, p::post_number, p::actual_author))
                .load(conn)
                .await?;
            for (id, number, actual) in rows {
                // The same comparison deleting a post makes.
                if actual == *viewer.post_author_hash(id) {
                    mine.push(PostRef {
                        board_discriminator: discriminator.clone(),
                        post_number: number,
                    });
                }
            }
        }
        Ok(mine)
    }

    /// Number of the newest post in a thread (0 if somehow empty).
    async fn thread_tail(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        thread_row: i64,
    ) -> Result<i64> {
        use database::posts::dsl as p;
        let tail: Option<i64> = p::posts
            .filter(p::thread.eq(thread_row))
            .select(diesel::dsl::max(p::post_number))
            .first(conn)
            .await?;
        Ok(tail.unwrap_or(0))
    }

    /// Record that the viewer has read a watched thread through post `through`. It only ever
    /// moves forward, so a late or repeated report cannot mark things unread again.
    pub async fn mark_thread_read(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        disc: &str,
        post: i64,
        viewer: &database::Viewer,
        through: i64,
    ) -> Result<()> {
        use database::member_watching::dsl::*;

        let watched = Self::get_raw_thread(conn, disc, post).await?.id;
        diesel::update(
            member_watching
                .filter(member_id.eq(viewer.id))
                .filter(thread_id.eq(watched))
                .filter(last_read.lt(through)),
        )
        .set(last_read.eq(through))
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Mark a post read for members who were reading the thread when it was made. Like every
    /// read mark it only moves forward.
    async fn advance_read(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        thread_row: i64,
        members: &std::collections::HashSet<uuid::Uuid>,
        through: i64,
    ) -> Result<()> {
        use database::member_watching::dsl::*;

        let ids: Vec<uuid::Uuid> = members.iter().copied().collect();
        diesel::update(
            member_watching
                .filter(thread_id.eq(thread_row))
                .filter(member_id.eq_any(ids))
                .filter(last_read.lt(through)),
        )
        .set(last_read.eq(through))
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Mark everything the viewer watches as read through its newest post.
    pub async fn mark_all_read(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
    ) -> Result<()> {
        use database::member_watching::dsl::*;

        let watched: Vec<i64> = member_watching
            .filter(member_id.eq(viewer.id))
            .select(thread_id)
            .load(conn)
            .await?;
        for tid in watched {
            let tail = Self::thread_tail(conn, tid).await?;
            diesel::update(
                member_watching
                    .filter(member_id.eq(viewer.id))
                    .filter(thread_id.eq(tid))
                    .filter(last_read.lt(tail)),
            )
            .set(last_read.eq(tail))
            .execute(conn)
            .await?;
        }
        Ok(())
    }

    /// The viewer's unread notifications: watched threads holding posts above the read mark,
    /// newest first, on boards they can still see.
    pub async fn get_notifications(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
    ) -> Result<Vec<ThreadNotification>> {
        let watched: Vec<(i64, i64)> = {
            use database::member_watching::dsl as w;
            w::member_watching
                .filter(w::member_id.eq(viewer.id))
                .select((w::thread_id, w::last_read))
                .load(conn)
                .await?
        };

        let mut out = Vec::new();
        for (thread_row, read_through) in watched {
            use database::posts::dsl as p;
            use database::threads::dsl as t;

            let Ok(thread) = t::threads
                .filter(t::id.eq(thread_row))
                .first::<database::Thread>(conn)
                .await
            else {
                continue;
            };
            // A watch outlives the right to see the board; do not leak what is on it.
            if !database::check_access(conn, viewer, thread.board)
                .await
                .unwrap_or(false)
            {
                continue;
            }

            // The thread's first post is not a "reply", and the read mark is a post number.
            let unread_posts = || {
                p::posts
                    .filter(p::thread.eq(thread_row))
                    .filter(p::id.ne(thread.post_id))
                    .filter(p::post_number.gt(read_through))
            };
            let unread: i64 = unread_posts()
                .select(diesel::dsl::count_star())
                .get_result(conn)
                .await?;
            if unread == 0 {
                continue;
            }
            let first = unread_posts()
                .order(p::post_number.asc())
                .first::<database::Post>(conn)
                .await?;
            let latest = unread_posts()
                .order(p::post_number.desc())
                .first::<database::Post>(conn)
                .await?;
            let latest = latest.safe(conn, viewer).await?;

            out.push(ThreadNotification {
                board_discriminator: latest.board_discriminator.clone(),
                thread_post_number: latest.thread_post_number,
                topic: thread.topic.clone(),
                unread,
                first_unread: first.post_number,
                latest,
            });
        }

        out.sort_by(|a, b| b.latest.timestamp.cmp(&a.latest.timestamp));
        out.truncate(50);
        Ok(out)
    }

    pub async fn get_watching(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        disc: &str,
        post: i64,
        viewer: &database::Viewer,
    ) -> Result<bool> {
        use database::member_watching::dsl::*;
        use diesel::dsl::{exists, select};

        let watched = Self::get_raw_thread(conn, disc, post).await?.id;

        let is_watching = select(exists(
            member_watching
                .filter(member_id.eq(viewer.id))
                .filter(thread_id.eq(watched)),
        ))
        .get_result(conn)
        .await?;

        Ok(is_watching)
    }

    pub async fn set_watching(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        disc: &str,
        post: i64,
        viewer: &database::Viewer,
        set_watching: bool,
    ) -> Result<bool> {
        use database::member_watching::dsl::*;

        let watched = Self::get_raw_thread(conn, disc, post).await?.id;

        if set_watching {
            // The membership row is the state, so re-watching is idempotent rather than a
            // read-modify-write of an array.
            // Watching starts now: what is already in the thread is not news.
            let tail = Self::thread_tail(conn, watched).await?;
            insert_into(member_watching)
                .values((member_id.eq(viewer.id), thread_id.eq(watched), last_read.eq(tail)))
                .on_conflict_do_nothing()
                .execute(conn)
                .await?;
        } else {
            diesel::delete(
                member_watching
                    .filter(member_id.eq(viewer.id))
                    .filter(thread_id.eq(watched)),
            )
            .execute(conn)
            .await?;
        }

        Ok(set_watching)
    }

    pub async fn add_user_push_url(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        viewer: &database::Viewer,
        sub: common::structs::SubscriptionData,
    ) -> Result<()> {
        database::add_push_subscription(conn, &viewer.id, &sub).await
    }

    // pub async fn dispatch_push_notifications(thread_id: i64, post: SafePost) -> Result<()> {
    //     tokio::task::spawn(async move {
    //         let mut conn = crate::POOL.get().await.unwrap_or_else(|_| {
    //             log::error!("Failed to get connection from pool!");
    //             panic!()
    //         });
    //         let users = match Self::get_subscribed_users(&mut conn, thread_id).await {
    //             Ok(x) => x,
    //             Err(e) => {
    //                 log::error!("Failed to get subscribed users: {}", e);
    //                 return;
    //             }
    //         };

    //         let payload = serde_json::to_string(&json!({
    //             "title": "New post in a thread you're watching!",
    //             "body": &format!("{}: {}", post.author, post.content),
    //         }))
    //         .unwrap();

    //         for (token, user) in users {
    //             // TODO: send push notifications to users :D
    //             // if failed to send call set_user_push_url with None
    //             if let Err(e) = Database::push(&user, payload.as_bytes()).await {
    //                 log::error!("Failed to send push notification: {}", e);
    //                 Self::set_user_push_url(&mut conn, token, None)
    //                     .await
    //                     .unwrap_or(());
    //             }
    //         }
    //     });
    //     Ok(())
    // }

    pub async fn push(
        user: &SubscriptionData,
        payload: &[u8],
    ) -> std::result::Result<(), PushError> {
        // Stored rows predate the allowlist, and the endpoint is member-supplied: never send
        // to one that fails it.
        if !database::push_endpoint_allowed(&user.endpoint) {
            return Err(PushError {
                gone: true,
                message: "endpoint is not a push service".into(),
            });
        }

        let subscription_info = web_push::SubscriptionInfo::new(
            user.endpoint.clone(),
            user.keys.p256dh.clone(),
            user.keys.auth.clone(),
        );

        let mut sig_builder = web_push::VapidSignatureBuilder::from_base64(
            &crate::config::get().vapid.private_key,
            &subscription_info,
        )
        .map_err(PushError::from)?;
        // RFC 8292 wants a contact in `sub`, and Apple's push service refuses a token without
        // one. The site's own address is a valid https contact.
        let contact = crate::config::get().public_url.clone();
        if contact.starts_with("https://") {
            sig_builder.add_claim("sub", contact);
        }

        let mut builder = web_push::WebPushMessageBuilder::new(&subscription_info);
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload);
        builder.set_vapid_signature(sig_builder.build().map_err(PushError::from)?);
        let message = builder.build().map_err(PushError::from)?;

        // One client for the process, and a deadline: the client itself never times out.
        static CLIENT: std::sync::OnceLock<web_push::IsahcWebPushClient> =
            std::sync::OnceLock::new();
        let client = match CLIENT.get() {
            Some(c) => c,
            None => {
                let fresh = web_push::IsahcWebPushClient::new().map_err(PushError::from)?;
                CLIENT.get_or_init(|| fresh)
            }
        };

        match tokio::time::timeout(PUSH_TIMEOUT, client.send(message)).await {
            Ok(result) => result.map_err(PushError::from),
            Err(_) => Err(PushError {
                gone: false,
                message: "timed out".into(),
            }),
        }
    }
}

/// How many devices one post is pushed to at once.
const PUSH_CONCURRENCY: usize = 16;
const PUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Web Push allows about 3800 bytes of payload; stay well inside it.
const PUSH_PAYLOAD_BUDGET: usize = 3000;
const PUSH_CONTENT_CHARS: usize = 200;

/// Why a push was not delivered.
#[derive(Debug)]
pub struct PushError {
    /// The push service says this subscription is permanently dead (404/410), as opposed to a
    /// transient failure. Only then may the stored row be deleted.
    pub gone: bool,
    pub message: String,
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<web_push::WebPushError> for PushError {
    fn from(e: web_push::WebPushError) -> Self {
        Self {
            gone: matches!(
                e,
                web_push::WebPushError::EndpointNotValid(_)
                    | web_push::WebPushError::EndpointNotFound(_)
            ),
            message: e.to_string(),
        }
    }
}

/// Cut `s` to at most `max` characters, marking the cut. Counts characters, not bytes, so a
/// multibyte character is never split.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The body the service worker displays. The shape (a JSON string inside `body`) is what the
/// deployed worker already parses, so it stays until the worker is replaced everywhere.
///
/// A post may be 5000 bytes but a push payload may not, and a rejected payload used to cost
/// the watcher their subscription, so the text shown is only a preview.
fn build_push_payload(safe: &SafePost, thread_topic: &str, reply: bool) -> Result<String> {
    let build = |content_chars: usize| -> Result<String> {
        let inner = serde_json::to_string(&serde_json::json!({
            "content": truncate_chars(&safe.content, content_chars),
            "author": truncate_chars(&safe.author.to_string(), 30),
            "board_discriminator": safe.board_discriminator,
            "thread_post_number": safe.thread_post_number,
            "thread_topic": truncate_chars(thread_topic, 100),
            "post_number": safe.post_number,
            "thumbnail": safe.file.as_ref().map(|x| x.claimed.thumbnail.clone()),
            "reply": reply,
        }))?;
        Ok(serde_json::to_string(&serde_json::json!({
            "title": if reply {
                "Someone replied to your post!"
            } else {
                "New post in a thread you're watching!"
            },
            "body": inner,
        }))?)
    };
    let payload = build(PUSH_CONTENT_CHARS)?;
    if payload.len() <= PUSH_PAYLOAD_BUDGET {
        return Ok(payload);
    }
    // Escaping can multiply the size of unusual text. Fall back to no preview at all rather
    // than send something the push service will refuse.
    build(0)
}

#[cfg(test)]
mod push_tests {
    use super::*;

    #[test]
    fn truncation_never_splits_a_character() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("日本語のテキスト", 4), "日本語…");
        assert_eq!(truncate_chars("😀😀😀😀", 3), "😀😀…");
        assert_eq!(truncate_chars("anything", 0), "…");
    }

    #[test]
    fn gone_means_404_or_410_only() {
        use web_push::WebPushError::*;
        let info = |code: u16| -> web_push::WebPushError {
            let info = serde_json::from_value(serde_json::json!({
                "code": code, "errno": 0, "error": "", "message": ""
            }))
            .unwrap();
            if code == 410 {
                EndpointNotValid(info)
            } else if code == 404 {
                EndpointNotFound(info)
            } else {
                ServerError {
                    retry_after: None,
                    info,
                }
            }
        };
        assert!(PushError::from(info(410)).gone);
        assert!(PushError::from(info(404)).gone);
        assert!(!PushError::from(info(503)).gone);
        assert!(!PushError::from(Unspecified).gone);
        assert!(!PushError::from(PayloadTooLarge).gone);
    }

    #[test]
    fn payload_stays_inside_the_budget_for_the_longest_post() {
        let long = "日".repeat(5000);
        let post: SafePost = serde_json::from_value(serde_json::json!({
            "post_number": 1, "file": null, "thread_post_number": 1,
            "board_discriminator": "b", "author": {"Named": "x".repeat(30)},
            "content": long, "timestamp": "2026-01-01T00:00:00Z", "replies": []
        }))
        .unwrap();
        for reply in [false, true] {
            let payload = build_push_payload(&post, &"t".repeat(100), reply).unwrap();
            assert!(payload.len() <= PUSH_PAYLOAD_BUDGET, "{} bytes", payload.len());
        }
    }
}

#[cfg(test)]
mod db_tests;
