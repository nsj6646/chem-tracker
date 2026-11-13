use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("데이터베이스 오류: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("입력값 검증 오류: {0}")]
    Validation(String),

    #[error("입출력 오류: {0}")]
    Io(#[from] std::io::Error),
}

pub type AppResult<T> = Result<T, AppError>;
