use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::Extension;
use axum::{
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use sqlx::PgPool;

use repositories::TodoRepository;

use crate::repositories::TodoRepositoryForDb;

mod handlers;
mod repositories;

async fn root() -> &'static str {
    "Hello, world!"
}

fn setup_logging() {
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
}

fn set_dotenv_vars() {
    dotenv().ok();
}

async fn create_db_conn(db_url: &str) -> PgPool {
    PgPool::connect(db_url)
        .await
        .expect("Can not connect to database")
}

fn create_app<R>(repo: R) -> Router
where
    R: TodoRepository,
{
    Router::new()
        .route("/", get(root))
        .route(
            "/todos",
            post(handlers::create_todo::<R>).get(handlers::all_todo::<R>),
        )
        .route(
            "/todos/:id",
            get(handlers::find_todo::<R>)
                .delete(handlers::delete_todo::<R>)
                .patch(handlers::update_todo::<R>),
        )
        .layer(Extension(Arc::new(repo)))
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
    setup_logging();
    set_dotenv_vars();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_conn = create_db_conn(&database_url).await;
    // init logging

    let repo = TodoRepositoryForDb::new(db_conn);

    let app = create_app::<TodoRepositoryForDb>(repo);
    let addr = SocketAddr::from(([127, 0, 0, 1], 8078));
    run_server(&addr, app).await;
}

#[cfg(test)]
mod tests {
    use axum::response::Response;
    use axum::{
        body::Body,
        http::{Method, Request},
    };
    use hyper::header::CONTENT_TYPE;
    use hyper::StatusCode;
    use mime::APPLICATION_JSON;
    use tower::ServiceExt;

    use crate::create_app;
    use crate::repositories::{test_repo::TodoRepositoryMemory, CreateTodo, Todo, TodoRepository};

    // Test utilities

    impl Todo {
        pub(crate) fn new(id: i32, text: &str, completed: bool) -> Todo {
            Todo {
                id,
                text: text.to_string(),
                completed,
            }
        }
    }

    struct RequestBuilder {
        uri: String,
        method: Method,
    }

    impl RequestBuilder {
        fn new(uri: &str, method: Method) -> RequestBuilder {
            RequestBuilder {
                uri: uri.to_string(),
                method,
            }
        }

        fn with_json_string(self, json_string: String) -> Request<Body> {
            Request::builder()
                .uri(self.uri)
                .header(CONTENT_TYPE, APPLICATION_JSON.as_ref())
                .method(self.method)
                .body(Body::from(json_string))
                .unwrap()
        }

        fn with_empty(&self) -> Request<Body> {
            Request::builder()
                .uri(self.uri.as_str())
                .method(self.method.as_ref())
                .body(Body::empty())
                .unwrap()
        }
    }

    async fn res_to_todo(res: Response) -> Todo {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();

        let todo: Todo = serde_json::from_str(&body)
            .unwrap_or_else(|_| panic!("failed to parse json: {}", body));
        todo
    }

    async fn res_to_todos(res: Response) -> Vec<Todo> {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();

        let todos: Vec<Todo> = serde_json::from_str(&body)
            .unwrap_or_else(|_| panic!("failed to parse json: {}", body));
        todos
    }

    // Tests

    #[tokio::test]
    async fn test_root() {
        let req = RequestBuilder::new("/", Method::GET).with_empty();
        let app = create_app::<TodoRepositoryMemory>(TodoRepositoryMemory::new());
        let res = app.oneshot(req).await.unwrap();
        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        assert_eq!(body, "Hello, world!");
    }

    #[tokio::test]
    async fn test_create_todo_route() {
        let req = RequestBuilder::new("/todos", Method::POST)
            .with_json_string(r#"{"text": "test todo"}"#.to_string());
        let repo = TodoRepositoryMemory::new();
        let app = create_app(repo);
        let res = app.oneshot(req).await.unwrap();

        let sut = res_to_todo(res).await;

        let expected = Todo::new(1, "test todo", false);
        assert_eq!(sut, expected);
    }

    #[tokio::test]
    async fn test_find_todo_by_id_route() {
        // Given a todo in the repository as memory
        let repo = TodoRepositoryMemory::new();
        let c_todo = CreateTodo::new("test todo".to_string());
        let todo_registered = repo.create(c_todo).await.expect("failed to create todo");

        // When a request is made to find the todo by id
        let req = RequestBuilder::new("/todos/1", Method::GET).with_empty();
        let app = create_app(repo);
        let res = app.oneshot(req).await.unwrap();
        let result_response = res_to_todo(res).await;

        // then
        assert_eq!(result_response, todo_registered);
    }

    #[tokio::test]
    async fn test_all_todos_route() {
        // Given a todo in the repository as memory
        let repo = TodoRepositoryMemory::new();
        let c_todo = CreateTodo::new("test todo".to_string());
        let todo_registered = repo.create(c_todo).await.expect("Failed to create todo");
        let c_todo2 = CreateTodo::new("test todo2".to_string());
        let todo_registered2 = repo.create(c_todo2).await.expect("Failed to create todo");

        // When a request is made to find the todo by id
        let req = RequestBuilder::new("/todos", Method::GET).with_empty();
        let app = create_app(repo);
        let res = app.oneshot(req).await.unwrap();
        let result_response = res_to_todos(res).await;

        // then
        assert_eq!(result_response, vec![todo_registered, todo_registered2]);
    }

    #[tokio::test]
    async fn test_delete_todo_route() {
        // Given a todo in the repository as memory
        let repo = TodoRepositoryMemory::new();
        let c_todo = CreateTodo::new("test todo".to_string());
        let _todo_registered = repo.create(c_todo).await.expect("Failed to create todo");

        // When a delete request made with path param id=1
        let req = RequestBuilder::new("/todos/1", Method::DELETE).with_empty();
        let app = create_app(repo);
        let res = app.clone().oneshot(req).await.unwrap();

        // then
        assert_eq!(StatusCode::NO_CONTENT, res.status());

        // and with not found request
        let req = RequestBuilder::new("/todos/2", Method::DELETE).with_empty();
        let res = app.oneshot(req).await.unwrap();
        // then
        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, res.status());
    }

    #[tokio::test]
    async fn update_todo_route() {
        // Given a todo in the repository as memory
        let repo = TodoRepositoryMemory::new();
        let c_todo = CreateTodo::new("test todo".to_string());
        let _todo_registered = repo.create(c_todo).await.expect("Failed to create todo");

        // When a delete request made with path param id=1
        let req = RequestBuilder::new("/todos/1", Method::PATCH)
            .with_json_string(r#"{"text": "test todo updated"}"#.to_string());
        let app = create_app(repo);
        let res = app.clone().oneshot(req).await.unwrap();

        // then
        assert_eq!(StatusCode::CREATED, res.status());

        // and with not found request
        let req = RequestBuilder::new("/todos/2", Method::PATCH)
            .with_json_string(r#"{"text": "test todo updated"}"#.to_string());
        let res = app.oneshot(req).await.unwrap();
        // then
        assert_eq!(StatusCode::NOT_FOUND, res.status());
    }
}
