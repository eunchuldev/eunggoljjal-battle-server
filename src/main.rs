use actix_web::{App, HttpServer};
use serde::Deserialize;
mod error;
mod model;
mod routes;
mod session;
#[cfg(test)]
mod test_util;
mod util;

#[derive(Deserialize, Debug)]
struct Config {
    database_url: String,
    redis_url: String,
}

#[actix_rt::main]
async fn main() -> Result<(), error::Error> {
    let config = match envy::from_env::<Config>() {
        Ok(config) => config,
        Err(err) => panic!("{:#?}", err),
    };

    let dbpool = model::DbPoolOptions::new()
        .connect(&config.database_url)
        .await?;
    let redispool = model::create_redispool(&config.redis_url)?;

    let schema = model::build_schema(dbpool.clone(), redispool.clone()).await?;

    HttpServer::new(move || {
        App::new()
            .data(schema.clone())
            .data(dbpool.clone())
            .data(redispool.clone())
            .configure(routes::routes)
    })
    .bind("0.0.0.0:8000")?
    .run()
    .await?;
    Ok(())
}
