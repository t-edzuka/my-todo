use std::sync::Arc;

use axum::extract::{FromRequest, Path};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::{async_trait, BoxError, Extension, Json};
use http_body::Body as HttpBody;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::repositories::todo::{CreateTodo, TodoRepository, UpdateTodo};

#[derive(Debug)]
pub struct ValidatedJson<T>(T);

pub async fn create_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    ValidatedJson(create_todo): ValidatedJson<CreateTodo>,
) -> anyhow::Result<impl IntoResponse, StatusCode> {
    let todo = repo
        .create(create_todo)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::CREATED, Json(todo)))
}

pub async fn find_todo<R: TodoRepository>(
    Path(id): Path<i32>,
    Extension(repo): Extension<Arc<R>>,
) -> anyhow::Result<impl IntoResponse, StatusCode> {
    let todo = repo.find(id).await.or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::OK, Json(todo)))
}

pub async fn all_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
) -> anyhow::Result<impl IntoResponse, StatusCode> {
    let todos = repo.all().await.expect("Can not get all todos");
    Ok((StatusCode::OK, Json(todos)))
}

pub async fn update_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    Path(id): Path<i32>,
    ValidatedJson(update_todo): ValidatedJson<UpdateTodo>,
) -> Result<impl IntoResponse, StatusCode> {
    let todo = repo
        .update(id, update_todo)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::CREATED, Json(todo)))
}

pub async fn delete_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    Path(id): Path<i32>,
) -> StatusCode {
    repo.delete(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[async_trait] // Rustのtraitでasync関数を実装できないためマクロを使用する。
impl<T, S, B> FromRequest<S, B> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|rejection| {
                let message = format!("Json parse error: [{}]", rejection);
                (StatusCode::BAD_REQUEST, message)
            })?;
        value.validate().map_err(|rejection| {
            let message = format!("Validation error: [{}]", rejection).replace('\n', ", ");
            (StatusCode::BAD_REQUEST, message)
        })?;
        Ok(ValidatedJson(value))
    }
}
