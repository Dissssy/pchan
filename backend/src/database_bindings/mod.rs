// use std::io::{Read, Write};

use std::sync::Arc;

use anyhow::{anyhow, Result};
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::now;
use diesel::insert_into;
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::methods::OrderDsl;
use diesel::BoolExpressionMethods as _;
use diesel::ExpressionMethods;
use diesel::PgArrayExpressionMethods;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};
use profanity::replace_possible_profanity;
use web_push::WebPushClient as _;
// use serde_json::json;
// use web_push::SubscriptionInfo;
// use web_push::VapidSignatureBuilder;
// use web_push::WebPushClient;
// use web_push::WebPushMessageBuilder;

use crate::filters::MemberToken;
use common::structs::SubscriptionData;
// use crate::endpoints::api::SubscriptionData;
use database::thread_post_number;
use database::Banner;
use database::Post;
use database::UserTag;

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
        token: MemberToken,
    ) -> Result<Vec<database::Board>> {
        use database::boards::dsl::*;

        let results: Vec<database::Board> = boards
            .order(name.desc())
            .load::<database::Board>(&mut *conn)
            .await?;

        let mut final_results = vec![];
        for board in results {
            if Self::is_allowed_access(
                conn,
                token.clone(),
                BoardOrDiscriminator::Board(board.clone()),
            )
            .await
            .inspect_err(|e| eprintln!("Error checking access: {}", e))
            .unwrap_or(false)
            {
                final_results.push(board);
            }
        }

        Ok(final_results)
    }

    pub async fn is_allowed_access<'a>(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
        board: BoardOrDiscriminator<'a>,
    ) -> Result<bool> {
        use database::user_tags::dsl::*;

        let board = match board {
            BoardOrDiscriminator::Discriminator(disc) => Self::get_raw_board(conn, disc).await?,
            BoardOrDiscriminator::Board(board) => board,
        };

        if !board.private {
            return Ok(true);
        }

        // let permission_level = Self::is_admin(conn, token.clone(), board.id).await?;
        let permission_level =
            database::permission_level(conn, board.id, &token.member_hash()).await?;
        if !permission_level.is_none() {
            return Ok(true);
        }

        let hash = common::hash_invitation(&token.member_hash(), board.id);

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

        // match results {
        //     Ok(_) => Ok(true),
        //     Err(diesel::NotFound) => Ok(false),
        //     Err(e) => Err(e.into()),
        // }
        Ok(exists)
    }

    pub async fn generate_board_access_code(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
        board_id: i64,
        invite_name: String,
    ) -> Result<String> {
        database::create_access(conn, invite_name.as_str(), &token.member_hash(), board_id).await
    }

    pub async fn generate_board_moderator_code(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
        board_id: i64,
        invite_name: String,
    ) -> Result<String> {
        database::create_moderation(conn, invite_name.as_str(), &token.member_hash(), board_id)
            .await
    }

    pub async fn consume_code(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
        invite_name: String,
    ) -> Result<()> {
        database::grant_access(conn, &invite_name, &token.member_hash()).await
    }

    pub async fn get_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discrim: &str,
        token: MemberToken,
    ) -> Result<database::Board> {
        use database::boards::dsl::*;

        let result = boards
            .filter(discriminator.eq(discrim))
            .first::<database::Board>(&mut *conn)
            .await?;

        Self::is_allowed_access(
            conn,
            token.clone(),
            BoardOrDiscriminator::Discriminator(discrim),
        )
        .await
        .inspect_err(|e| eprintln!("Error checking access: {}", e))
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
        token: MemberToken,
    ) -> Result<SafePost> {
        Self::is_allowed_access(
            conn,
            token.clone(),
            BoardOrDiscriminator::Discriminator(discriminator),
        )
        .await
        .inspect_err(|e| eprintln!("Error checking access: {}", e))
        .and_then(|x| {
            if x {
                Ok(())
            } else {
                Err(anyhow!("Not authorized to view post"))
            }
        })?;

        Self::get_raw_post(conn, discriminator, number)
            .await?
            .safe(conn)
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
        token: MemberToken,
    ) -> Result<i64> {
        Self::is_allowed_access(
            conn,
            token.clone(),
            BoardOrDiscriminator::Discriminator(discriminator),
        )
        .await
        .inspect_err(|e| eprintln!("Error checking access: {}", e))
        .and_then(|x| {
            if x {
                Ok(())
            } else {
                Err(anyhow!("Not authorized to delete post"))
            }
        })?;
        let tpost = Self::get_raw_post(conn, discriminator, number).await?;
        let tauthor = tpost.actual_author == *token.post_hash(&tpost.id.to_string());
        // let tadmin = Self::is_admin(conn, token, tpost.board).await?;
        let at_least_mod = !database::permission_level(conn, tpost.board, &token.member_hash())
            .await?
            .is_none();

        if !(at_least_mod || tauthor) {
            return Err(anyhow!("Not authorized to delete post"));
        }

        let tthread = Self::get_raw_thread(conn, discriminator, number).await;
        let id = tpost.id;
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
        Ok(id)
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
        file: FileInfo,
        post_id: i64,
    ) -> Result<database::File> {
        use database::files::dsl::*;

        let tf = insert_into(files)
            .values((
                filepath.eq(file.path),
                hash.eq(file.hash),
                id.eq(post_id),
                spoiler.eq(file.spoiler),
            ))
            .get_result::<database::File>(conn)
            .await?;

        Ok(tf)
    }
    pub async fn get_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: &str,
        number: i64,
        token: MemberToken,
    ) -> Result<ThreadWithPosts> {
        use database::threads::dsl::*;
        let this_board = Self::get_board(conn, discriminator, token).await?;
        let this_post = Self::get_raw_post(conn, discriminator, number).await?;
        let results = threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(this_post.id))
            .first::<database::Thread>(&mut *conn)
            .await?;
        results.with_posts(conn).await
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
        token: MemberToken,
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

        let this_board = Self::get_board(conn, tboard, token.clone()).await?;

        let mut t = insert_into(threads)
            .values((
                board.eq(this_board.id),
                post_id.eq(0),
                latest_post.eq(0),
                topic.eq(thread.topic),
            ))
            .get_result::<database::Thread>(conn)
            .await?;

        let p = match Self::create_post(conn, this_board.id, tboard, t.id, thread.post, token, None)
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
        t.with_posts(conn).await
    }

    pub async fn create_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: i64,
        discriminator: &str,
        tthread: i64,
        mut post: CreatePost,
        token: MemberToken,
        check_hash_against: Option<Vec<FileInfo>>,
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
        let this_post_number = Self::get_board(conn, discriminator, token.clone())
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
                .claim_file(&file, token.clone(), thread_post_number == this_post_number)
                .await?;

            if let Some(files_check) = check_hash_against {
                if files_check.iter().any(|x| x.hash == f.hash) {
                    return Err(anyhow::anyhow!("File already exists"));
                }
            }

            Some(f)
        } else {
            None
        };

        let at_least_mod = !database::permission_level(conn, tboard, &token.member_hash())
            .await?
            .is_none();

        // if !Self::is_admin(conn, token.clone(), tboard).await? {
        //     post.moderator = false;
        // }

        let member_hash = token.member_hash();

        let t = insert_into(posts).values((
            post_number.eq(this_post_number),
            moderator.eq(post.moderator && at_least_mod),
            thread.eq(tthread),
            board.eq(tboard),
            author.eq(&post.author),
            content.eq(&post.content),
            replies_to.eq(replieses),
            timestamp.eq(now),
            actual_author.eq(&*member_hash),
        ));
        let p = t.get_result::<database::Post>(conn).await?;

        if let Some(f) = pending_file {
            Self::create_file(conn, f, p.id).await?;
        }

        drop(lock);

        diesel::update(posts.filter(id.eq(p.id)))
            .set(actual_author.eq(&*token.post_hash(&p.id.to_string())))
            .execute(conn)
            .await?;

        let safe = p.safe(conn).await?;
        tokio::spawn(async move {
            Self::dispatch_push_notifications(safe, tthread, &member_hash).await;
        });

        Ok(p)
    }

    pub async fn dispatch_push_notifications(
        safe: common::structs::SafePost,
        thread: i64,
        this_author: &str,
    ) {
        let mut conn = match crate::POOL.get().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error getting connection: {}", e);
                return;
            }
        };
        let mut sse = crate::PUSH_NOTIFS.lock().await;
        let raw_members = Self::get_subscribed_users(&mut conn, thread)
            .await
            .unwrap_or_default();
        let all_members = raw_members
            .iter()
            .filter(|x| x.token_hash != this_author)
            .collect::<Vec<_>>();
        let mut idents = all_members
            .iter()
            .map(|x| &x.token_hash)
            .collect::<Vec<&String>>();
        let this_thing = format!(
            "board: {} | thread: {}",
            safe.board_discriminator, safe.thread_post_number
        );
        idents.push(&this_thing);
        sse.send_to(
            idents.as_slice(),
            common::structs::PushMessage::NewPost(Arc::new(safe.clone())),
        );

        let thread_topic = Self::get_raw_thread(
            &mut conn,
            &safe.board_discriminator,
            safe.thread_post_number,
        )
        .await
        .map(|x| x.topic)
        .unwrap_or_default();

        use database::members::dsl::*;

        let payload = match serde_json::to_string(&serde_json::json!({
            "title": "New post in a thread you're watching!",
            "body": serde_json::to_string(&serde_json::json!(
                {
                    "content": safe.content,
                    "author": format!("{}", safe.author),
                    "board_discriminator": safe.board_discriminator,
                    "thread_post_number": safe.thread_post_number,
                    "thread_topic": thread_topic,
                    "post_number": safe.post_number,
                    "thumbnail": safe.file.map(|x| x.thumbnail),
                }
            )).unwrap_or_default(),
        })) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Error serializing push payload: {}", e);
                return;
            }
        };

        for member in &all_members {
            let mut pushy = vec![];
            let changed = match member.parse_push_data() {
                Ok(v) => {
                    let mut changed = false;
                    for push in v {
                        match Self::push(&push, payload.as_bytes()).await {
                            Ok(_) => {
                                pushy.push(push);
                            }
                            Err(_e) => {
                                // eprintln!("Error sending push notification: {}", e);
                                changed = true;
                            }
                        }
                    }
                    changed
                }
                Err(e) => {
                    eprintln!("Error parsing push data: {}", e);
                    continue;
                }
            };
            if changed {
                if let Ok(v) = serde_json::to_value(pushy) {
                    if let Err(e) = diesel::update(members.filter(id.eq(member.id)))
                        .set(push_data.eq(&v))
                        .execute(&mut conn)
                        .await
                    {
                        eprintln!("Error updating push data: {}", e);
                    }
                }
            }
        }
        // println!(
        //     "Push notifications dispatched for {} users on thread {}",
        //     all_members.len(),
        //     thread
        // );
    }

    pub async fn get_all_files(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<FileInfo>> {
        use database::files::dsl::*;
        let filelist = files
            .load::<database::File>(conn)
            .await?
            .iter()
            .map(|x| x.raw_info())
            .collect::<Vec<FileInfo>>();
        Ok(filelist)
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
        file.info(conn).await
    }

    pub async fn get_random_banner(
        board_discriminator: &str,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
    ) -> Result<common::structs::Banner> {
        use database::banners::dsl::*;
        use diesel::query_dsl::methods::OrFilterDsl;
        let board = Self::get_raw_board(conn, board_discriminator).await?;

        Self::is_allowed_access(conn, token, BoardOrDiscriminator::Board(board.clone()))
            .await
            .inspect_err(|e| eprintln!("Error checking access: {}", e))
            .and_then(|x| {
                if x {
                    Ok(())
                } else {
                    Err(anyhow!("Not authorized to view board"))
                }
            })?;

        let banner = banners
            .or_filter(boards.contains(vec![board.id]))
            .or_filter(boards.is_null())
            .load::<Banner>(conn)
            .await?;

        use rand::seq::SliceRandom;
        Ok(banner
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| anyhow!("No banners found for board {}!", board_discriminator))?
            .safe())
    }

    pub async fn is_valid_token(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
    ) -> Result<bool> {
        use database::members::dsl::*;
        // println!("Checking token: {}", token.member_hash());
        // check if a user with this token exists
        let user = members
            .filter(token_hash.eq(&*token.member_hash()))
            .first::<database::Member>(conn)
            .await;

        Ok(user.is_ok())
    }

    pub async fn add_token(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
    ) -> Result<()> {
        use database::members::dsl::*;

        // check if a user with this token exists
        if Self::is_valid_token(conn, token.clone()).await? {
            return Ok(());
        };

        // if not, create one
        diesel::insert_into(members)
            .values((token_hash.eq(&*token.member_hash()),))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn remove_token(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
    ) -> Result<()> {
        use database::members::dsl::*;

        // check if a user with this token exists
        if !Self::is_valid_token(conn, token.clone()).await? {
            return Ok(());
        };

        // if so, delete it
        diesel::delete(members.filter(token_hash.eq(&*token.member_hash())))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn sync_tokens(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tokens: Vec<MemberToken>,
    ) -> Result<()> {
        use database::members::dsl::*;

        let hashed_tokens = tokens
            .iter()
            .map(|x| (*x.member_hash()).clone())
            .collect::<Vec<String>>();

        // delete all tokens that are not in the list
        diesel::delete(members.filter(token_hash.ne_all(hashed_tokens)))
            .execute(conn)
            .await?;

        // add all tokens that are not in the database
        for token in tokens {
            Self::add_token(conn, token).await?;
        }

        Ok(())
    }

    // pub async fn set_user_push_url(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     token: MemberToken,
    //     push_url: Option<&str>,
    // ) -> Result<()> {
    //     use database::members::dsl::*;

    //     let hashed_token = common::hash_with_salt(&token, &crate::statics::TOKEN_SALT);

    //     diesel::update(members.filter(token_hash.eq(hashed_token)))
    //         .set(push_notif_url.eq(push_url))
    //         .execute(conn)
    //         .await?;

    //     Ok(())
    // }

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

        let mut users = {
            use database::members::dsl::*;

            members
                .filter(watching.contains(vec![thread_id]))
                .load::<database::Member>(conn)
                .await?
        };

        if board.private {
            use database::members::dsl::*;
            let mut new_users = vec![];
            for user in users {
                let token = Arc::new(user.token_hash.clone());
                let has_access = Self::is_allowed_access(
                    conn,
                    MemberToken::new(Arc::new("DUMMY TOKEN OBJ".to_string()), Arc::clone(&token)),
                    BoardOrDiscriminator::Board(board.clone()),
                )
                .await
                .inspect_err(|e| eprintln!("Error checking access: {}", e))
                .unwrap_or(false);

                if has_access {
                    new_users.push(user);
                } else {
                    // disable watching for this thread on this user
                    println!("Disabling watching for user: {} because they don't have access to board: {}", user.id, board.discriminator);
                    let mut twatching = user.watching;
                    twatching.retain(|x| *x != thread_id);
                    diesel::update(members.filter(token_hash.eq(&*token)))
                        .set(watching.eq(twatching))
                        .execute(conn)
                        .await?;
                }
            }
            users = new_users;
        }
        Ok(users)
    }

    pub async fn get_watching(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        disc: &str,
        post: i64,
        token: MemberToken,
    ) -> Result<bool> {
        use database::members::dsl::*;
        let watching_id = Self::get_raw_thread(conn, disc, post).await?.id;

        // if user where token_hash == token && watching.contains(post.thread_id)
        match members
            .filter(token_hash.eq(&*token.member_hash()))
            .filter(watching.contains(vec![watching_id]))
            .first::<database::Member>(conn)
            .await
        {
            Ok(_) => Ok(true),
            Err(diesel::NotFound) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn set_watching(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        disc: &str,
        post: i64,
        token: MemberToken,
        set_watching: bool,
    ) -> Result<bool> {
        use database::members::dsl::*;
        let watching_id = Self::get_raw_thread(conn, disc, post).await?.id;

        // get user and, put or remove post.id from watching depending on watching
        let user = members
            .filter(token_hash.eq(&*token.member_hash()))
            .first::<database::Member>(conn)
            .await?;

        match (set_watching, user.watching.contains(&watching_id)) {
            (true, false) => {
                let mut twatching = user.watching;
                twatching.push(watching_id);
                diesel::update(members.filter(token_hash.eq(&*token.member_hash())))
                    .set(watching.eq(twatching))
                    .execute(conn)
                    .await?;
                Ok(true)
            }
            (false, true) => {
                let mut twatching = user.watching;
                twatching.retain(|x| *x != watching_id);
                diesel::update(members.filter(token_hash.eq(&*token.member_hash())))
                    .set(watching.eq(twatching))
                    .execute(conn)
                    .await?;
                Ok(false)
            }
            _ => Ok(set_watching),
        }
    }

    pub async fn add_user_push_url(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: MemberToken,
        sub: common::structs::SubscriptionData,
    ) -> Result<()> {
        use database::members::dsl::*;

        let user = members
            .filter(token_hash.eq(&*token.member_hash()))
            .first::<database::Member>(conn)
            .await?;

        // let substring = serde_json::to_string(&sub)
        //     .map_err(|_| anyhow::anyhow!("Unknown error, try again?"))?;

        // let mut subs = user.push_data;
        // // only push if not already subscribed
        // if !subs.contains(&substring) {
        //     subs.push(substring);
        //     diesel::update(members.filter(token_hash.eq(token)))
        //         .set(push_data.eq(subs))
        //         .execute(conn)
        //         .await?;
        // }

        let mut subs: Vec<SubscriptionData> = user.parse_push_data()?;
        // only push if not already subscribed
        if !subs.contains(&sub) {
            subs.push(sub);
            diesel::update(members.filter(token_hash.eq(&*token.member_hash())))
                .set(push_data.eq(serde_json::to_value(&subs)?))
                .execute(conn)
                .await?;
        }

        Ok(())
    }

    // pub async fn dispatch_push_notifications(thread_id: i64, post: SafePost) -> Result<()> {
    //     tokio::task::spawn(async move {
    //         let mut conn = crate::POOL.get().await.unwrap_or_else(|_| {
    //             eprintln!("Failed to get connection from pool!");
    //             panic!()
    //         });
    //         let users = match Self::get_subscribed_users(&mut conn, thread_id).await {
    //             Ok(x) => x,
    //             Err(e) => {
    //                 eprintln!("Failed to get subscribed users: {}", e);
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
    //                 eprintln!("Failed to send push notification: {}", e);
    //                 Self::set_user_push_url(&mut conn, token, None)
    //                     .await
    //                     .unwrap_or(());
    //             }
    //         }
    //     });
    //     Ok(())
    // }

    pub async fn push(user: &SubscriptionData, payload: &[u8]) -> Result<()> {
        let subscription_info = web_push::SubscriptionInfo::new(
            user.endpoint.clone(),
            user.keys.p256dh.clone(),
            user.keys.auth.clone(),
        );

        let sig_builder = web_push::VapidSignatureBuilder::from_base64(
            env!("VAPID_PRIVATE_KEY"),
            web_push::URL_SAFE_NO_PAD,
            &subscription_info,
        )?;

        let mut builder = web_push::WebPushMessageBuilder::new(&subscription_info);
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload);
        builder.set_vapid_signature(sig_builder.build()?);

        let client = web_push::IsahcWebPushClient::new()?;

        client.send(builder.build()?).await?;
        Ok(())
    }
}

pub enum BoardOrDiscriminator<'a> {
    Board(database::Board),
    Discriminator(&'a str),
}
