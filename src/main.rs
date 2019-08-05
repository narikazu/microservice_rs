extern crate hyper;
extern crate futures;

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde_derive;
extern crate diesel;

use diesel::prelude::*;
use diesel::pg::PgConnection;

use hyper::{Chunk, StatusCode};
use hyper::Method::{Get, Post};
use hyper::server::{Request, Response, Service};

use std::env;
const DEFAULT_DATABASE_URL: &'static str = "postgresql://postgres@localhost:5432";

use crate::futures::Stream;
use futures::future::{Future, FutureResult};
mod schema;
mod models;

use models::{Message, NewMessage};

struct Microservice;

fn write_to_db(new_message: NewMessage,
    db_connection: &PgConnection,
) -> FutureResult<i64, hyper::Error> {
    use schema::messages;
    let timestamp = diesel::insert_into(messages::table)
        .values(&new_message)
        .returning(messages::timestamp)
        .get_result(db_connection);
    match timestamp {
        Ok(timestamp) => futures::future::ok(timestamp),
        Err(error) => {
            error!("Error writing to database: {}", error.description());
            futures::future::err(hyper::Error::from(
                io::Error::new(io::ErrorKind::Other, "server error"),
            ))
        }
    }
}

fn connect_to_db() -> Option<PgConnection> {
    let database_url = env::var("DATABASE_URL").unwrap_or(String::from(DEFAULT_DATABASE_URL));
    match PgConnection::establish(&database_url) {
        Ok(connection) => Some(connection),
        Err(error) => {
            error!("Error connecting to database: {}", error.description())
        },
    }
}

fn query_db(time_range: TimeRange, db_connection: &PgConnection) -> Option<Vec<Message>> {
    use schema::messages;
    let TimeRange { before, after } = time_range;
    let query_result = match (before, after) {
        (Some(before), Some(after)) => {
            messages::table
                .filter(messages::timestamp.lt(before as i64))
                .filter(messages::timestamp.gt(after as i64))
                .load::<Message>(db_connection)
        }
        (Some(before), _) => {
            messages::table
                .filter(messages::timestamp.lt(before as i64))
                .load::<Message>(db_connection)
        }
        (_, Some(after)) => {
            messages::table
                .filter(messages::timestamp.gt(after as i64))
                .load::<Message>(db_connection)
        }
        _ => messages::table.load::<Message>(db_connection),
    };
    match query_result {
        Ok(result) => Some(result),
        Err(error) => {
            error!("Error querying DB: {}", error);
            None
        },
    }
}

impl Service for Microservice {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;
    fn call(&self, request: Request) -> Self::Future {
        let db_connection = match connect_to_db() {
            Some(connection) => connection,
            None => {
                return Box::new(futures::future::ok(
                    Response::new().with_status(StatusCode::InternalServerError),
                ))
            }
        };
        match (request.method(), request.path()) {
            (&Post, "/") => {
                let future = request
                    .body()
                    .concat2()
                    .and_then(parse_form)
                    .and_then(move |new_message| write_to_db(new_message, &db_connection))
                    .then(make_post_response);
                Box::new(future)
            },
            (&Get, "/") => {
                let time_range = match request.query() {
                    Some(query) => parse_query(query),
                    None => Ok(TimeRange {
                        before: None,
                        after: None,
                    }),
                };
                let response = match time_range {
                    Ok(time_range) => make_get_response(query_db(time_range, &db_connection)),
                    Err(error) => make_error_response(&error),
                };
                Box::new(response)
            }
            _ => Box::new(futures::future::ok(
                Response::new().with_status(StatusCode::NotFound),
            )),
        }
    }
}

fn main() {
    env_logger::init();
    let address = "127.0.0.1:8080".parse().unwrap();
    let server = hyper::server::Http::new()
        .bind(&address, || Ok(Microservice {}))
        .unwrap();
    info!("Running microservice at {}", address);
    server.run().unwrap();
}
