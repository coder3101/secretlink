use uuid::Uuid;

#[derive(Clone)]
pub struct SecretController;

#[derive(Debug)]
pub enum SecretState {
    Secret(uuid::Uuid),
    Consumed,
    Expired,
    Invalid,
}

pub struct Secret {
    pub secret: Option<String>,
    pub iv: Option<String>,
}

pub type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

pub trait SecretControl {
    async fn add(
        &self,
        tx: &mut Tx,
        secret: &str,
        iv: &str,
        expiry: Option<i32>,
    ) -> Result<Uuid, sqlx::Error>;
    async fn get_secret_for_update(
        &self,
        tx: &mut Tx,
        id: &Uuid,
    ) -> Result<Option<Secret>, sqlx::Error>;
    async fn consume_secret(&self, tx: &mut Tx, id: &Uuid) -> Result<(), sqlx::Error>;
    async fn check_state(&self, tx: &mut Tx, id: &Uuid) -> Result<SecretState, sqlx::Error>;
}

impl SecretControl for SecretController {
    #[tracing::instrument(skip(self, tx), err)]
    async fn consume_secret(&self, tx: &mut Tx, id: &Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!("UPDATE secrets SET expired = true where uuid = $1", id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self, tx), err)]
    async fn get_secret_for_update(
        &self,
        tx: &mut Tx,
        id: &Uuid,
    ) -> Result<Option<Secret>, sqlx::Error> {
        let resp = sqlx::query_as!(
            Secret,
            "SELECT secret, iv from secrets WHERE uuid = $1 AND expired = false FOR UPDATE",
            id
        )
        .fetch_optional(&mut **tx)
        .await?;

        Ok(resp)
    }

    #[tracing::instrument(skip(self, tx, secret, iv), ret, err)]
    async fn add(
        &self,
        tx: &mut Tx,
        secret: &str,
        iv: &str,
        expiry: Option<i32>,
    ) -> Result<Uuid, sqlx::Error> {
        let u = uuid::Uuid::new_v4();
        sqlx::query!(
            "INSERT INTO secrets (secret, uuid, expiry, iv) VALUES ($1, $2, $3, $4)",
            secret,
            u,
            expiry,
            iv
        )
        .execute(&mut **tx)
        .await?;
        Ok(u)
    }

    #[tracing::instrument(skip(self, tx), ret(Debug), err)]
    async fn check_state(&self, tx: &mut Tx, id: &Uuid) -> Result<SecretState, sqlx::Error> {
        let expired = sqlx::query!(
            r#"
        SELECT
            expiry,
            expired AS consumed,
            (created_at + INTERVAL '1 second' * expiry) < CURRENT_TIMESTAMP AS expired
        FROM
            secrets where uuid = $1"#,
            id
        )
        .fetch_optional(&mut **tx)
        .await?;

        // If no rows, invalid secret
        let Some(rec) = expired else {
            return Ok(SecretState::Invalid);
        };

        let resp = if let Some(true) = rec.consumed {
            SecretState::Consumed
        } else if let Some(true) = rec.expired {
            SecretState::Expired
        } else {
            SecretState::Secret(*id)
        };

        Ok(resp)
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::{SecretControl, SecretController, SecretState};
    use sqlx::PgPool;
    use tokio::time::sleep;

    #[sqlx::test]
    async fn test_add(pool: PgPool) -> sqlx::Result<()> {
        let mut tx = pool.begin().await?;

        let sc = SecretController {};
        let r = sqlx::query!("SELECT COUNT(*) FROM secrets")
            .fetch_one(&mut *tx)
            .await?;
        assert_eq!(r.count, Some(0));

        sc.add(&mut tx, "123", "123", None).await?;

        let r = sqlx::query!("SELECT COUNT(*) FROM secrets")
            .fetch_one(&mut *tx)
            .await?;
        assert_eq!(r.count, Some(1));

        Ok(())
    }

    #[sqlx::test]
    async fn test_consume(pool: PgPool) -> sqlx::Result<()> {
        let mut tx = pool.begin().await?;
        let sc = SecretController {};
        let u = sc.add(&mut tx, "123", "123", Some(1200)).await?;

        let r = sqlx::query!("SELECT COUNT(*) FROM secrets WHERE expired = true")
            .fetch_one(&mut *tx)
            .await?;

        assert_eq!(r.count, Some(0));
        sc.consume_secret(&mut tx, &u).await?;

        let r = sqlx::query!("SELECT COUNT(*) FROM secrets WHERE expired = true")
            .fetch_one(&mut *tx)
            .await?;

        assert_eq!(r.count, Some(1));
        Ok(())
    }

    #[sqlx::test]
    async fn test_check_state(pool: PgPool) -> sqlx::Result<()> {
        let mut tx = pool.begin().await?;
        let sc = SecretController {};
        let u = sc.add(&mut tx, "123", "123", Some(1)).await?;
        tx.commit().await?;

        let mut tx = pool.begin().await?;
        let r = sc.check_state(&mut tx, &u).await?;
        tx.commit().await?;

        let tx = pool.begin().await?;
        assert!(matches!(r, SecretState::Secret(_)));
        sleep(Duration::from_secs(5)).await;
        tx.commit().await?;

        let mut tx = pool.begin().await?;
        let r = sc.check_state(&mut tx, &u).await?;
        assert!(matches!(r, SecretState::Expired));
        Ok(())
    }
}
