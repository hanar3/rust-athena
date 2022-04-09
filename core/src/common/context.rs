use diesel::mysql::MysqlConnection;
use rust_athena_db::establish_connection;

pub struct ServerContext {
  pub db_connection: MysqlConnection,
}

pub fn create_server_context() -> ServerContext {
  let db_connection = establish_connection();
  ServerContext { db_connection }
}
