use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};

use crate::handlers::ValidatedJson;
use crate::repositories::todo::{CreateTodo, TodoRepository, UpdateTodo};

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
