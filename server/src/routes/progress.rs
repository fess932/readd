use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::{auth::Claims, error::AppError, state::AppState};

pub async fn last(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Value>, AppError> {
    let row = sqlx::query!(
        "SELECT p.book_id, p.chapter_path, p.position_sec,
                b.title, b.author, b.cover_path, p.updated_at
         FROM progress p
         JOIN books b ON p.book_id = b.id
         JOIN user_library ul ON ul.user_id = p.user_id AND ul.book_id = p.book_id
         WHERE p.user_id = ?
         ORDER BY p.updated_at DESC
         LIMIT 1",
        claims.id
    )
    .fetch_optional(&state.pool)
    .await?;

    match row {
        None => Ok(Json(Value::Null)),
        Some(r) => Ok(Json(json!({
            "bookId": r.book_id,
            "chapterPath": r.chapter_path,
            "positionSec": r.position_sec,
            "title": r.title,
            "author": r.author,
            "coverPath": r.cover_path,
            "updatedAt": r.updated_at,
        }))),
    }
}

pub async fn get_book(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query!(
        "SELECT chapter_path, position_sec FROM progress
         WHERE user_id = ? AND book_id = ?",
        claims.id,
        book_id
    )
    .fetch_all(&state.pool)
    .await?;

    let result: Vec<Value> = rows
        .into_iter()
        .map(|r| json!({ "chapterPath": r.chapter_path, "positionSec": r.position_sec }))
        .collect();

    Ok(Json(json!(result)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBody {
    pub chapter_path: String,
    pub position_sec: f64,
    pub chapter_duration: Option<f64>,
}

pub async fn save(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(book_id): Path<i64>,
    Json(body): Json<SaveBody>,
) -> Result<Json<Value>, AppError> {
    if !body.position_sec.is_finite() {
        return Err(AppError::BadRequest("positionSec must be a number".into()));
    }
    let position_sec = body.position_sec.max(0.0);

    let chapter = sqlx::query!(
        "SELECT id AS \"id!\" FROM chapters WHERE book_id = ? AND file_path = ?",
        book_id,
        body.chapter_path
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query!(
        "INSERT INTO progress (user_id, book_id, chapter_path, position_sec, updated_at)
         VALUES (?, ?, ?, ?, datetime('now'))
         ON CONFLICT(user_id, book_id, chapter_path) DO UPDATE SET
           position_sec = excluded.position_sec,
           updated_at   = datetime('now')",
        claims.id,
        book_id,
        body.chapter_path,
        position_sec
    )
    .execute(&state.pool)
    .await?;

    // The player reports the duration; keep it only where the server could not probe one
    if let Some(dur) = body.chapter_duration.filter(|d| d.is_finite() && *d > 0.0) {
        sqlx::query!(
            "UPDATE chapters SET duration_sec = ? WHERE id = ? AND duration_sec IS NULL",
            dur,
            chapter.id
        )
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing;

    fn body(
        chapter_path: &str,
        position_sec: f64,
        chapter_duration: Option<f64>,
    ) -> Json<SaveBody> {
        Json(SaveBody {
            chapter_path: chapter_path.into(),
            position_sec,
            chapter_duration,
        })
    }

    async fn duration(state: &AppState, path: &str) -> Option<f64> {
        sqlx::query_scalar("SELECT duration_sec FROM chapters WHERE file_path = ?")
            .bind(path)
            .fetch_one(&state.pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn save_upserts_one_row_per_chapter() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3", "2.mp3"]).await;
        let ch1 = format!("book-{id}/1.mp3");

        save(
            State(t.state.clone()),
            user.clone(),
            Path(id),
            body(&ch1, 10.0, None),
        )
        .await
        .unwrap();
        save(
            State(t.state.clone()),
            user.clone(),
            Path(id),
            body(&ch1, 25.0, None),
        )
        .await
        .unwrap();

        let Json(rows) = get_book(State(t.state.clone()), user, Path(id))
            .await
            .unwrap();
        assert_eq!(rows, json!([{ "chapterPath": ch1, "positionSec": 25.0 }]));
    }

    #[tokio::test]
    async fn save_rejects_a_chapter_of_another_book() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let mine = testing::book(&t.state, &user, "Моя", &["1.mp3"]).await;
        let other = testing::book(&t.state, &user, "Чужая", &["1.mp3"]).await;
        let foreign = format!("book-{other}/1.mp3");

        let res = save(
            State(t.state.clone()),
            user,
            Path(mine),
            body(&foreign, 1.0, Some(999.0)),
        )
        .await;

        assert!(matches!(res, Err(AppError::NotFound)));
        assert_eq!(duration(&t.state, &foreign).await, None);
        assert_eq!(testing::count(&t.state, "progress").await, 0);
    }

    #[tokio::test]
    async fn reported_duration_fills_a_gap_but_never_overwrites() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3"]).await;
        let ch = format!("book-{id}/1.mp3");

        save(
            State(t.state.clone()),
            user.clone(),
            Path(id),
            body(&ch, 1.0, Some(300.0)),
        )
        .await
        .unwrap();
        assert_eq!(duration(&t.state, &ch).await, Some(300.0));

        save(
            State(t.state.clone()),
            user,
            Path(id),
            body(&ch, 2.0, Some(1e9)),
        )
        .await
        .unwrap();
        assert_eq!(duration(&t.state, &ch).await, Some(300.0));
    }

    #[tokio::test]
    async fn negative_and_non_finite_positions_are_handled() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3"]).await;
        let ch = format!("book-{id}/1.mp3");

        let nan = save(
            State(t.state.clone()),
            user.clone(),
            Path(id),
            body(&ch, f64::NAN, None),
        )
        .await;
        assert!(matches!(nan, Err(AppError::BadRequest(_))));

        save(
            State(t.state.clone()),
            user.clone(),
            Path(id),
            body(&ch, -5.0, None),
        )
        .await
        .unwrap();
        let Json(rows) = get_book(State(t.state.clone()), user, Path(id))
            .await
            .unwrap();
        assert_eq!(rows[0]["positionSec"], 0.0);
    }

    #[tokio::test]
    async fn last_skips_books_removed_from_the_library() {
        let t = testing::state().await;
        let user = testing::user(&t.state, "u", false).await;
        let id = testing::book(&t.state, &user, "Книга", &["1.mp3"]).await;
        let ch = format!("book-{id}/1.mp3");
        save(
            State(t.state.clone()),
            user.clone(),
            Path(id),
            body(&ch, 7.0, None),
        )
        .await
        .unwrap();

        // progress exists, but the book is not in the library
        let Json(none) = last(State(t.state.clone()), user.clone()).await.unwrap();
        assert_eq!(none, Value::Null);

        testing::add_to_library(&t.state, &user, id).await;
        let Json(found) = last(State(t.state.clone()), user).await.unwrap();
        assert_eq!(found["bookId"], id);
        assert_eq!(found["positionSec"], 7.0);
    }
}
