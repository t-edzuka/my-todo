use std::collections::BTreeMap;
use std::option::Option;

use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use validator::Validate;

use crate::repositories::label::Label;
use crate::repositories::RepositoryError;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, FromRow)]
pub struct Todo {
    pub(crate) id: i32,
    pub(crate) text: String,
    pub(crate) completed: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, FromRow)]
pub struct TodoEntity {
    pub(crate) id: i32,
    pub(crate) text: String,
    pub(crate) completed: bool,
    pub(crate) labels: Vec<Label>,
}

impl TodoEntity {
    /// Assume grouped TodoWithLabelRow by todo_id
    fn maybe_from(value: Vec<TodoWithLabelRow>) -> Option<Self> {
        let labels = value
            .iter()
            .filter_map(|row| match (row.label_id, row.label_name.clone()) {
                (Some(id), Some(name)) => Some(Label { id, name }),
                _ => None,
            })
            .collect::<Vec<Label>>();

        value.first().map(|row| TodoEntity {
            id: row.id, // id is primary key, so the first one is always the same as the rest.
            text: row.text.clone(),
            completed: row.completed,
            labels,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, FromRow)]
pub struct TodoWithLabelRow {
    // Left joined table mapping : todos.id -> labels.todo_id
    //
    // SELECT todos.*, labels.id label_id, labels.name label_name
    // FROM todos
    // LEFT OUTER JOIN todo_labels tl on todos.id = tl.todo_id
    // LEFT OUTER JOIN labels on labels.id = tl.label_id
    // WHERE todos.id=$1
    //
    id: i32,
    text: String,
    completed: bool,
    label_id: Option<i32>,
    label_name: Option<String>,
}

impl TodoWithLabelRow {
    pub fn from_entity(te: TodoEntity) -> Vec<Self> {
        te.labels
            .iter()
            .map(|label| TodoWithLabelRow {
                id: te.id,
                text: te.text.clone(),
                completed: te.completed,
                label_id: Some(label.id),
                label_name: Some(label.name.clone()),
            })
            .collect::<Vec<TodoWithLabelRow>>()
    }
}

#[test]
fn test_from_entity() {
    let te = TodoEntity {
        id: 1,
        text: "text".to_string(),
        completed: false,
        labels: vec![Label {
            id: 1,
            name: "label".to_string(),
        }],
    };
    let rows = TodoWithLabelRow::from_entity(te);
    assert_eq!(
        rows,
        vec![TodoWithLabelRow {
            id: 1,
            text: "text".to_string(),
            completed: false,
            label_id: Some(1),
            label_name: Some("label".to_string()),
        }]
    );
}

fn fold_to_entities(flatten_row: Vec<TodoWithLabelRow>) -> Vec<TodoEntity> {
    let todos_grouped_by_id = flatten_row.iter().fold(
        BTreeMap::<i32, Vec<TodoWithLabelRow>>::new(),
        |mut acc, value| {
            acc.entry(value.id)
                .or_insert_with(Vec::<TodoWithLabelRow>::new)
                .push(value.to_owned());
            acc
        },
    );

    todos_grouped_by_id
        .iter()
        .filter_map(|(_, todo_grouped)| TodoEntity::maybe_from(todo_grouped.to_owned()))
        .collect::<Vec<TodoEntity>>()
}

#[test]
fn test_fold_entities() {
    // Prepare five rows
    let mut rows = Vec::<TodoWithLabelRow>::new();
    rows.push(TodoWithLabelRow {
        id: 1,
        text: "text1".to_string(),
        completed: false,
        label_id: Some(1),
        label_name: Some("label1".to_string()),
    });
    rows.push(TodoWithLabelRow {
        id: 1,
        text: "text1".to_string(),
        completed: false,
        label_id: Some(2),
        label_name: Some("label2".to_string()),
    });
    rows.push(TodoWithLabelRow {
        id: 2,
        text: "text2".to_string(),
        completed: false,
        label_id: Some(3),
        label_name: Some("label3".to_string()),
    });
    rows.push(TodoWithLabelRow {
        id: 2,
        text: "text2".to_string(),
        completed: false,
        label_id: Some(4),
        label_name: Some("label4".to_string()),
    });
    rows.push(TodoWithLabelRow {
        id: 3,
        text: "text3".to_string(),
        completed: false,
        label_id: None,
        label_name: None,
    });

    // Then fold to entities
    let entities = fold_to_entities(rows);
    assert_eq!(entities.len(), 3);
    // Check first entity
    assert_eq!(entities[0].id, 1);
    assert_eq!(entities[0].text, "text1");
    assert!(!entities[0].completed);
    assert_eq!(
        entities[0].labels,
        vec![
            Label {
                id: 1,
                name: "label1".to_string(),
            },
            Label {
                id: 2,
                name: "label2".to_string(),
            },
        ]
    );
    // Check second entity
    assert_eq!(entities[1].id, 2);
    assert_eq!(entities[1].text, "text2");
    assert!(!entities[1].completed);
    assert_eq!(
        entities[1].labels,
        vec![
            Label {
                id: 3,
                name: "label3".to_string(),
            },
            Label {
                id: 4,
                name: "label4".to_string(),
            },
        ]
    );
    // Check third entity
    assert_eq!(entities[2].id, 3);
    assert_eq!(entities[2].text, "text3");
    assert!(!entities[2].completed);
    assert_eq!(entities[2].labels, vec![]);
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
