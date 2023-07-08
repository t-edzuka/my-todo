use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::env;

async fn root() -> &'static str {
    "Hello, world!"
}

#[tokio::main]
async fn main() {
    // init logging
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/", get(root));
    let addr = SocketAddr::from(([127, 0, 0, 1], 8078));

    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    
}

