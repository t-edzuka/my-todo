use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};

use crate::handlers::ValidatedJson;
use crate::repositories::label::{CreateLabel, LabelRepository};

pub async fn create_label<R: LabelRepository>(
    Extension(repo): Extension<R>,
    ValidatedJson(payload): ValidatedJson<CreateLabel>,
) -> Result<impl IntoResponse, StatusCode> {
    let label = repo
        .create(payload)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok((StatusCode::CREATED, Json(label)))
}

pub async fn all_label<R: LabelRepository>(
    Extension(repo): Extension<R>,
) -> Result<impl IntoResponse, StatusCode> {
    let labels = repo
        .all()
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok((StatusCode::OK, Json(labels)))
}

pub async fn delete_label<R: LabelRepository>(
    Extension(repo): Extension<R>,
    Path(id): Path<i32>,
) -> StatusCode {
    repo.delete(id)
        .await
        .map_or(StatusCode::INTERNAL_SERVER_ERROR, |_| {
            StatusCode::NO_CONTENT
        })
}
