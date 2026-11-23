use crate::schema::characters;

#[derive(Queryable)]
pub struct Account {
    pub id: i64,
    pub password_hash: String,
    pub sex: u8,
}

#[derive(Queryable)]
pub struct Character {
    pub id: u32,
    pub account_id: i64,
    pub slot: u8,
    pub name: String,
    pub job: i16,
    pub hair: i16,
    pub hair_color: i16,
    pub sex: u8,
    pub map_name: String,
}

#[derive(Insertable)]
#[table_name = "characters"]
pub struct NewCharacter<'a> {
    pub account_id: i64,
    pub slot: u8,
    pub name: &'a str,
    pub job: i16,
    pub hair: i16,
    pub hair_color: i16,
    pub sex: u8,
    pub map_name: &'a str,
}
