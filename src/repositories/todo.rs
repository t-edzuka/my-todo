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
    let mut rows = vec![];
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
    labels: Vec<i32>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Validate)]
pub struct UpdateTodo {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 288, message = "Over the text length"))]
    text: Option<String>,
    completed: Option<bool>,
    labels: Option<Vec<i32>>,
}

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, todo: CreateTodo) -> anyhow::Result<TodoEntity>;
    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity>;
    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
    async fn update(&self, id: i32, todo: UpdateTodo) -> anyhow::Result<TodoEntity>;
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
    async fn create(&self, create_todo: CreateTodo) -> anyhow::Result<TodoEntity> {
        // Todoを登録する際に、同時にLabelデータと紐づけするという実装.
        // 前提として, labelsテーブルに先にデータを登録してあることが必要で、
        // ここで行うことは todo_labelsテーブルにtodo_idとlabel_idを紐づけること
        // + todosテーブルへのデータの登録
        let tx = self.pool.begin().await?;
        //todos tableへのデータの登録.
        let todo = sqlx::query_as::<_, Todo>(
            r#"
        insert into todos (text, completed) values ($1, false) returning *
        "#,
        )
        .bind(create_todo.text.clone())
        .fetch_one(&self.pool)
        .await?;

        // todo_labels tableへのデータの登録で, labelsテーブルに登録されているデータと紐づける
        // このように展開される.
        // INSERT INTO todo_labels (todo_id, label_id)
        // SELECT 1, id
        // FROM unnest(array[1, 2, 3]) as t(id)
        sqlx::query(
            r#"
            insert into todo_labels (todo_id, label_id)
            select $1, id
            from unnest($2) as t(id);
        "#,
        )
        .bind(todo.id)
        .bind(create_todo.labels)
        .execute(&self.pool)
        .await?;

        tx.commit().await?;

        tracing::debug!("todo result {:?}", todo);

        let todo = self.find(todo.id).await?;
        Ok(todo)
    }

    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
        let find_query = r#"
        select todos.*, labels.id as label_id, labels.name as label_name 
        from todos 
        left outer join todo_labels tl on todos.id=tl.todo_id 
        left outer join labels on labels.id=tl.label_id 
        where todos.id=$1"#;
        let items = sqlx::query_as::<_, TodoWithLabelRow>(find_query)
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| match err {
                sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
                _ => RepositoryError::Unexpected(err.to_string()),
            })?;
        let todo = fold_to_entities(items)
            .into_iter()
            .next() // first rowのみ取得
            .ok_or(RepositoryError::NotFound(id))?;
        Ok(todo)
    }

    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
        let all_query = r#"
        select todos.*, labels.id as label_id, labels.name as label_name 
        from todos 
        left outer join todo_labels tl on todos.id = tl.todo_id 
        left outer join labels on labels.id = tl.label_id"#;
        let todos = sqlx::query_as::<_, TodoWithLabelRow>(all_query)
            .fetch_all(&self.pool)
            .await?;
        Ok(fold_to_entities(todos))
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
        let tx = self.pool.begin().await?;

        let old_todo = self.find(id).await?;
        sqlx::query_as::<_, Todo>(
            r#"
            update todos set text=$1, completed=$2
            where id=$3
            returning *
            "#,
        )
        .bind(payload.text.unwrap_or(old_todo.text))
        .bind(payload.completed.unwrap_or(old_todo.completed))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        // payload が labels を持っているなら交差テーブル todo_labelsをそのレコードを削除してから新しいレコードを挿入する
        // フロントエンド側では毎回更新時は既存で紐づいているラベルを含めたすべてのラベルidをこちらに送信してくることを想定されている.
        //もっと良い設計ありそうだが..
        if let Some(labels) = payload.labels {
            // 関連テーブルのレコードを一旦削除Z
            sqlx::query(
                r#"
                delete from todo_labels where todo_id = $1
                "#,
            )
            .bind(id)
            .execute(&self.pool)
            .await?;

            // 新しい label ids を insert
            sqlx::query(
                r#"
                insert into todo_labels (todo_id, label_id)
                select $1, id as label_id
                from unnest($2) as t(id);
                "#,
            )
            .bind(id)
            .bind(labels)
            .execute(&self.pool)
            .await?;
        }

        tx.commit().await?;
        let todo = self.find(id).await?;

        Ok(todo)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let tx = self.pool.begin().await?;

        // 中間テーブルの関係を外す
        sqlx::query(
            r#"
            delete from todo_labels where todo_id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        // todo の削除
        sqlx::query(
            r#"
            delete from todos where id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        tx.commit().await?;

        Ok(())
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

    type TodoEntityHashMap = HashMap<i32, TodoEntity>;

    #[cfg(test)]
    impl TodoEntity {
        pub fn new(id: i32, text: String) -> Self {
            Self {
                id,
                text,
                completed: false,
                labels: vec![],
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct TodoRepositoryMemory {
        store: Arc<RwLock<TodoEntityHashMap>>,
    }

    impl TodoRepositoryMemory {
        pub fn new() -> Self {
            Self {
                store: Arc::default(),
            }
        }

        fn write_store_ref(&self) -> RwLockWriteGuard<TodoEntityHashMap> {
            self.store.write().unwrap()
        }

        fn read_store_ref(&self) -> RwLockReadGuard<TodoEntityHashMap> {
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
        async fn create(&self, todo: CreateTodo) -> anyhow::Result<TodoEntity> {
            let mut store = self.write_store_ref();

            let id = store.len() as i32 + 1;
            let todo = TodoEntity::new(id, todo.text);
            store.insert(id, todo.clone());
            Ok(todo)
        }

        async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
            let store = self.read_store_ref();
            let todo_found = store
                .get(&id)
                .cloned()
                .ok_or(RepositoryError::NotFound(id))?;
            Ok(todo_found)
        }

        async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
            let store = self.read_store_ref();
            let mut res = Vec::from_iter(store.values().cloned());
            res.sort_by_key(|todo| todo.id);
            Ok(res)
        }

        async fn delete(&self, id: i32) -> anyhow::Result<()> {
            let mut store = self.write_store_ref();
            store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
            Ok(())
        }

        async fn update(&self, id: i32, update_todo: UpdateTodo) -> anyhow::Result<TodoEntity> {
            let mut store = self.write_store_ref();
            let todo = store.get(&id).context(RepositoryError::NotFound(id))?;
            let text = update_todo.text.unwrap_or(todo.text.clone());
            let completed = update_todo.completed.unwrap_or(todo.completed);
            let todo = TodoEntity {
                id,
                text,
                completed,
                labels: vec![],
            };
            store.insert(id, todo.clone()).unwrap();
            Ok(todo)
        }
    }

    #[tokio::test]
    async fn test_todo_repo_scenario() {
        // create todo
        let repo = TodoRepositoryMemory::new();
        let todo = repo
            .create(CreateTodo {
                text: "test todo".to_string(),
                labels: vec![],
            })
            .await
            .expect("failed to create todo");
        assert_eq!(todo.id, 1);

        let todo2 = repo
            .create(CreateTodo {
                text: "test todo2".to_string(),
                labels: vec![],
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
                labels: Some(vec![]),
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

    #[tokio::test]
    async fn crud_scenario() {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
        let pool = PgPool::connect(&database_url)
            .await
            .unwrap_or_else(|_| panic!("failed to connect database: [{}]", database_url));
        let _ = sqlx::query("DELETE FROM todos").execute(&pool).await;
        let _ = sqlx::query("DELETE FROM labels").execute(&pool).await;
        let _ = sqlx::query("DELETE FROM todo_labels").execute(&pool).await;

        let label_name = String::from("test label");
        let optional_label = sqlx::query_as::<_, Label>(
            r#"
            SELECT * FROM labels WHERE name = $1
            "#,
        )
        .bind(label_name.clone())
        .fetch_optional(&pool)
        .await
        .expect("failed to prepare label data.");

        let label_1 = if let Some(label) = optional_label {
            // DB に label_name と同名のラベルが存在するならそれを使う
            label
        } else {
            // DB に label_name と同名のラベルが存在しないなら、新規に作成する

            sqlx::query_as::<_, Label>(
                r#"
                INSERT INTO labels ( name )
                VALUES ( $1 )
                RETURNING *
                "#,
            )
            .bind(label_name)
            .fetch_one(&pool)
            .await
            .expect("failed to insert label data.")
        };

        let repo = TodoRepositoryForDb::new(pool.clone());
        let todo_text = "[crud_scenario] text";

        // create
        let created = repo
            .create(CreateTodo::new(todo_text.to_string(), vec![label_1.id]))
            .await
            .expect("[create] returned Err");
        assert_eq!(created.text, todo_text);
        assert!(!created.completed);
        assert_eq!(*created.labels.first().unwrap(), label_1);

        // find
        let todo = repo.find(created.id).await.expect("[find] returned Err");
        assert_eq!(todo, created);

        // all
        let todos = repo.all().await.expect("[all] returned Err");
        // assert_eq!(todos, vec![todo]);
        let todo = todos.into_iter().max_by_key(|t| t.id).unwrap();
        assert_eq!(created, todo);

        // update
        let update_text = "[crud_scenario] updated text";
        let todo = repo
            .update(
                todo.id,
                UpdateTodo {
                    text: Some(update_text.to_string()),
                    completed: Some(true),
                    labels: Some(vec![]),
                },
            )
            .await
            .expect("[update] returned Err");
        assert_eq!(created.id, todo.id);
        assert_eq!(todo.text, update_text);
        assert_eq!(todo.labels.len(), 0);

        // delete
        repo.delete(todo.id).await.expect("[delete] returned Err");
        let res = repo.find(created.id).await;
        assert!(res.is_err());

        let todo_rows = sqlx::query(
            r#"
            SELECT * FROM todos where id = $1
            "#,
        )
        .bind(todo.id)
        .fetch_all(&pool)
        .await
        .expect("[delete] todo_labels fetch error");
        assert_eq!(todo_rows.len(), 0);

        let rows = sqlx::query(
            r#"
            SELECT * FROM todo_labels WHERE todo_id = $1
            "#,
        )
        .bind(todo.id)
        .fetch_all(&pool)
        .await
        .expect("[delete] todo_labels error");
        assert_eq!(rows.len(), 0);
    }
}
