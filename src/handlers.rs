use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};

use crate::repositories::{CreateTodo, TodoRepository, UpdateTodo};

pub async fn create_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    Json(todo): Json<CreateTodo>,
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

pub async fn update_todo<R: TodoRepository>(
    Extension(repo): Extension<Arc<R>>,
    Path(id): Path<u64>,
    Json(update_todo): Json<UpdateTodo>,
) -> Result<impl IntoResponse, StatusCode> {
    let todo = repo
        .update(id, update_todo)
        .or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::CREATED, Json(todo)))
}
