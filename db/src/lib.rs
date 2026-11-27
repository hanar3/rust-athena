#[macro_use]
extern crate diesel;

mod models;
mod schema;

pub use models::{Account, Character, NewCharacter};

use diesel::mysql::MysqlConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::{BigInt, Integer, Nullable, TinyInt, Unsigned, VarChar};
use schema::{characters, users};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone)]
pub struct Database(Pool<ConnectionManager<MysqlConnection>>);

#[derive(Clone, Copy)]
pub struct CharacterCredentials {
    pub login_ids: [u32; 2],
    pub sex: u8,
}

#[derive(QueryableByName)]
pub struct MapTransfer {
    #[sql_type = "Nullable<Unsigned<Integer>>"]
    login_id2: Option<u32>,
    #[sql_type = "Nullable<Unsigned<TinyInt>>"]
    account_sex: Option<u8>,
}

impl MapTransfer {
    pub fn return_credentials(&self, auth_code: u32) -> Option<CharacterCredentials> {
        Some(CharacterCredentials {
            login_ids: [auth_code, self.login_id2?],
            sex: self.account_sex?,
        })
    }
}

impl Database {
    pub fn connect(url: &str) -> Result<Self> {
        Ok(Self(
            Pool::builder()
                .max_size(8)
                .build(ConnectionManager::new(url))?,
        ))
    }

    pub fn register(&self, username: &str, hash: &str, email: &str) -> Result<()> {
        diesel::insert_into(users::table)
            .values((
                users::username.eq(username),
                users::password_hash.eq(hash),
                users::email.eq(email),
            ))
            .execute(&self.0.get()?)?;
        Ok(())
    }

    pub fn account(&self, username: &str) -> Result<Option<Account>> {
        Ok(users::table
            .filter(users::username.eq(username))
            .select((users::id, users::password_hash, users::sex))
            .first(&self.0.get()?)
            .optional()?)
    }

    pub fn issue_handoff(&self, account: u32, ids: [u32; 2], sex: u8) -> Result<()> {
        let connection = self.0.get()?;
        diesel::sql_query("DELETE FROM login_handoffs WHERE expires_at <= UTC_TIMESTAMP(6)")
            .execute(&connection)?;
        diesel::sql_query(
            "INSERT INTO login_handoffs (account_id, login_id1, login_id2, sex, expires_at)
             VALUES (?, ?, ?, ?, UTC_TIMESTAMP(6) + INTERVAL 60 SECOND)
             ON DUPLICATE KEY UPDATE login_id1 = ?, login_id2 = ?, sex = ?,
                 expires_at = UTC_TIMESTAMP(6) + INTERVAL 60 SECOND",
        )
        .bind::<BigInt, _>(i64::from(account))
        .bind::<Unsigned<Integer>, _>(ids[0])
        .bind::<Unsigned<Integer>, _>(ids[1])
        .bind::<Unsigned<TinyInt>, _>(sex)
        .bind::<Unsigned<Integer>, _>(ids[0])
        .bind::<Unsigned<Integer>, _>(ids[1])
        .bind::<Unsigned<TinyInt>, _>(sex)
        .execute(&connection)?;
        Ok(())
    }

    pub fn issue_return_handoff(
        &self,
        account: u32,
        credentials: CharacterCredentials,
    ) -> Result<()> {
        let connection = self.0.get()?;
        diesel::sql_query(
            "DELETE FROM login_handoffs WHERE account_id = ? AND expires_at <= UTC_TIMESTAMP(6)",
        )
        .bind::<BigInt, _>(i64::from(account))
        .execute(&connection)?;
        diesel::sql_query(
            "INSERT INTO login_handoffs (account_id, login_id1, login_id2, sex, expires_at)
             VALUES (?, ?, ?, ?, UTC_TIMESTAMP(6) + INTERVAL 60 SECOND)",
        )
        .bind::<BigInt, _>(i64::from(account))
        .bind::<Unsigned<Integer>, _>(credentials.login_ids[0])
        .bind::<Unsigned<Integer>, _>(credentials.login_ids[1])
        .bind::<Unsigned<TinyInt>, _>(credentials.sex)
        .execute(&connection)?;
        Ok(())
    }

    pub fn redeem_handoff(&self, account: u32, ids: [u32; 2], sex: u8) -> Result<bool> {
        Ok(diesel::sql_query(
            "DELETE FROM login_handoffs WHERE account_id = ? AND login_id1 = ?
             AND login_id2 = ? AND sex = ? AND expires_at > UTC_TIMESTAMP(6)",
        )
        .bind::<BigInt, _>(i64::from(account))
        .bind::<Unsigned<Integer>, _>(ids[0])
        .bind::<Unsigned<Integer>, _>(ids[1])
        .bind::<Unsigned<TinyInt>, _>(sex)
        .execute(&self.0.get()?)?
            == 1)
    }

    pub fn issue_map_transfer(
        &self,
        account: u32,
        character: u32,
        credentials: CharacterCredentials,
        map: &str,
    ) -> Result<bool> {
        let connection = self.0.get()?;
        diesel::sql_query("DELETE FROM map_transfers WHERE expires_at <= UTC_TIMESTAMP(6)")
            .execute(&connection)?;
        Ok(diesel::sql_query(
            "INSERT INTO map_transfers
                 (account_id, character_id, auth_code, sex, map_name, expires_at, login_id2, account_sex)
             SELECT account_id, id, ?, sex, map_name, UTC_TIMESTAMP(6) + INTERVAL 60 SECOND, ?, ?
             FROM characters WHERE account_id = ? AND id = ? AND map_name = ?
             ON DUPLICATE KEY UPDATE character_id = VALUES(character_id),
                 auth_code = VALUES(auth_code), sex = VALUES(sex), map_name = VALUES(map_name),
                 expires_at = VALUES(expires_at), login_id2 = VALUES(login_id2),
                 account_sex = VALUES(account_sex)",
        )
        .bind::<Unsigned<Integer>, _>(credentials.login_ids[0])
        .bind::<Unsigned<Integer>, _>(credentials.login_ids[1])
        .bind::<Unsigned<TinyInt>, _>(credentials.sex)
        .bind::<BigInt, _>(i64::from(account))
        .bind::<Unsigned<Integer>, _>(character)
        .bind::<VarChar, _>(map)
        .execute(&connection)? != 0)
    }

    pub fn redeem_map_transfer(
        &self,
        account: u32,
        character: u32,
        auth_code: u32,
        sex: u8,
        map: &str,
    ) -> Result<Option<MapTransfer>> {
        let connection = self.0.get()?;
        let transfer = connection.transaction::<_, diesel::result::Error, _>(|| {
            let transfer = diesel::sql_query(
                "SELECT login_id2, account_sex FROM map_transfers
                 WHERE account_id = ? AND character_id = ? AND auth_code = ?
                   AND sex = ? AND map_name = ? AND expires_at > UTC_TIMESTAMP(6)
                 FOR UPDATE",
            )
            .bind::<BigInt, _>(i64::from(account))
            .bind::<Unsigned<Integer>, _>(character)
            .bind::<Unsigned<Integer>, _>(auth_code)
            .bind::<Unsigned<TinyInt>, _>(sex)
            .bind::<VarChar, _>(map)
            .get_result::<MapTransfer>(&connection)
            .optional()?;
            if transfer.is_some() {
                diesel::sql_query("DELETE FROM map_transfers WHERE account_id = ?")
                    .bind::<BigInt, _>(i64::from(account))
                    .execute(&connection)?;
            }
            Ok(transfer)
        })?;
        Ok(transfer)
    }

    pub fn character(&self, account: u32, id: u32) -> Result<Option<Character>> {
        Ok(characters::table
            .filter(characters::account_id.eq(i64::from(account)))
            .find(id)
            .first(&self.0.get()?)
            .optional()?)
    }

    pub fn character_position(
        &self,
        account: u32,
        id: u32,
        map: &str,
    ) -> Result<Option<(u16, u16)>> {
        let row = characters::table
            .find(id)
            .filter(characters::account_id.eq(i64::from(account)))
            .filter(characters::map_name.eq(map))
            .select((characters::last_x, characters::last_y))
            .first::<(Option<u16>, Option<u16>)>(&self.0.get()?)?;
        Ok(row.0.zip(row.1))
    }

    pub fn save_character_position(
        &self,
        account: u32,
        id: u32,
        map: &str,
        (x, y): (u16, u16),
    ) -> Result<()> {
        let row = characters::table
            .find(id)
            .filter(characters::account_id.eq(i64::from(account)))
            .filter(characters::map_name.eq(map));
        diesel::update(row)
            .set((
                characters::last_x.eq(Some(x)),
                characters::last_y.eq(Some(y)),
            ))
            .execute(&self.0.get()?)?;
        Ok(())
    }

    pub fn characters(&self, account: u32) -> Result<Vec<Character>> {
        Ok(characters::table
            .filter(characters::account_id.eq(i64::from(account)))
            .order(characters::slot.asc())
            .load(&self.0.get()?)?)
    }

    pub fn create_character(&self, character: NewCharacter<'_>) -> Result<Option<Character>> {
        let connection = self.0.get()?;
        let result = connection.transaction::<_, diesel::result::Error, _>(|| {
            diesel::insert_into(characters::table)
                .values(&character)
                .execute(&connection)?;
            characters::table
                .filter(characters::account_id.eq(character.account_id))
                .filter(characters::slot.eq(character.slot))
                .first(&connection)
        });
        match result {
            Ok(created) => Ok(Some(created)),
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}
