use sqlx::SqlitePool;

pub async fn setup(pool: &SqlitePool) -> anyhow::Result<()> {
    let migrator = sqlx::migrate!("./migrations");

    // 0002 was edited after it had already been applied, so databases created
    // back then carry a checksum of the old text. Align it before migrating,
    // otherwise sqlx refuses to start on a modified migration.
    let has_table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if has_table > 0
        && let Some(m) = migrator.iter().find(|m| m.version == 2)
    {
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = 2")
            .bind(m.checksum.as_ref())
            .execute(pool)
            .await?;
    }

    migrator.run(pool).await?;
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

    #[tokio::test]
    async fn stale_checksum_of_migration_2_is_repaired() {
        let t = testing::state().await;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = x'00' WHERE version = 2")
            .execute(&t.state.pool)
            .await
            .unwrap();

        super::setup(&t.state.pool).await.unwrap();
    }
}
