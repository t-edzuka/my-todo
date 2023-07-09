use axum::extract::Extension;
use axum::response::IntoResponse;
use axum::Json;
use axum::{
    routing::{get, post},
    Router,
};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use thiserror::Error;

async fn root() -> &'static str {
    "Hello, world!"
}

fn setup_logging() {
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
}

fn create_app<R>(repo: R) -> Router
where
    R: TodoRepository,
{
    Router::new()
        .layer(Extension(Arc::new(repo)))
        .route("/", get(root))
        .route("/todos", post(create_todo_handler::<R>))
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

    let repo = TodoRepositoryMemory::new(); // TODO: use other repository lator

    let app = create_app::<TodoRepositoryMemory>(repo);
    let addr = SocketAddr::from(([127, 0, 0, 1], 8078));

    run_server(&addr, app).await;
}

pub async fn create_todo_handler<R>(
    Extension(repo): Extension<Arc<R>>,
    Json(todo): Json<CreateTodo>,
) -> impl IntoResponse
where
    R: TodoRepository,
{
    let todo = repo.create(todo);
    (StatusCode::CREATED, Json(todo))
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Todo {
    id: u64,
    text: String,
    done: bool,
}

impl Todo {
    pub fn new(id: u64, text: String) -> Self {
        Self {
            id,
            text,
            done: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct CreateTodo {
    text: String,
}
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct UpdateTodo {
    text: Option<String>,
    done: Option<bool>,
}
#[derive(Error, Debug)]
enum RepositoryError {
    #[error("Not found id: {0}")]
    NotFound(u64),
}

pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    fn create(&self, todo: CreateTodo) -> Todo;
    fn update(&self, id: u64, todo: UpdateTodo) -> anyhow::Result<Todo>;
    fn delete(&self, id: u64) -> anyhow::Result<()>;
    fn find(&self, id: u64) -> Option<Todo>;
    fn all(&self) -> Vec<Todo>;
}

type TodoHashMap = HashMap<u64, Todo>;

#[derive(Clone, Debug)]
pub struct TodoRepositoryMemory {
    store: Arc<RwLock<TodoHashMap>>,
}

impl TodoRepositoryMemory {
    pub fn new() -> Self {
        Self {
            store: Arc::default(),
        }
    }

    fn write_store_ref(&self) -> RwLockWriteGuard<TodoHashMap> {
        self.store.write().unwrap()
    }

    fn read_store_ref(&self) -> RwLockReadGuard<TodoHashMap> {
        self.store.read().unwrap()
    }
}

impl Default for TodoRepositoryMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl TodoRepository for TodoRepositoryMemory {
    fn create(&self, todo: CreateTodo) -> Todo {
        let mut store = self.write_store_ref();

        let id = store.len() as u64 + 1;
        let todo = Todo::new(id, todo.text);
        store.insert(id, todo.clone());
        todo
    }

    fn update(&self, id: u64, update_todo: UpdateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();
        let todo = store.get_mut(&id).ok_or(RepositoryError::NotFound(id))?;
        let UpdateTodo { text, done } = update_todo;
        if let Some(text) = text {
            todo.text = text;
        }
        if let Some(done) = done {
            todo.done = done;
        }
        Ok(todo.clone())
    }

    fn delete(&self, id: u64) -> anyhow::Result<()> {
        let mut store = self.write_store_ref();
        store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
        Ok(())
    }

    fn find(&self, id: u64) -> Option<Todo> {
        let store = self.read_store_ref();
        store.get(&id).cloned()
    }

    fn all(&self) -> Vec<Todo> {
        let store = self.read_store_ref();
        let mut res = store.values().cloned().collect::<Vec<Todo>>();
        res.sort_by_key(|todo: &Todo| todo.id);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
    };

    use tower::ServiceExt;

    #[tokio::test]
    async fn test_root() {
        let req = Request::builder()
            .uri("/")
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();
        let repo = TodoRepositoryMemory::new();
        let res = create_app(repo).oneshot(req).await.unwrap();

        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(body, "Hello, world!");
    }

    #[tokio::test]
    async fn test_todo_repo_scenario() {
        // create todo
        let repo = TodoRepositoryMemory::new();
        let todo = repo.create(CreateTodo {
            text: "test todo".to_string(),
        });
        assert_eq!(todo.id, 1);

        let todo2 = repo.create(CreateTodo {
            text: "test todo2".to_string(),
        });

        assert_eq!(todo2.id, 2);

        // get id = 1 todo
        let todo_found = repo.find(1).unwrap();
        assert_eq!(todo_found, todo);

        // list all todo
        let all = repo.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], todo);
        assert_eq!(all[1], todo2);

        // update todo
        repo.update(
            1,
            UpdateTodo {
                text: Some("updated todo".to_string()),
                done: Some(true),
            },
        )
        .unwrap();

        let todo_updated = repo.find(1).unwrap();
        assert_eq!(todo_updated.text, "updated todo".to_string());
        assert!(todo_updated.done);
    }
}
