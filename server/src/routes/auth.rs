use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;

use crate::{auth as jwt, error::AppError, state::AppState};

#[derive(Deserialize)]
pub struct LoginBody {
    pub name: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest("Name is required".into()));
    }

    let is_admin = name.to_lowercase() == "admin";

    let row = sqlx::query("SELECT id, name, is_admin FROM users WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await?;

    let (id, final_is_admin) = match row {
        None => {
            let r = sqlx::query("INSERT INTO users (name, is_admin) VALUES (?, ?) RETURNING id")
                .bind(&name)
                .bind(is_admin as i64)
                .fetch_one(&state.pool)
                .await?;
            let id: i64 = r.get("id");
            tracing::info!("user registered: {} (admin={})", name, is_admin);
            (id, is_admin)
        }
        Some(u) => {
            let uid: i64 = u.get("id");
            let current_admin: i64 = u.get("is_admin");
            let current_admin = current_admin != 0;
            if is_admin && !current_admin {
                sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
                    .bind(uid)
                    .execute(&state.pool)
                    .await?;
                tracing::info!("user promoted to admin: {}", name);
                (uid, true)
            } else {
                tracing::info!("user login: {} (admin={})", name, current_admin);
                (uid, current_admin)
            }
        }
    };

    let token = jwt::encode_token(id, &name, final_is_admin, &state.jwt_secret)
        .map_err(anyhow::Error::from)?;

    Ok(Json(json!({
        "token": token,
        "user": { "id": id, "name": name, "isAdmin": final_is_admin },
    })))
}

pub async fn me(claims: jwt::Claims) -> Json<serde_json::Value> {
    Json(json!({
        "user": {
            "id": claims.id,
            "name": claims.name,
            "isAdmin": claims.is_admin,
        }
    }))
}
