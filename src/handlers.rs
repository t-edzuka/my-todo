use std::sync::Arc;

use axum::body::HttpBody;
use axum::extract::{FromRequest, Path};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::{async_trait, BoxError, Extension, Json};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::repositories::{CreateTodo, TodoRepository, UpdateTodo};

#[derive(Debug)]
pub struct ValidatedJson<T>(T);

pub async fn create_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    ValidatedJson(todo): ValidatedJson<CreateTodo>,
) -> impl IntoResponse {
    let todo = repo.create(todo);
    (StatusCode::CREATED, Json(todo))
}

pub async fn find_todo<R: TodoRepository>(
    Path(id): Path<u64>,
    Extension(repo): Extension<Arc<R>>,
) -> Result<impl IntoResponse, StatusCode> {
    let todo = repo.find(id).ok_or(StatusCode::NOT_FOUND)?;
    Ok((StatusCode::OK, Json(todo)))
}

pub async fn all_todo<R: TodoRepository>(Extension(repo): Extension<Arc<R>>) -> impl IntoResponse {
    let todos = repo.all();
    (StatusCode::OK, Json(todos))
}

pub async fn delete_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    Path(id): Path<u64>,
) -> StatusCode {
    repo.delete(id)
        .map(|_| StatusCode::NO_CONTENT)
        .unwrap_or(StatusCode::NOT_FOUND)
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

pub async fn update_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    Path(id): Path<u64>,
    ValidatedJson(update_todo): ValidatedJson<UpdateTodo>,
) -> Result<impl IntoResponse, StatusCode> {
    let todo = repo
        .update(id, update_todo)
        .or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::CREATED, Json(todo)))
}
