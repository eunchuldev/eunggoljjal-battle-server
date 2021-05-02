use crate::error::Error;
use crate::model::{Schema, RedisPool};
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
    use actix_web::{test, App};
    //use async_graphql::{value, Name, Value};

    #[actix_rt::test]
    async fn test_session_id_set() {
        let docker = TestDocker::new();
        let db = docker.run().await;
        
        let mut app = test::init_service(
            App::new()
                .data(web::Data::new(db.schema.clone()))
                .data(web::Data::new(db.pgpool.clone()))
                .data(web::Data::new(db.redispool.clone()))
                .configure(routes)
            ).await;

        let query = r#"mutation { register(email:"a", password:"b", nickname:"c") }"#;
        let req = test::TestRequest::post()
            .uri("/graphql")
            .set_payload(query).to_request();
        let resp = test::call_service(&mut app, req).await;
        let body = test::read_body(resp).await;
        assert_eq!(body, web::Bytes::from_static(b"data: 5\n\n"));
        //assert!(resp.status().is_success());
    }
}
