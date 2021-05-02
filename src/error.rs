use actix_web::ResponseError;
use async_graphql::{Error as GraphqlError, ErrorExtensions};
use thiserror::Error as thisError;

#[derive(thisError, Debug)]
pub enum Error {
    #[error("redis pool not found in context")]
    RedisPoolNotFoundInContext,
    #[error("database query error(sqlx): {0:?}")]
    Database(#[from] sqlx::Error),
    #[error("sqlx migration error: {0:?}")]
    DatabaseMigrate(#[from] sqlx::migrate::MigrateError),
    #[error("invalid request form. method={0:?} detail={1:?}")]
    BadRequest(&'static str, &'static str),
    #[allow(dead_code)]
    #[error("not implemtned yet. method={0:?} detail={1:?}")]
    NotImplemented(&'static str, &'static str),
    #[error("bcrypt error: {0:?}")]
    BcryptError(#[from] bcrypt::BcryptError),
    #[error("jwt error: {0:?}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("not authorized to do such request")]
    NotAuthorized,
    #[error("wrong password")]
    WrongPassword,
    #[error("bincode error: {0:?}")]
    BincodeError(#[from] bincode::Error),
    #[error("base64 error: {0:?}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("io error: {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("redis pool error: {0:?}")]
    RedisPoolError(#[from] deadpool_redis::PoolError),
    #[error("redis error: {0:?}")]
    RedisError(#[from] redis::RedisError),
}

impl ResponseError for Error {}

/*impl From<Error> for GraphqlError {
    fn from(e: Error) -> Self {
        GraphqlError::new(format!("{}", e))
    }
}*/

impl ErrorExtensions for Error {
    fn extend(&self) -> GraphqlError {
        GraphqlError::new(format!("{}", self))
    }
}
