use async_trait::async_trait;
use sqlx::{Error, Row};

use crate::repository::{LoginRepository, PgRepository};

#[async_trait]
impl LoginRepository for PgRepository {
    async fn login(&self, username: String, password: String) -> Result<u32, Error> {
        sqlx::query("SELECT account_id FROM login WHERE userid = $1 AND user_pass = $2") // TODO add bcrypt on user_pass column, but not supported by hercules
            .bind(username)
            .bind(password)
            .fetch_one(&self.pool)
            .await
            .map(|row| row.get::<i32, _>("account_id") as u32)
    }

    async fn verify_password(&self, account_id: u32, password: String) -> Result<bool, Error> {
        sqlx::query("SELECT account_id FROM login WHERE account_id = $1 AND user_pass = $2")
            .bind(account_id as i32)
            .bind(password)
            .fetch_optional(&self.pool)
            .await
            .map(|row| row.is_some())
    }
}
