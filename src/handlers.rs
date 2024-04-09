use axum::extract::{FromRequest, Request,};
use axum::http::StatusCode;
use axum::{async_trait, Json};
use serde::de::DeserializeOwned;
use validator::Validate;

pub mod label;
pub mod todo;

#[derive(Debug)]
pub struct ValidatedJson<T>(T);

#[async_trait] // Rustのtraitでasync関数を実装できないためマクロを使用する。
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    // B: HttpBody + Send + 'static,
    // B::Data: Send,
    // B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
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
