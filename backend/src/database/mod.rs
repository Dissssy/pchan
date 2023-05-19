// use std::io::{Read, Write};

use std::sync::Arc;

use anyhow::{anyhow, Result};
use common::structs::*;
use deadpool::managed::Object;
use diesel::dsl::now;
use diesel::insert_into;
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::methods::OrderDsl;
use diesel::ExpressionMethods;
use diesel::PgArrayExpressionMethods;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};
use profanity::replace_possible_profanity;
// use serde_json::json;
// use web_push::SubscriptionInfo;
// use web_push::VapidSignatureBuilder;
// use web_push::WebPushClient;
// use web_push::WebPushMessageBuilder;

// use crate::endpoints::api::SubscriptionData;
use crate::schema::thread_post_number;
use crate::schema::Banner;
use crate::schema::Post;

// pub struct Users {
//     valid_users: Vec<String>,
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
//             let auth: Vec<String> = postcard::from_bytes(&bytes).unwrap_or_default();
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

//     pub async fn is_auth(&mut self, token: String) -> Result<bool> {
//         Ok(self.valid_users.contains(&token))
//     }
//     pub async fn add_auth(&mut self, token: String) -> Result<()> {
//         self.valid_users.push(token);
//         Ok(())
//     }
//     pub async fn sync_auth(&mut self, tokens: Vec<String>) -> Result<()> {
//         self.valid_users = tokens;
//         Ok(())
//     }
//     pub async fn remove_auth(&mut self, token: String) -> Result<()> {
//         self.valid_users.retain(|x| x != &token);
//         Ok(())
//     }
// }

pub struct Database;

impl Database {
    pub async fn create_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        new_discriminator: String,
        new_name: String,
    ) -> Result<()> {
        use crate::schema::boards::dsl::*;

        insert_into(boards)
            .values((discriminator.eq(new_discriminator), name.eq(new_name)))
            .execute(conn)
            .await?;
        Ok(())
    }
    pub async fn get_boards(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<crate::schema::Board>> {
        use crate::schema::boards::dsl::*;

        let results = boards
            .order(name.desc())
            .load::<crate::schema::Board>(&mut *conn)
            .await?;
        Ok(results)
    }

    pub async fn get_board(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discim: String,
    ) -> Result<crate::schema::Board> {
        use crate::schema::boards::dsl::*;

        let results = boards
            .filter(discriminator.eq(discim))
            .first::<crate::schema::Board>(&mut *conn)
            .await?;
        Ok(results)
    }

    pub async fn get_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
    ) -> Result<SafePost> {
        Self::get_raw_post(conn, discriminator, number)
            .await?
            .safe(conn)
            .await
    }

    pub async fn get_raw_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
    ) -> Result<crate::schema::Thread> {
        use crate::schema::threads::dsl::*;
        let tpost = Self::get_raw_post(conn, discriminator.clone(), number).await?;
        let this_board = Self::get_board(conn, discriminator).await?;
        Ok(threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(tpost.id))
            .first::<crate::schema::Thread>(&mut *conn)
            .await?)
    }

    // pub async fn get_post_from_id(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     id: i64,
    // ) -> Result<SafePost> {
    //     use crate::schema::posts::dsl::*;

    //     let post = posts
    //         .filter(id.eq(id))
    //         .first::<crate::schema::Post>(&mut *conn)
    //         .await?;
    //     post.safe(conn).await
    // }

    async fn get_raw_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
    ) -> Result<Post> {
        use crate::schema::posts::dsl::*;

        let this_board = Self::get_board(conn, discriminator).await?;
        Ok(posts
            .filter(board.eq(this_board.id))
            .filter(post_number.eq(number))
            .first::<crate::schema::Post>(&mut *conn)
            .await?)
    }

    pub async fn delete_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
        token: String,
    ) -> Result<i64> {
        let tpost = Self::get_raw_post(conn, discriminator.clone(), number).await?;
        let tauthor =
            tpost.actual_author == common::hash_with_salt(&token, &format!("{}", tpost.id));
        let tadmin = Self::is_admin(conn, token, tpost.board).await?;

        if !(tadmin || tauthor) {
            return Err(anyhow!("Not authorized to delete post"));
        }

        let tthread = Self::get_raw_thread(conn, discriminator, number).await;
        let id = tpost.id;
        match (tadmin, tauthor, tthread, tpost) {
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
        use crate::schema::posts::dsl::*;

        diesel::delete(posts.filter(id.eq(tid)))
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn raw_delete_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tid: i64,
    ) -> Result<()> {
        use crate::schema::threads::dsl::*;

        diesel::delete(threads.filter(id.eq(tid)))
            .execute(conn)
            .await?;
        Ok(())
    }

    async fn is_admin(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: String,
        board_id: i64,
    ) -> Result<bool> {
        use crate::schema::members::dsl::*;
        use diesel::query_dsl::methods::SelectDsl;

        let hashed_token = common::hash_with_salt(&token, &crate::statics::TOKEN_SALT);

        // get this members moderates Vec<i64>
        let status = members
            .select(moderates)
            .filter(token_hash.eq(hashed_token))
            .first::<Option<Vec<i64>>>(&mut *conn)
            .await?;

        Ok(match status {
            Some(status) => status.is_empty() || status.contains(&board_id),
            None => false,
        })
    }

    pub async fn create_file(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        file: FileInfo,
        post_id: i64,
    ) -> Result<crate::schema::File> {
        use crate::schema::files::dsl::*;

        let tf = insert_into(files)
            .values((
                filepath.eq(file.path),
                hash.eq(file.hash),
                id.eq(post_id),
                spoiler.eq(file.spoiler),
            ))
            .get_result::<crate::schema::File>(conn)
            .await?;

        Ok(tf)
    }
    pub async fn get_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discriminator: String,
        number: i64,
    ) -> Result<ThreadWithPosts> {
        use crate::schema::threads::dsl::*;
        let this_board = Self::get_board(conn, discriminator.clone()).await?;
        let this_post = Self::get_raw_post(conn, discriminator, number).await?;
        let results = threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(this_post.id))
            .first::<crate::schema::Thread>(&mut *conn)
            .await?;
        results.with_posts(conn).await
    }

    // pub async fn get_thread_from_post_number(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     bboard: i64,
    //     number: i64,
    // ) -> Result<ThreadWithPosts> {
    //     use crate::schema::threads::dsl::*;
    //     let this_post = Self::get_post_from_post_number(conn, bboard, number).await?;
    //     let results = threads
    //         .filter(board.eq(bboard))
    //         .filter(post_id.eq(this_post.id))
    //         .first::<crate::schema::Thread>(&mut *conn)
    //         .await?;
    //     results.with_posts(conn).await
    // }

    // pub async fn get_post_from_post_number(
    //     conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    //     bboard: i64,
    //     number: i64,
    // ) -> Result<crate::schema::Post> {
    //     use crate::schema::posts::dsl::*;
    //     let results = posts
    //         .filter(board.eq(bboard))
    //         .filter(post_number.eq(number))
    //         .first::<crate::schema::Post>(&mut *conn)
    //         .await?;
    //     Ok(results)
    // }

    pub async fn create_thread(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: String,
        mut thread: CreateThread,
        actual_author: String,
    ) -> Result<ThreadWithPosts> {
        use crate::schema::threads::dsl::*;

        if thread.topic.is_empty() {
            return Err(anyhow::anyhow!("No topic provided"));
        }

        thread.topic = replace_possible_profanity(thread.topic, &crate::PROFANITY, || {
            crate::QUOTES.random_quote()
        });

        if thread.post.file.is_none() {
            return Err(anyhow::anyhow!("No file provided"));
        }

        let this_board = Self::get_board(conn, tboard.clone()).await?;

        let mut t = insert_into(threads)
            .values((
                board.eq(this_board.id),
                post_id.eq(0),
                latest_post.eq(0),
                topic.eq(thread.topic),
            ))
            .get_result::<crate::schema::Thread>(conn)
            .await?;

        let p = match Self::create_post(
            conn,
            this_board.id,
            tboard,
            t.id,
            thread.post,
            actual_author,
            None,
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
        t.with_posts(conn).await
    }

    pub async fn create_post(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tboard: i64,
        discriminator: String,
        tthread: i64,
        mut post: CreatePost,
        tactual_author: String,
        check_hash_against: Option<Vec<FileInfo>>,
    ) -> Result<crate::schema::Post> {
        use crate::schema::posts::dsl::*;

        post.content = post.content.trim().to_owned();
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
        let this_post_number = Self::get_board(conn, discriminator.clone())
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
                common::structs::Reply::from_str(x, &discriminator, &thread_post_number.to_string())
            })
            .collect::<Vec<common::structs::Reply>>();
        let mut replieses = Vec::new();

        for reply in replies {
            let this_post = Self::get_raw_post(
                conn,
                reply.board_discriminator,
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
                .claim_file(
                    &file,
                    tactual_author.clone(),
                    thread_post_number == this_post_number,
                )
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

        if !Self::is_admin(conn, tactual_author.clone(), tboard).await? {
            post.code = None;
        }

        let t = insert_into(posts).values((
            post_number.eq(this_post_number),
            code.eq(post.code),
            thread.eq(tthread),
            board.eq(tboard),
            author.eq(post.author.clone()),
            content.eq(post.content.clone()),
            replies_to.eq(replieses),
            timestamp.eq(now),
            actual_author.eq(tactual_author.clone()),
        ));
        let p = t.get_result::<crate::schema::Post>(conn).await?;

        if let Some(f) = pending_file {
            crate::database::Database::create_file(conn, f, p.id).await?;
        }

        drop(lock);

        diesel::update(posts.filter(id.eq(p.id)))
            .set(actual_author.eq(common::hash_with_salt(
                &tactual_author,
                &format!("{}", p.id),
            )))
            .execute(conn)
            .await?;

        {
            let mut sse = crate::PUSH_NOTIFS.lock().await;
            let safe = p.safe(conn).await?;
            let mut idents = crate::database::Database::get_subscribed_users(conn, tthread).await?;
            idents.push(format!(
                "board: {} | thread: {}",
                safe.board_discriminator, safe.thread_post_number
            ));
            sse.send_to(
                idents.as_slice(),
                common::structs::PushMessage::NewPost(Arc::new(safe)),
            );
        }

        Ok(p)
    }

    pub async fn get_all_files(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<FileInfo>> {
        use crate::schema::files::dsl::*;
        let filelist = files
            .load::<crate::schema::File>(conn)
            .await?
            .iter()
            .map(|x| x.raw_info())
            .collect::<Vec<FileInfo>>();
        Ok(filelist)
    }

    pub async fn get_random_banner(
        board_discriminator: String,
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<common::structs::Banner> {
        use crate::schema::banners::dsl::*;
        use diesel::query_dsl::methods::OrFilterDsl;
        let board = Self::get_board(conn, board_discriminator.clone()).await?;

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

    pub async fn get_random_spoiler(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<String> {
        use crate::schema::spoilers::dsl::*;
        use diesel::query_dsl::methods::SelectDsl;

        let spoiler = spoilers.select(img).load::<String>(conn).await?;

        use rand::seq::SliceRandom;
        spoiler
            .choose(&mut rand::thread_rng())
            .cloned()
            .ok_or_else(|| anyhow!("No spoilers found!"))
    }

    pub async fn is_valid_token(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: String,
    ) -> Result<bool> {
        use crate::schema::members::dsl::*;
        let hashed_token = common::hash_with_salt(&token, &crate::statics::TOKEN_SALT);

        // check if a user with this token exists
        let user = members
            .filter(token_hash.eq(hashed_token))
            .first::<crate::schema::Member>(conn)
            .await;

        Ok(user.is_ok())
    }

    pub async fn add_token(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: String,
    ) -> Result<()> {
        use crate::schema::members::dsl::*;

        let hashed_token = common::hash_with_salt(&token, &crate::statics::TOKEN_SALT);

        // check if a user with this token exists
        if Self::is_valid_token(conn, token.clone()).await? {
            return Ok(());
        };

        // if not, create one
        diesel::insert_into(members)
            .values((token_hash.eq(hashed_token),))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn remove_token(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        token: String,
    ) -> Result<()> {
        use crate::schema::members::dsl::*;

        let hashed_token = common::hash_with_salt(&token, &crate::statics::TOKEN_SALT);

        // check if a user with this token exists
        if !Self::is_valid_token(conn, token.clone()).await? {
            return Ok(());
        };

        // if so, delete it
        diesel::delete(members.filter(token_hash.eq(hashed_token)))
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn sync_tokens(
        conn: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        tokens: Vec<String>,
    ) -> Result<()> {
        use crate::schema::members::dsl::*;

        let hashed_tokens = tokens
            .iter()
            .map(|x| common::hash_with_salt(x, &crate::statics::TOKEN_SALT))
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
    //     token: String,
    //     push_url: Option<String>,
    // ) -> Result<()> {
    //     use crate::schema::members::dsl::*;

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
    ) -> Result<Vec<String>> {
        use crate::schema::members::dsl::*;
        let users = members
            .filter(watching.contains(vec![thread_id]))
            .load::<crate::schema::Member>(conn)
            .await?;
        Ok(users
            .into_iter()
            .map(|s| s.token_hash)
            .collect::<Vec<String>>())
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

    // pub async fn push(user: &SubscriptionData, payload: &[u8]) -> Result<()> {
    //     let subscription_info = SubscriptionInfo::new(
    //         user.endpoint.clone(),
    //         user.keys.p256dh.clone(),
    //         user.keys.auth.clone(),
    //     );

    //     let sig_builder = VapidSignatureBuilder::from_base64(
    //         env!("VAPID_PRIVATE_KEY"),
    //         web_push::URL_SAFE_NO_PAD,
    //         &subscription_info,
    //     )?;

    //     let mut builder = WebPushMessageBuilder::new(&subscription_info)?;
    //     builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload);
    //     builder.set_vapid_signature(sig_builder.build()?);

    //     let client = WebPushClient::new()?;

    //     client.send(builder.build()?).await?;
    //     println!("Push notification sent!");
    //     Ok(())
    // }
}
