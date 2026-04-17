use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, sync::Arc};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    client: Client,
    openrouter_key: String,
    together_key: String,
}

#[derive(Deserialize)]
struct TextRequest {
    prompt: String,
    model: Option<String>,
}

#[derive(Serialize)]
struct TextResponse {
    ok: bool,
    provider: String,
    model: String,
    text: String,
    raw: Value,
}

#[derive(Deserialize)]
struct ImageRequest {
    prompt: String,
    model: Option<String>,
    steps: Option<u32>,
}

#[derive(Serialize)]
struct ImageResponse {
    ok: bool,
    provider: String,
    model: String,
    image_url: String,
    raw: Value,
}

#[tokio::main]
async fn main() {
    let openrouter_key = env::var("OPENROUTER_API_KEY").unwrap_or_default();
    let together_key = env::var("TOGETHER_API_KEY").unwrap_or_default();

    let state = Arc::new(AppState {
        client: Client::new(),
        openrouter_key,
        together_key,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/text/generate", post(generate_text))
        .route("/api/image/generate", post(generate_image))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(10000);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("failed to bind address");

    println!("AI Tokens backend is running on port {}", port);

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

async fn root() -> impl IntoResponse {
    Json(json!({
        "name": "AI Tokens",
        "status": "running",
        "routes": [
            "GET /health",
            "POST /api/text/generate",
            "POST /api/image/generate"
        ]
    }))
}

async fn health() -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "service": "AI Tokens"
    }))
}

async fn generate_text(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TextRequest>,
) -> impl IntoResponse {
    if state.openrouter_key.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "ok": false,
                "error": "OPENROUTER_API_KEY not configured"
            })),
        );
    }

    let model = payload
        .model
        .unwrap_or_else(|| "openrouter/free".to_string());

    let body = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": payload.prompt
            }
        ]
    });

    let response = match state
        .client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", state.openrouter_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://ai-tokens.onrender.com")
        .header("X-Title", "AI Tokens")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": format!("OpenRouter request failed: {}", e)
                })),
            );
        }
    };

    let status = response.status();
    let raw: Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": format!("Invalid OpenRouter response: {}", e)
                })),
            );
        }
    };

    if !status.is_success() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "ok": false,
                "provider": "openrouter",
                "raw": raw
            })),
        );
    }

    let text = raw["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    (
        StatusCode::OK,
        Json(json!(TextResponse {
            ok: true,
            provider: "openrouter".to_string(),
            model,
            text,
            raw,
        })),
    )
}

async fn generate_image(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ImageRequest>,
) -> impl IntoResponse {
    if state.together_key.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "ok": false,
                "error": "TOGETHER_API_KEY not configured"
            })),
        );
    }

    let model = payload
        .model
        .unwrap_or_else(|| "black-forest-labs/FLUX.1-schnell".to_string());

    let steps = payload.steps.unwrap_or(4);

    let body = json!({
        "model": model,
        "prompt": payload.prompt,
        "steps": steps,
        "n": 1
    });

    let response = match state
        .client
        .post("https://api.together.xyz/v1/images/generations")
        .header("Authorization", format!("Bearer {}", state.together_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": format!("Together request failed: {}", e)
                })),
            );
        }
    };

    let status = response.status();
    let raw: Value = match response.json().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "ok": false,
                    "error": format!("Invalid Together response: {}", e)
                })),
            );
        }
    };

    if !status.is_success() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "ok": false,
                "provider": "together",
                "raw": raw
            })),
        );
    }

    let image_url = raw["data"][0]["url"]
        .as_str()
        .unwrap_or("")
        .to_string();

    (
        StatusCode::OK,
        Json(json!(ImageResponse {
            ok: true,
            provider: "together".to_string(),
            model,
            image_url,
            raw,
        })),
    )
}
