use crate::error::Error;
use crate::model::{RedisPool, Schema};
use crate::session::extract_session;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Result as ActixWebResult};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_actix_web::{Request, Response};

#[post("/graphql")]
async fn graphql(
    schema: web::Data<Schema>,
    redis_pool: web::Data<RedisPool>,
    req: HttpRequest,
    gql_request: Request,
) -> ActixWebResult<Response> {
    let mut redis_conn = redis_pool.get().await.map_err(Error::from)?;
    let session = extract_session(&mut redis_conn, &req).await?;
    let mut request = gql_request.into_inner();
    if let Some(session) = session {
        request = request.data(session);
    }
    Ok(schema.execute(request).await.into())
}

#[get("/graphiql")]
async fn graphiql() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(playground_source(
            GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"),
        ))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(graphql).service(graphiql);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;
    use actix_web::{http::header::IntoHeaderValue, test, App};
    //use async_graphql::{value, Name, Value};

    #[actix_rt::test]
    async fn test_session_id_cookie_set() {
        let docker = TestDocker::new();
        let db = docker.run().await;

        let mut app = test::init_service(
            App::new()
                .data(db.schema.clone())
                .data(db.pgpool.clone())
                .data(db.redispool.clone())
                .configure(routes),
        )
        .await;

        let query =
            r#"{"query":"mutation { register(email:\"a\", password:\"b\", nickname:\"c\") }"}"#;
        let req = test::TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/graphql")
            .set_payload(query)
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert!(resp
            .headers()
            .get("Set-Cookie")
            .unwrap()
            .try_into_value()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("session-id="));
    }
    #[actix_rt::test]
    async fn test_authorized_request() {
        let docker = TestDocker::new();
        let db = docker.run().await;

        let mut app = test::init_service(
            App::new()
                .data(db.schema.clone())
                .data(db.pgpool.clone())
                .data(db.redispool.clone())
                .configure(routes),
        )
        .await;

        let query =
            r#"{"query":"mutation { register(email:\"a\", password:\"b\", nickname:\"c\") }"}"#;
        let req = test::TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/graphql")
            .set_payload(query)
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        let cookie_header = resp
            .headers()
            .get("Set-Cookie")
            .unwrap()
            .try_into_value()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        #[derive(serde::Deserialize)]
        struct Data {
            register: String,
        }
        #[derive(serde::Deserialize)]
        struct Res {
            data: Data,
        }
        let body: Res = serde_json::from_slice(test::read_body(resp).await.as_ref()).unwrap();
        let user_id = body.data.register;

        let query = format!(
            r#"{{"query":"query {{ user(id:\"{}\") {{ email }} }}"}}"#,
            user_id
        );
        let req = test::TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/graphql")
            .set_payload(query)
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        let body = test::read_body(resp).await;
        assert!(String::from_utf8_lossy(body.as_ref())
            .find("error")
            .is_some());
        let query = format!(
            r#"{{"query":"query {{ user(id:\"{}\") {{ email }} }}"}}"#,
            user_id
        );
        let req = test::TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("COOKIE", cookie_header.split(";").next().unwrap()))
            .uri("/graphql")
            .set_payload(query)
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        let body = test::read_body(resp).await;
        assert!(String::from_utf8_lossy(body.as_ref())
            .find("error")
            .is_none());
    }
}
