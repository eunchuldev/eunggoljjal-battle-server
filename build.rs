#![allow(dead_code)]
//use async_graphql::{EmptyMutation, EmptySubscription, Schema as GraphqlSchema, SimpleObject, Context, Error as GraphqlError, Object};
//include!("src/error.rs");
//include!("src/lib.rs");
#[path = "src/error.rs"]
mod error;
#[path = "src/model.rs"]
mod model;
#[path = "src/session.rs"]
mod session;
#[path = "src/util.rs"]
mod util;

use crate::model::{Mutation, Query, Schema};
use async_graphql::EmptySubscription;
use std::fs;

fn main() {
    // Tell Cargo that if the given file changes, to rerun this build script.
    let schema = Schema::build(Query, Mutation, EmptySubscription).finish();
    fs::write("./schema.graphql", schema.sdl()).unwrap();

    println!("cargo:rerun-if-changed=src/lib.rs");
}
