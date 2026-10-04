use sqlx::SqlitePool;

pub async fn setup(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::testing;

    #[tokio::test]
    async fn migrations_apply_to_an_empty_database_and_rerun_cleanly() {
        let t = testing::state().await; // runs setup once
        super::setup(&t.state.pool).await.unwrap();

        let applied: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success")
                .fetch_one(&t.state.pool)
                .await
                .unwrap();
        assert_eq!(
            applied as usize,
            sqlx::migrate!("./migrations").iter().count()
        );
    }
}
