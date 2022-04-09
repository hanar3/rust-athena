#[derive(Queryable)]
pub struct User {
  pub id: i32,
  pub username: String,
  pub password_hash: String,
  pub email: String,
  pub created_at: i32,
  pub updated_at: i32,
}
