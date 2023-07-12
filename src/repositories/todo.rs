use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use validator::Validate;

use crate::repositories::RepositoryError;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, FromRow)]
pub struct Todo {
    pub(crate) id: i32,
    pub(crate) text: String,
    pub(crate) completed: bool,
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
    completed: Option<bool>,
}

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, todo: CreateTodo) -> anyhow::Result<Todo>;
    async fn find(&self, id: i32) -> anyhow::Result<Todo>;
    async fn all(&self) -> anyhow::Result<Vec<Todo>>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
    async fn update(&self, id: i32, todo: UpdateTodo) -> anyhow::Result<Todo>;
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct TodoRepositoryForDb {
    pool: PgPool,
}

impl TodoRepositoryForDb {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TodoRepository for TodoRepositoryForDb {
    async fn create(&self, todo: CreateTodo) -> anyhow::Result<Todo> {
        let todo = sqlx::query_as::<_, Todo>(
            r#"
        insert into todos (text, completed) values ($1, false) returning *
        "#,
        )
        .bind(todo.text.clone())
        .fetch_one(&self.pool)
        .await?;

        tracing::debug!("todo result {:?}", todo);

        Ok(todo)
    }

    async fn find(&self, id: i32) -> anyhow::Result<Todo> {
        let find_query = r#"select * from todos where id = $1"#;
        let todo = sqlx::query_as::<_, Todo>(find_query)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(todo)
    }

    async fn all(&self) -> anyhow::Result<Vec<Todo>> {
        let all_query = r#"select * from todos order by id desc"#;
        let todos = sqlx::query_as::<_, Todo>(all_query)
            .fetch_all(&self.pool)
            .await?;
        Ok(todos)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let delete_query = r#"delete from todos where id = $1"#;
        sqlx::query(delete_query)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
                _ => RepositoryError::Unexpected(e.to_string()),
            })?;
        Ok(())
    }

    async fn update(&self, id: i32, update_todo: UpdateTodo) -> anyhow::Result<Todo> {
        let todo_to_be_updated = self.find(id).await?;

        let update_query = r#"
        update todos 
        set text = $1, completed = $2 where id = $3
        returning *
        "#;

        let todo = sqlx::query_as::<_, Todo>(update_query)
            .bind(update_todo.text.unwrap_or(todo_to_be_updated.text))
            .bind(
                update_todo
                    .completed
                    .unwrap_or(todo_to_be_updated.completed),
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(todo)
    }
}

#[cfg(test)]
pub mod test_inmemory_repo {
    use std::collections::HashMap;
    use std::sync::RwLock;
    use std::sync::RwLockWriteGuard;
    use std::sync::{Arc, RwLockReadGuard};

    use anyhow::Context;

    use super::*;

    type TodoHashMap = HashMap<i32, Todo>;

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

            let id = store.len() as i32 + 1;
            let todo = Todo::new(id, todo.text.as_ref(), false);
            store.insert(id, todo.clone());
            Ok(todo)
        }

        async fn find(&self, id: i32) -> anyhow::Result<Todo> {
            let store = self.read_store_ref();
            let todo_found = store
                .get(&id)
                .cloned()
                .ok_or(RepositoryError::NotFound(id))?;
            Ok(todo_found)
        }

        async fn all(&self) -> anyhow::Result<Vec<Todo>> {
            let store = self.read_store_ref();
            let mut res = Vec::from_iter(store.values().cloned());
            res.sort_by_key(|todo: &Todo| todo.id);
            Ok(res)
        }

        async fn delete(&self, id: i32) -> anyhow::Result<()> {
            let mut store = self.write_store_ref();
            store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
            Ok(())
        }

        async fn update(&self, id: i32, update_todo: UpdateTodo) -> anyhow::Result<Todo> {
            let mut store = self.write_store_ref();
            let todo = store.get(&id).context(RepositoryError::NotFound(id))?;

            let text = update_todo.text.unwrap_or(todo.text.clone());
            let completed = update_todo.completed.unwrap_or(todo.completed);
            let new_todo = Todo {
                id,
                text,
                completed,
            };
            store.insert(id, new_todo.clone());
            Ok(new_todo)
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
                completed: Some(true),
            },
        )
        .await
        .expect("failed to update todo");

        let todo_updated = repo.find(1).await.expect("failed to find todo");
        assert_eq!(todo_updated.text, "updated todo".to_string());
        assert!(todo_updated.completed);
    }
}

#[cfg(test)]
#[cfg(feature = "db-test")]
mod test_psql_repo {
    use std::env;

    use dotenvy::dotenv;
    use sqlx::PgPool;

    use super::*;

    fn db_url() -> String {
        dotenv().ok();
        env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    }

    async fn create_pool() -> PgPool {
        PgPool::connect(&db_url())
            .await
            .expect("failed to create pool")
    }

    #[tokio::test]
    async fn crud_scenario() {
        let pool = create_pool().await;
        let repo = TodoRepositoryForDb::new(pool);

        let todo_text = "[crud_scenario] test todo";

        // create todo
        let todo_created = repo
            .create(CreateTodo::new(todo_text.to_string()))
            .await
            .expect("failed to create todo");

        assert_eq!(todo_created.text, todo_text);
        assert!(!todo_created.completed);

        // get id = 1 todo
        let todo_found = repo
            .find(todo_created.id)
            .await
            .expect("failed to find todo");
        assert_eq!(todo_found, todo_created);

        // list all todo
        let all = repo.all().await.expect("failed to get all todo");
        let found_single = all.first().expect("failed to get first todo");
        assert_eq!(todo_created, *found_single);

        // update todo
        let updated_text = "[crud_scenario] updated todo";
        let todo_updated = repo
            .update(
                todo_created.id,
                UpdateTodo {
                    text: Some(updated_text.to_string()),
                    completed: Some(true),
                },
            )
            .await
            .expect("failed to update todo");
        assert_eq!(todo_updated.id, todo_created.id.clone());
        assert_eq!(todo_updated.text, updated_text);
        assert!(todo_updated.completed);

        //delete todo
        repo.delete(todo_created.id)
            .await
            .expect("failed to delete todo");
        let after_deleted = repo.find(todo_created.id).await;
        // becomes error to try to find after deletion.
        assert!(after_deleted.is_err());

        let todo_rows = sqlx::query(
            r#"
            select * from todos where id=$1
            "#,
        )
        .bind(todo_created.id)
        .fetch_all(&repo.pool)
        .await
        .expect("failed to fetch todo rows");
        assert!(todo_rows.is_empty());
    }
}
