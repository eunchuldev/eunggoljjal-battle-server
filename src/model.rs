use crate::error;
use crate::session::{create_session, Session};
use crate::util::{hash_password, verify_password};
//use crate::util::{hash_password, verify_password, create_jwt_token, create_jwt_token};

use async_graphql::{
    connection::{Connection, CursorType, Edge, EmptyFields},
    Context, EmptySubscription, Enum, Error as GraphqlError, Object, Schema as GraphqlSchema,
    SimpleObject,
};
pub use deadpool_redis::{Config as RedisConfig, Pool as RedisPool};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use error::Error;

type DateTime = chrono::DateTime<chrono::Utc>;
pub type DbPool = sqlx::postgres::PgPool;
pub type DbPoolOptions = sqlx::postgres::PgPoolOptions;

pub fn create_redispool(url: &str) -> Result<RedisPool, Error> {
    Ok(RedisConfig {
        url: Some(url.to_string()),
        pool: None,
    }
    .create_pool()?)
}

#[derive(sqlx::FromRow, Clone, Debug, Deserialize, Serialize, PartialEq, SimpleObject)]
pub struct Card {
    pub id: Uuid,
    pub rating: f64,
    pub owned_at: DateTime,
    pub created_at: DateTime,
    pub owner_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum CardCursor {
    OwnedAt(DateTime),
    Rating(f64),
}
impl CursorType for CardCursor {
    type Error = error::Error;
    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize(&base64::decode(s)?)?)
    }
    fn encode_cursor(&self) -> String {
        base64::encode(bincode::serialize(&self).unwrap())
    }
}

#[derive(sqlx::Type, Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Enum)]
#[sqlx(rename = "userkind")]
pub enum UserKind {
    #[sqlx(rename = "super")]
    Super,
    #[sqlx(rename = "normal")]
    Normal,
}

#[derive(sqlx::FromRow, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct User {
    #[serde(skip)]
    pub id: Uuid,
    #[serde(skip)]
    pub password: String,

    pub kind: UserKind,
    pub email: String,
    pub nickname: String,
    pub created_at: DateTime,
}

#[Object]
impl User {
    async fn id(&self) -> Uuid {
        self.id
    }
    async fn kind(&self) -> UserKind {
        self.kind
    }
    async fn nickname(&self) -> &str {
        &self.nickname
    }
    async fn created_at(&self) -> &DateTime {
        &self.created_at
    }
    /// Email addr. Not fetchable by other users.
    async fn email(&self, ctx: &Context<'_>) -> Result<&str, GraphqlError> {
        let session: &Session = ctx
            .data_opt::<Session>()
            .ok_or_else(|| GraphqlError::from(Error::NotAuthorized))?;
        if session.user_id == self.id || session.user_kind == UserKind::Super {
            Ok(&self.email)
        } else {
            Err(GraphqlError::from(Error::NotAuthorized))
        }
    }
    /// Cards owned by the user.
    async fn cards(
        &self,
        ctx: &Context<'_>,
        after: Option<String>,
        before: Option<String>,
        #[graphql(desc = "first N items. clamped by [0-100]")] first: Option<i32>,
        #[graphql(desc = "last N items. clamped by [0-100]")] last: Option<i32>,
    ) -> Result<Connection<CardCursor, Card, EmptyFields, EmptyFields>, GraphqlError> {
        let dbpool = ctx.data::<DbPool>()?;
        if first.is_some() && last.is_some() {
            return Err(Error::BadRequest("cards", "first or last, not both").into());
        }
        let first = if first.is_none() && last.is_none() {
            Some(100)
        } else {
            first
        };
        let first = first.map(|l| l.min(100).max(0));
        let last = last.map(|l| l.min(100).max(0));
        async_graphql::connection::query(after, before, first, last, |after, before, first, last| async move {
            let (after, before) = match (after, before) {
                (Some(CardCursor::OwnedAt(after)), None) => (CardCursor::OwnedAt(after), CardCursor::OwnedAt(chrono::MAX_DATETIME)),
                (None, Some(CardCursor::OwnedAt(before))) => (CardCursor::OwnedAt(chrono::MIN_DATETIME), CardCursor::OwnedAt(before)),
                (Some(CardCursor::Rating(after)), None) => (CardCursor::Rating(after), CardCursor::Rating(f64::MAX)),
                (None, Some(CardCursor::Rating(before))) => (CardCursor::Rating(f64::MIN), CardCursor::Rating(before)),
                _ => (CardCursor::OwnedAt(chrono::MIN_DATETIME), CardCursor::OwnedAt(chrono::MAX_DATETIME))
            };
            let cursor_kind = after.clone();
            let cards = match (after, before, first, last) {
                (CardCursor::OwnedAt(after), CardCursor::OwnedAt(before), Some(limit), None) => {
                    Ok(sqlx::query_as::<_, Card>("SELECT * FROM cards WHERE owner_id = $1 AND owned_at > $2 AND owned_at < $3 ORDER BY owned_at ASC LIMIT $4")
                        .bind(self.id)
                        .bind(after)
                        .bind(before)
                        .bind(limit as i32)
                        .fetch_all(dbpool)
                        .await?)
                }
                (CardCursor::OwnedAt(after), CardCursor::OwnedAt(before), None, Some(limit)) => {
                    Ok(sqlx::query_as::<_, Card>("SELECT * FROM cards WHERE owner_id = $1 AND owned_at > $2 AND owned_at < $3 ORDER BY owned_at DESC LIMIT $4")
                        .bind(self.id)
                        .bind(after)
                        .bind(before)
                        .bind(limit as i32)
                        .fetch_all(dbpool)
                        .await?)
                }
                (CardCursor::Rating(after), CardCursor::Rating(before), Some(limit), None) => {
                    Ok(sqlx::query_as::<_, Card>("SELECT * FROM cards WHERE owner_id = $1 AND rating > $2 AND rating < $3 ORDER BY rating ASC LIMIT $4")
                        .bind(self.id)
                        .bind(after)
                        .bind(before)
                        .bind(limit as i32)
                        .fetch_all(dbpool)
                        .await?)
                }
                (CardCursor::Rating(after), CardCursor::Rating(before), None, Some(limit)) => {
                    Ok(sqlx::query_as::<_, Card>("SELECT * FROM cards WHERE owner_id = $1 AND rating > $2 AND rating < $3 ORDER BY rating DESC LIMIT $4")
                        .bind(self.id)
                        .bind(after)
                        .bind(before)
                        .bind(limit as i32)
                        .fetch_all(dbpool)
                        .await?)
                }
                _ => {
                    Err(Error::BadRequest("cards", "cursor type not match"))
                }
            }?;
            let mut connection = Connection::new(
                last.filter(|limit| limit > &cards.len()).is_some(),
                first.filter(|limit| limit > &cards.len()).is_some());
            match cursor_kind {
                CardCursor::OwnedAt(_) => {
                    connection.append(
                        cards.into_iter().map(|card| Edge::new(CardCursor::OwnedAt(card.owned_at), card))
                    );
                }
                CardCursor::Rating(_) => {
                    connection.append(
                        cards.into_iter().map(|card| Edge::new(CardCursor::Rating(card.rating), card))
                    );
                }
            };
            Ok(connection)
        }).await
    }
}

pub struct Mutation;

#[Object]
impl Mutation {
    async fn register(
        &self,
        ctx: &Context<'_>,
        email: String,
        password: String,
        nickname: String,
    ) -> Result<Uuid, GraphqlError> {
        let dbpool = ctx.data::<DbPool>()?;
        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (email, password, nickname) VALUES ($1, $2, $3) RETURNING id, password, kind, email, nickname, created_at")
            .bind(email)
            .bind(hash_password(password)?)
            .bind(nickname)
            .fetch_one(dbpool)
            .await?;
        create_session(&ctx, &user).await?;
        Ok(user.id)
    }
    async fn login(
        &self,
        ctx: &Context<'_>,
        email: String,
        password: String,
    ) -> Result<Uuid, GraphqlError> {
        let dbpool = ctx.data::<DbPool>()?;
        //let redis = ctx.data::<RedisPool>()?.get().await?;
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(dbpool)
            .await?;
        if !verify_password(&password, &user.password)? {
            Err(GraphqlError::from(Error::WrongPassword))
        } else {
            create_session(&ctx, &user).await?;
            Ok(user.id)
        }
    }
    /*async fn start_battle(
        &self,
        ctx: &Context<'_>,
        card_id: Uuid,
    ) -> Result<Uuid, GraphqlError> {
        let card = sqlx::query_as<_, bool>("SELECT * FROM cards WHERE id = $1 AND owner_id = $2")
    }*/
}

pub struct Query;

#[Object]
impl Query {
    async fn api_version(&self) -> String {
        "0.1".to_string()
    }
    async fn user(&self, ctx: &Context<'_>, id: Uuid) -> Result<User, GraphqlError> {
        let dbpool = ctx.data::<DbPool>()?;
        Ok(
            sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
                .bind(id)
                .fetch_one(dbpool)
                .await?,
        )
    }
}

pub type Schema = GraphqlSchema<Query, Mutation, EmptySubscription>;

pub async fn build_schema(dbpool: DbPool, redispool: RedisPool) -> Result<Schema, Error> {
    Ok(GraphqlSchema::build(Query, Mutation, EmptySubscription)
        .data(dbpool)
        .data(redispool)
        .finish())
}

#[cfg(test)]
pub mod tests {
    use crate::test_util::*;
    use async_graphql::{value, Name, Value};

    #[actix_rt::test]
    async fn test_migration_and_build_schema() {
        let docker = TestDocker::new();
        let db = docker.run().await;
        let schema = db.schema.clone();
    }
    #[actix_rt::test]
    async fn test_register() {
        let docker = TestDocker::new();
        let db = docker.run().await;
        let schema = db.schema.clone();

        let query = r#"mutation { register(email:"a", password:"b", nickname:"c") }"#;
        let res = schema.execute(query).await;
        let data = res.data;
        assert_eq!(res.errors, Vec::new(),);

        let user_id = match data {
            Value::Object(v) => match v.get(&Name::new("register")) {
                Some(Value::String(id)) => id.clone(),
                _ => panic!("unexpected value type"),
            },
            _ => panic!("unexpected value type"),
        };

        let query = format!(r#"query {{ user(id:"{}") {{ nickname }} }}"#, user_id);
        let res = schema.execute(&query).await;
        let data = res.data;
        assert_eq!(res.errors, Vec::new(),);
        assert_eq!(
            data,
            value!( {
                "user": {
                    "nickname": "c"
                }
            } )
        );
    }

    #[actix_rt::test]
    async fn test_login() {
        let docker = TestDocker::new();
        let db = docker.run().await;
        let schema = db.schema.clone();

        let query = r#"mutation { register(email:"a", password:"b", nickname:"c") }"#;
        let res = schema.execute(query).await;
        let data = res.data;

        let user_id = match data {
            Value::Object(v) => match v.get(&Name::new("register")) {
                Some(Value::String(id)) => id.clone(),
                _ => panic!("unexpected value type"),
            },
            _ => panic!("unexpected value type"),
        };

        let query = r#"mutation { login(email:"a", password:"b") }"#;
        let res = schema.execute(query).await;
        let data = res.data;
        assert_eq!(res.errors, Vec::new(),);
        let user_id2 = match data {
            Value::Object(v) => match v.get(&Name::new("login")) {
                Some(Value::String(id)) => id.clone(),
                _ => panic!("unexpected value type"),
            },
            _ => panic!("unexpected value type"),
        };

        assert_eq!(user_id, user_id2);

        let query = r#"mutation { login(email:"a", password:"c") }"#;
        let res = schema.execute(query).await;
        assert_eq!(
            res.errors
                .into_iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>(),
            vec!["wrong password"]
        );
    }
}
