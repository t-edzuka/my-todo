use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use anyhow::Context;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use validator::Validate;

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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Validate)]
pub struct CreateTodo {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 288, message = "Over the text length"))]
    text: String,
}

impl CreateTodo {
    #[allow(dead_code)]
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Validate)]
pub struct UpdateTodo {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 288, message = "Over the text length"))]
    text: Option<String>,
    done: Option<bool>,
}

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Not found id: {0}")]
    NotFound(u64),
}

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, todo: CreateTodo) -> anyhow::Result<Todo>;
    async fn update(&self, id: u64, todo: UpdateTodo) -> anyhow::Result<Todo>;
    async fn delete(&self, id: u64) -> anyhow::Result<()>;
    async fn find(&self, id: u64) -> Option<Todo>;
    async fn all(&self) -> anyhow::Result<Vec<Todo>>;
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

#[async_trait]
impl TodoRepository for TodoRepositoryMemory {
    async fn create(&self, todo: CreateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();

        let id = store.len() as u64 + 1;
        let todo = Todo::new(id, todo.text);
        store.insert(id, todo.clone());
        Ok(todo)
    }

    async fn update(&self, id: u64, update_todo: UpdateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();
        let todo = store.get(&id).context(RepositoryError::NotFound(id))?;

        let text = update_todo.text.unwrap_or(todo.text.clone());
        let done = update_todo.done.unwrap_or(todo.done);
        let new_todo = Todo { id, text, done };
        store.insert(id, new_todo.clone());
        Ok(new_todo)
    }

    async fn delete(&self, id: u64) -> anyhow::Result<()> {
        let mut store = self.write_store_ref();
        store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
        Ok(())
    }

    async fn find(&self, id: u64) -> Option<Todo> {
        let store = self.read_store_ref();
        store.get(&id).cloned()
    }

    async fn all(&self) -> anyhow::Result<Vec<Todo>> {
        let store = self.read_store_ref();
        let mut res = Vec::from_iter(store.values().map(|todo| todo.clone()));
        res.sort_by_key(|todo: &Todo| todo.id);
        Ok(res)
    }
}

#[tokio::test]
async fn test_todo_repo_scenario() {
    // create todo
    let repo = TodoRepositoryMemory::new();
    let todo = repo
        .create(CreateTodo {
            text: "test todo".to_string(),
        })
        .await
        .expect("failed to create todo");
    assert_eq!(todo.id, 1);

    let todo2 = repo
        .create(CreateTodo {
            text: "test todo2".to_string(),
        })
        .await
        .expect("failed to create todo");

    assert_eq!(todo2.id, 2);

    // get id = 1 todo
    let todo_found = repo.find(1).await.expect("failed to find todo");
    assert_eq!(todo_found, todo);

    // list all todo
    let all = repo.all().await.expect("failed to get all todo");
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
    .await
    .expect("failed to update todo");

    let todo_updated = repo.find(1).await.expect("failed to find todo");
    assert_eq!(todo_updated.text, "updated todo".to_string());
    assert!(todo_updated.done);
}
