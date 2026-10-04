//! Client for the external TTS server.

use std::sync::Arc;

use crate::state::AppState;

pub(super) enum TtsError {
    /// Could not talk to the server at all, or it answered 5xx: not the chunk's fault.
    Unavailable(String),
    /// The server refused this particular text.
    Rejected(String),
}

pub(super) async fn call_tts(
    state: &Arc<AppState>,
    url: &str,
    text: &str,
) -> Result<(Vec<u8>, String), TtsError> {
    let resp = state
        .http_client
        .post(url)
        .json(&serde_json::json!({ "text": text }))
        .send()
        .await
        .map_err(|e| TtsError::Unavailable(format!("TTS-сервер недоступен: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let body: String = body.chars().take(300).collect();
        let msg = format!("TTS-сервер ответил {}: {}", status, body);
        return Err(if status.is_server_error() {
            TtsError::Unavailable(msg)
        } else {
            TtsError::Rejected(msg)
        });
    }

    let ext = content_type_to_ext(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("audio/mpeg"),
    );

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| TtsError::Unavailable(format!("TTS-сервер оборвал ответ: {e}")))?;
    if bytes.is_empty() {
        return Err(TtsError::Rejected("TTS-сервер вернул пустой ответ".into()));
    }
    Ok((bytes.to_vec(), ext.to_string()))
}

fn content_type_to_ext(ct: &str) -> &'static str {
    if ct.contains("ogg") || ct.contains("vorbis") {
        "ogg"
    } else if ct.contains("wav") {
        "wav"
    } else if ct.contains("aac") {
        "aac"
    } else if ct.contains("opus") {
        "opus"
    } else {
        "mp3"
    }
}
