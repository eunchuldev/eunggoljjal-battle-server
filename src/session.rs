use crate::error::Error;
use crate::model::{User, UserKind};
use actix_web::{HttpMessage, HttpRequest};
use deadpool_redis::{cmd, ConnectionWrapper as RedisConn, Pool as RedisPool};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SESSION_LIFETIME_SECONDS: i64 = 60 * 60 * 24;
const SESSION_LENGTH: usize = 30;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Session {
    pub user_id: Uuid,
    pub user_kind: UserKind,
}

pub async fn create_session(ctx: &async_graphql::Context<'_>, user: &User) -> Result<(), Error> {
    let mut redis_conn = ctx
        .data_opt::<RedisPool>()
        .ok_or(Error::RedisPoolNotFoundInContext)?
        .get()
        .await?;
    let session = Session {
        user_id: user.id,
        user_kind: user.kind,
    };
    let session_id: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(SESSION_LENGTH)
        .map(char::from)
        .collect();
    let expire_at = chrono::Utc::now() + chrono::Duration::seconds(SESSION_LIFETIME_SECONDS);
    let session_cookie_header = format!(
        "session-id={}; Secure; HttpOnly; Expires={}",
        session_id,
        expire_at.format("%a, %d %b %Y %H:%M:%S GMT")
    );
    ctx.append_http_header("Set-Cookie", session_cookie_header);
    let key = format!("session/{}", session_id);
    cmd("SET")
        .arg(&[key.as_bytes(), bincode::serialize(&session)?.as_ref()])
        .execute_async(&mut redis_conn)
        .await?;
    cmd("EXPIRE")
        .arg(&[
            key.as_bytes(),
            SESSION_LIFETIME_SECONDS.to_string().as_bytes(),
        ])
        .execute_async(&mut redis_conn)
        .await?;
    Ok(())
}

/*pub async fn remove_session(
    ctx: &async_graphql::Context<'_>,
) -> Result<Option<Session>, Error> {
}*/

pub async fn extract_session(
    redis_conn: &mut RedisConn,
    req: &HttpRequest,
) -> Result<Option<Session>, Error> {
    if let Some(session_id) = req.cookie("session-id") {
        let bytes: Vec<u8> = cmd("GET")
            .arg(&[&format!("session/{}", session_id.value())])
            .query_async(redis_conn)
            .await?;
        let sess: Session = bincode::deserialize(&bytes)?;
        Ok(Some(sess))
    } else {
        Ok(None)
    }
}
