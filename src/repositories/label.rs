use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx;
use validator::Validate;

use crate::repositories::RepositoryError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::FromRow)]
pub struct Label {
    pub id: i32,
    pub name: String,
}

#[async_trait]
pub trait LabelRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, label: CreateLabel) -> anyhow::Result<Label>;
    async fn all(&self) -> anyhow::Result<Vec<Label>>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Validate)]
pub struct CreateLabel {
    #[validate(length(min = 1, message = "Label name is required"))]
    #[validate(length(max = 255, message = "Label name is too long"))]
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct LabelRepositoryForDb {
    pool: sqlx::PgPool,
}

#[allow(dead_code)]
impl LabelRepositoryForDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        LabelRepositoryForDb { pool }
    }
}

#[async_trait]
impl LabelRepository for LabelRepositoryForDb {
    async fn create(&self, label: CreateLabel) -> anyhow::Result<Label> {
        // Name duplication check
        let select_query = r#"select * from labels where name = $1"#;
        let maybe_exists_row = sqlx::query_as::<_, Label>(select_query)
            .bind(label.name.clone())
            .fetch_optional(&self.pool)
            .await?;
        if let Some(label) = maybe_exists_row {
            return Err(RepositoryError::DuplicatedLabel(label.id).into());
        }

        let insert_query = r#"
        insert into labels (name) values ($1) returning *
        "#;
        let label = sqlx::query_as::<_, Label>(insert_query)
            .bind(label.name.clone())
            .fetch_one(&self.pool)
            .await?;
        Ok(label)
    }

    async fn all(&self) -> anyhow::Result<Vec<Label>> {
        let select_query = r#"select * from labels"#;
        let labels = sqlx::query_as::<_, Label>(select_query)
            .fetch_all(&self.pool)
            .await?;
        Ok(labels)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let delete_query = r#"delete from labels where id = $1"#;
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
}

#[cfg(test)]
pub mod test_inmemory_repo {
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    };

    use axum::async_trait;

    use crate::repositories::label::CreateLabel;
    use crate::repositories::RepositoryError;

    use super::*;

    impl Label {
        pub fn new(id: i32, name: String) -> Self {
            Self { id, name }
        }
    }

    type LabelHashMap = HashMap<i32, Label>;

    #[derive(Debug, Clone)]
    pub struct LabelRepositoryForMemory {
        store: Arc<RwLock<LabelHashMap>>,
    }

    impl LabelRepositoryForMemory {
        pub fn new() -> Self {
            LabelRepositoryForMemory {
                store: Arc::default(),
            }
        }

        fn write_store_ref(&self) -> RwLockWriteGuard<LabelHashMap> {
            self.store.write().unwrap()
        }

        fn read_store_ref(&self) -> RwLockReadGuard<LabelHashMap> {
            self.store.read().unwrap()
        }
    }

    #[async_trait]
    impl LabelRepository for LabelRepositoryForMemory {
        async fn create(&self, payload: CreateLabel) -> anyhow::Result<Label> {
            let mut store = self.write_store_ref();
            let id = (store.len() + 1) as i32;
            let label = Label::new(id, payload.name);
            store.insert(id, label.clone());
            Ok(label)
        }

        async fn all(&self) -> anyhow::Result<Vec<Label>> {
            let store = self.read_store_ref();
            let labels = Vec::from_iter(store.values().cloned());
            Ok(labels)
        }

        async fn delete(&self, id: i32) -> anyhow::Result<()> {
            let mut store = self.write_store_ref();
            store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[tokio::test]
        async fn label_crud_scenario() {
            let name = "label name".to_string();
            let id = 1;
            let expected = Label::new(id, name.clone());

            let repo = LabelRepositoryForMemory::new();

            // create
            let label = repo
                .create(CreateLabel { name })
                .await
                .expect("failed create label");
            assert_eq!(expected, label);

            // all
            let labels = repo.all().await.expect("failed get all labels");
            assert_eq!(vec![label], labels);

            // delete
            repo.delete(id).await.expect("failed delete label");
            let labels = repo.all().await.expect("failed get all labels");
            assert_eq!(labels.len(), 0);
        }
    }
}
