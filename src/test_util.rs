use crate::error::Error;
use crate::model::*;
use testcontainers::{
    clients::Cli,
    images::{postgres::Postgres, redis::Redis},
    Container, Docker, Image,
};

pub struct TestDocker {
    inner: Cli,
}

impl TestDocker {
    pub fn new() -> Self {
        TestDocker {
            inner: Cli::default(),
        }
    }
    pub async fn run<'a>(&'a self) -> TestNodes<'a> {
        TestNodes::new(&self.inner).await
    }
}

pub struct TestNodes<'a> {
    pub pg: Container<'a, Cli, Postgres>,
    pub redis: Container<'a, Cli, Redis>,
    pub pgpool: DbPool,
    pub redispool: RedisPool,
    pub schema: Schema,
}
impl<'a> TestNodes<'a> {
    pub async fn new(docker: &'a Cli) -> TestNodes<'a> {
        let pg = docker.run(Postgres::default());
        let redis = docker.run(Redis::default());
        let db_host_port = pg.get_host_port(5432).unwrap_or(5432);
        let redis_host_port = redis.get_host_port(6379).unwrap_or(6379);
        let dburl = format!("postgres://postgres@localhost:{}/postgres", db_host_port);
        let redisurl = format!("redis://localhost:{}", redis_host_port);

        let dbpool = DbPoolOptions::new().connect(&dburl).await.unwrap();
        let redispool = create_redispool(&redisurl).unwrap();

        let mut migrator = sqlx::migrate!();
        migrator
            .migrations
            .to_mut()
            .retain(|migration| !migration.description.ends_with(".down"));
        migrator.run(&dbpool).await.unwrap();
        TestNodes {
            pg, redis,
            pgpool: dbpool.clone(),
            redispool: redispool.clone(),
            schema: build_schema(dbpool, redispool).await.unwrap(),
        }
    }
}
