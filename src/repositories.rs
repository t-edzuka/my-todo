use thiserror::Error;

pub mod label;
pub mod todo;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Unexpected error: {0}")]
    Unexpected(String),
    #[error("Not found id: {0}")]
    NotFound(i32),
    #[error("Duplicated error: {0}")]
    DuplicatedLabel(i32),
}
