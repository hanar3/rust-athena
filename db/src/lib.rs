#[macro_use]
extern crate diesel;
extern crate dotenv;

pub mod models;
pub mod schema;

use diesel::mysql::MysqlConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Error};
use dotenv::dotenv;
use lazy_static::lazy_static;
use r2d2;
use std::env;

type Pool = r2d2::Pool<ConnectionManager<MysqlConnection>>;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<MysqlConnection>>;

lazy_static! {
  static ref POOL: Pool = {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    Pool::new(manager).expect("Failed to create db pool")
  };
}

pub fn connection() -> Result<DbConnection, diesel::ConnectionError> {
  POOL
    .get()
    .map_err(|e| diesel::ConnectionError::BadConnection("Failed to get db connection".to_string()))
}

pub fn init() {
  lazy_static::initialize(&POOL);
  let conn = connection().expect("Failed to get db connection");
}
