use axum::{routing::get, Router};
use std::env;

async fn root() -> &'static str {
    "AI Tokens backend is running"
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(root));

    let port = env::var("PORT").unwrap_or_else(|_| "10000".to_string());
    let addr = format!("0.0.0.0:{port}");

    println!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
