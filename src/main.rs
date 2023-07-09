use axum::response::IntoResponse;
use axum::Json;
use axum::{
    routing::{get, post},
    Router,
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;

async fn root() -> &'static str {
    "Hello, world!"
}

fn setup_logging() {
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
}

fn create_app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/users", post(create_user))
}

async fn run_server(socket_addr: &SocketAddr, app: Router) {
    tracing::debug!("listening on {}", socket_addr);
    axum::Server::bind(socket_addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[tokio::main]
async fn main() {
    // init logging
    setup_logging();

    let app = create_app();
    let addr = SocketAddr::from(([127, 0, 0, 1], 8078));

    run_server(&addr, app).await;
}

async fn create_user(Json(payload): Json<CreateUser>) -> impl IntoResponse {
    let user = User::new(1, payload.name);
    (StatusCode::CREATED, Json(user))
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
struct CreateUser {
    name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
struct User {
    id: u64,
    name: String,
}

impl User {
    fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header::CONTENT_TYPE, Method, Request},
    };

    use tower::ServiceExt;

    #[tokio::test]
    async fn test_root() {
        let req = Request::builder()
            .uri("/")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();
        let res = create_app().oneshot(req).await.unwrap();

        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(body, "Hello, world!");
    }

    #[tokio::test]
    async fn test_create_user() {
        // Create reauest for user.
        let req = Request::builder()
            .header(CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .method(Method::POST)
            .uri("/users")
            .body(Body::from(r#"{ "name" : "田中 太郎"}"#))
            .unwrap();

        let res = create_app().oneshot(req).await.unwrap();

        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = std::str::from_utf8(&bytes).unwrap();
        let user = serde_json::from_str::<User>(body).expect("Cannot convert to User struct.");
        assert_eq!(
            user,
            User {
                id: 1,
                name: "田中 太郎".to_string()
            }
        );
    }
}
