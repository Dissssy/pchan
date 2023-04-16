use std::io::{Read, Write};

use anyhow::Result;
use deadpool::managed::Object;
use diesel::insert_into;
use diesel::query_dsl::methods::FilterDsl;
use diesel::ExpressionMethods;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::AsyncDieselConnectionManager, AsyncPgConnection};

pub struct Users {
    valid_users: Vec<String>,
}

impl Users {
    pub fn new() -> Result<Self> {
        Ok(Self {
            valid_users: vec![],
        })
    }
    pub async fn open(&mut self) -> Result<()> {
        if let Ok(auth) = tokio::fs::read("./auth.bin.gz").await {
            let mut bytes = vec![];
            {
                let mut auth = flate2::read::GzDecoder::new(auth.as_slice());
                let _ = auth.read_to_end(&mut bytes);
            }
            let auth: Vec<String> = postcard::from_bytes(&bytes).unwrap_or_default();
            self.valid_users = auth;
        }

        Ok(())
    }
    pub async fn close(&mut self) -> Result<()> {
        let mut auth = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        auth.write_all(&postcard::to_allocvec(&self.valid_users)?)?;
        tokio::fs::write("./auth.bin.gz", auth.finish()?).await?;
        Ok(())
    }

    pub async fn is_auth(&mut self, token: String) -> Result<bool> {
        Ok(self.valid_users.contains(&token))
    }
    pub async fn add_auth(&mut self, token: String) -> Result<()> {
        self.valid_users.push(token);
        Ok(())
    }
    pub async fn sync_auth(&mut self, tokens: Vec<String>) -> Result<()> {
        self.valid_users = tokens;
        Ok(())
    }
    pub async fn remove_auth(&mut self, token: String) -> Result<()> {
        self.valid_users.retain(|x| x != &token);
        Ok(())
    }
}

pub struct Database;

impl Database {
    pub async fn create_board(
        pool: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        new_discriminator: String,
        new_name: String,
    ) -> Result<()> {
        use crate::schema::boards::dsl::*;

        insert_into(boards)
            .values((discriminator.eq(new_discriminator), name.eq(new_name)))
            .execute(pool)
            .await?;
        Ok(())
    }
    pub async fn get_boards(
        pool: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
    ) -> Result<Vec<crate::schema::Board>> {
        use crate::schema::boards::dsl::*;

        let results = boards.load::<crate::schema::Board>(&mut *pool).await?;
        Ok(results)
    }

    pub async fn get_board(
        pool: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discim: String,
    ) -> Result<crate::schema::Board> {
        use crate::schema::boards::dsl::*;

        let results = boards
            .filter(discriminator.eq(discim))
            .first::<crate::schema::Board>(&mut *pool)
            .await?;
        Ok(results)
    }

    pub async fn get_post(
        pool: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discrim: String,
        number: i64,
    ) -> Result<crate::schema::SafePost> {
        use crate::schema::posts::dsl::*;

        let this_board = Self::get_board(pool, discrim).await?;
        let results = posts
            .filter(board.eq(this_board.id))
            .filter(post_number.eq(number))
            .first::<crate::schema::Post>(&mut *pool)
            .await?;
        Ok(results.safe())
    }

    pub async fn get_thread(
        pool: &mut Object<AsyncDieselConnectionManager<AsyncPgConnection>>,
        discrim: String,
        number: i64,
    ) -> Result<crate::schema::ThreadWithPosts> {
        use crate::schema::threads::dsl::*;

        let this_board = Self::get_board(pool, discrim.clone()).await?;
        let this_post = Self::get_post(pool, discrim, number).await?;
        let results = threads
            .filter(board.eq(this_board.id))
            .filter(post_id.eq(this_post.id))
            .first::<crate::schema::Thread>(&mut *pool)
            .await?;
        results.with_posts(pool).await
    }
}
