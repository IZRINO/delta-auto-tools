use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::delta::{error::DeltaError, storage::secrets};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AccountKind {
    Qq,
    Wechat,
    QqSafe,
    WegameQq,
    WegameWechat,
    Pioneer,
}

impl AccountKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qq => "qq",
            Self::Wechat => "wechat",
            Self::QqSafe => "qqsafe",
            Self::WegameQq => "wegame_qq",
            Self::WegameWechat => "wegame_wechat",
            Self::Pioneer => "pioneer",
        }
    }

    fn from_str(value: &str) -> Result<Self, DeltaError> {
        match value {
            "qq" => Ok(Self::Qq),
            "wechat" => Ok(Self::Wechat),
            "qqsafe" => Ok(Self::QqSafe),
            "wegame_qq" => Ok(Self::WegameQq),
            "wegame_wechat" => Ok(Self::WegameWechat),
            "pioneer" => Ok(Self::Pioneer),
            _ => Err(DeltaError::Parse(format!("unknown account kind: {value}"))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeltaAccountRecord {
    pub id: i64,
    pub kind: AccountKind,
    pub uin_or_openid: String,
    pub cookie_json: String,
    pub openid: Option<String>,
    pub access_token: Option<String>,
    pub extra_json: Option<String>,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeltaAccountView {
    pub id: i64,
    pub kind: AccountKind,
    pub uin_or_openid: String,
    pub has_access_token: bool,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&DeltaAccountRecord> for DeltaAccountView {
    fn from(record: &DeltaAccountRecord) -> Self {
        Self {
            id: record.id,
            kind: record.kind,
            uin_or_openid: record.uin_or_openid.clone(),
            has_access_token: record
                .access_token
                .as_deref()
                .is_some_and(|token| !token.is_empty()),
            expires_at: record.expires_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeltaAccountUpsert {
    pub kind: AccountKind,
    pub uin_or_openid: String,
    pub cookie_json: String,
    pub openid: Option<String>,
    pub access_token: Option<String>,
    pub extra_json: Option<String>,
    pub expires_at: Option<i64>,
    pub now: i64,
}

#[derive(Debug)]
pub struct DeltaRepo {
    conn: Mutex<Connection>,
}

impl DeltaRepo {
    pub fn new(path: &Path) -> Result<Self, DeltaError> {
        let conn = Connection::open(path)?;
        let repo = Self {
            conn: Mutex::new(conn),
        };
        repo.init()?;
        Ok(repo)
    }

    pub fn init(&self) -> Result<(), DeltaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS delta_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                uin_or_openid TEXT NOT NULL,
                cookie_json TEXT NOT NULL,
                openid TEXT,
                access_token TEXT,
                extra_json TEXT,
                expires_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(kind, uin_or_openid)
            );
            ",
        )?;
        Ok(())
    }

    pub fn migrate_plaintext_secrets(&self) -> Result<(), DeltaError> {
        let rows = {
            let conn = self
                .conn
                .lock()
                .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;
            let mut stmt = conn.prepare(
                "
                SELECT id, cookie_json, access_token, extra_json
                FROM delta_accounts
                ",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        for (id, cookie_json, access_token, extra_json) in rows {
            if secrets::is_sealed(&cookie_json)
                && access_token
                    .as_deref()
                    .map(secrets::is_sealed)
                    .unwrap_or(true)
                && extra_json
                    .as_deref()
                    .map(secrets::is_sealed)
                    .unwrap_or(true)
            {
                continue;
            }

            let cookie_json = secrets::open_secret(&cookie_json)?;
            let access_token = access_token
                .as_deref()
                .map(secrets::open_secret)
                .transpose()?;
            let extra_json = extra_json
                .as_deref()
                .map(secrets::open_secret)
                .transpose()?;
            self.rewrite_account_secrets(
                id,
                &cookie_json,
                access_token.as_deref(),
                extra_json.as_deref(),
            )?;
        }
        Ok(())
    }

    pub fn upsert_account(
        &self,
        payload: DeltaAccountUpsert,
    ) -> Result<DeltaAccountRecord, DeltaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;

        let sealed_cookie = secrets::seal_secret(&payload.cookie_json)?;
        let sealed_access_token = payload
            .access_token
            .as_deref()
            .map(secrets::seal_secret)
            .transpose()?;
        let sealed_extra_json = payload
            .extra_json
            .as_deref()
            .map(secrets::seal_secret)
            .transpose()?;

        conn.execute(
            "
            INSERT INTO delta_accounts (
                kind, uin_or_openid, cookie_json, openid, access_token, extra_json, expires_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            ON CONFLICT(kind, uin_or_openid) DO UPDATE SET
                cookie_json = excluded.cookie_json,
                openid = excluded.openid,
                access_token = excluded.access_token,
                extra_json = excluded.extra_json,
                expires_at = excluded.expires_at,
                updated_at = excluded.updated_at
            ",
            params![
                payload.kind.as_str(),
                payload.uin_or_openid,
                sealed_cookie,
                payload.openid,
                sealed_access_token,
                sealed_extra_json,
                payload.expires_at,
                payload.now,
            ],
        )?;

        let id = conn.query_row(
            "SELECT id FROM delta_accounts WHERE kind = ?1 AND uin_or_openid = ?2",
            params![payload.kind.as_str(), payload.uin_or_openid],
            |row| row.get::<_, i64>(0),
        )?;
        drop(conn);
        self.get_account(id)?.ok_or(DeltaError::AccountNotFound)
    }

    pub fn get_account(&self, id: i64) -> Result<Option<DeltaAccountRecord>, DeltaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;
        conn.query_row(
            "
            SELECT id, kind, uin_or_openid, cookie_json, openid, access_token, extra_json, expires_at, created_at, updated_at
            FROM delta_accounts WHERE id = ?1
            ",
            [id],
            map_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_accounts(&self) -> Result<Vec<DeltaAccountRecord>, DeltaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;
        let mut stmt = conn.prepare(
            "
            SELECT id, kind, uin_or_openid, cookie_json, openid, access_token, extra_json, expires_at, created_at, updated_at
            FROM delta_accounts
            ORDER BY updated_at DESC, id DESC
            ",
        )?;
        let rows = stmt.query_map([], map_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn rewrite_account_secrets(
        &self,
        id: i64,
        cookie_json: &str,
        access_token: Option<&str>,
        extra_json: Option<&str>,
    ) -> Result<(), DeltaError> {
        let sealed_cookie = secrets::seal_secret(cookie_json)?;
        let sealed_access_token = access_token.map(secrets::seal_secret).transpose()?;
        let sealed_extra_json = extra_json.map(secrets::seal_secret).transpose()?;

        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;
        conn.execute(
            "
            UPDATE delta_accounts
            SET cookie_json = ?1, access_token = ?2, extra_json = ?3
            WHERE id = ?4
            ",
            params![sealed_cookie, sealed_access_token, sealed_extra_json, id],
        )?;
        Ok(())
    }

    pub fn delete_account(&self, id: i64) -> Result<bool, DeltaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;
        Ok(conn.execute("DELETE FROM delta_accounts WHERE id = ?1", [id])? > 0)
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeltaAccountRecord> {
    let kind = row.get::<_, String>(1)?;
    let cookie_json = open_secret_for_row(row.get::<_, String>(3)?)?;
    let access_token = row
        .get::<_, Option<String>>(5)?
        .map(open_secret_for_row)
        .transpose()?;
    let extra_json = row
        .get::<_, Option<String>>(6)?
        .map(open_secret_for_row)
        .transpose()?;

    Ok(DeltaAccountRecord {
        id: row.get(0)?,
        kind: AccountKind::from_str(&kind)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        uin_or_openid: row.get(2)?,
        cookie_json,
        openid: row.get(4)?,
        access_token,
        extra_json,
        expires_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn open_secret_for_row(value: String) -> rusqlite::Result<String> {
    secrets::open_secret(&value)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{AccountKind, DeltaAccountUpsert, DeltaRepo};
    use crate::delta::storage::secrets;

    #[test]
    fn upserts_and_lists_accounts() {
        let dir = tempdir().unwrap();
        let repo = DeltaRepo::new(&dir.path().join("delta.db")).unwrap();

        let created = repo
            .upsert_account(DeltaAccountUpsert {
                kind: AccountKind::Qq,
                uin_or_openid: "10001".to_string(),
                cookie_json: r#"{"p_skey":"abc"}"#.to_string(),
                openid: Some("openid-1".to_string()),
                access_token: Some("token-1".to_string()),
                extra_json: Some(r#"{"source":"test"}"#.to_string()),
                expires_at: Some(123),
                now: 100,
            })
            .unwrap();

        let updated = repo
            .upsert_account(DeltaAccountUpsert {
                kind: AccountKind::Qq,
                uin_or_openid: "10001".to_string(),
                cookie_json: r#"{"p_skey":"xyz"}"#.to_string(),
                openid: Some("openid-1".to_string()),
                access_token: Some("token-2".to_string()),
                extra_json: None,
                expires_at: Some(456),
                now: 200,
            })
            .unwrap();

        assert_eq!(created.id, updated.id);
        assert!(updated.cookie_json.contains("xyz"));
        assert_eq!(repo.list_accounts().unwrap().len(), 1);
    }

    #[test]
    fn deletes_accounts() {
        let dir = tempdir().unwrap();
        let repo = DeltaRepo::new(&dir.path().join("delta.db")).unwrap();
        let record = repo
            .upsert_account(DeltaAccountUpsert {
                kind: AccountKind::Wechat,
                uin_or_openid: "openid-2".to_string(),
                cookie_json: "{}".to_string(),
                openid: Some("openid-2".to_string()),
                access_token: Some("token".to_string()),
                extra_json: None,
                expires_at: None,
                now: 300,
            })
            .unwrap();

        assert!(repo.delete_account(record.id).unwrap());
        assert!(repo.get_account(record.id).unwrap().is_none());
    }

    #[test]
    fn migrates_plaintext_secrets_without_losing_values() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("delta.db");
        let repo = DeltaRepo::new(&db_path).unwrap();
        let record = repo
            .upsert_account(DeltaAccountUpsert {
                kind: AccountKind::QqSafe,
                uin_or_openid: "10003".to_string(),
                cookie_json: r#"{"uin":"o10003"}"#.to_string(),
                openid: Some("openid-3".to_string()),
                access_token: Some("token-3".to_string()),
                extra_json: Some(r#"{"code":"safe-code"}"#.to_string()),
                expires_at: None,
                now: 500,
            })
            .unwrap();

        {
            let conn = repo.conn.lock().unwrap();
            conn.execute(
                "UPDATE delta_accounts SET cookie_json = ?1, access_token = ?2, extra_json = ?3 WHERE id = ?4",
                rusqlite::params![
                    r#"{"uin":"o10003"}"#,
                    "token-3",
                    r#"{"code":"safe-code"}"#,
                    record.id,
                ],
            )
            .unwrap();
        }

        repo.migrate_plaintext_secrets().unwrap();

        let migrated = repo.get_account(record.id).unwrap().unwrap();
        assert_eq!(migrated.cookie_json, r#"{"uin":"o10003"}"#);
        assert_eq!(migrated.access_token.as_deref(), Some("token-3"));
        assert_eq!(
            migrated.extra_json.as_deref(),
            Some(r#"{"code":"safe-code"}"#)
        );

        let raw_values = {
            let conn = repo.conn.lock().unwrap();
            conn.query_row(
                "SELECT cookie_json, access_token, extra_json FROM delta_accounts WHERE id = ?1",
                [record.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap()
        };
        assert!(secrets::is_sealed(&raw_values.0));
        assert!(secrets::is_sealed(&raw_values.1));
        assert!(secrets::is_sealed(&raw_values.2));
    }

    #[test]
    fn migrates_mixed_sealed_and_plaintext_secrets() {
        let dir = tempdir().unwrap();
        let repo = DeltaRepo::new(&dir.path().join("delta.db")).unwrap();
        let record = repo
            .upsert_account(DeltaAccountUpsert {
                kind: AccountKind::Qq,
                uin_or_openid: "10004".to_string(),
                cookie_json: r#"{"uin":"o10004"}"#.to_string(),
                openid: Some("openid-4".to_string()),
                access_token: Some("token-4".to_string()),
                extra_json: Some(r#"{"appid":"app"}"#.to_string()),
                expires_at: None,
                now: 600,
            })
            .unwrap();

        let sealed_cookie = secrets::test_seal_secret(r#"{"uin":"o10004"}"#).unwrap();
        {
            let conn = repo.conn.lock().unwrap();
            conn.execute(
                "UPDATE delta_accounts SET cookie_json = ?1, access_token = ?2, extra_json = ?3 WHERE id = ?4",
                rusqlite::params![sealed_cookie, "token-4", r#"{"appid":"app"}"#, record.id],
            )
            .unwrap();
        }

        repo.migrate_plaintext_secrets().unwrap();

        let migrated = repo.get_account(record.id).unwrap().unwrap();
        assert_eq!(migrated.cookie_json, r#"{"uin":"o10004"}"#);
        assert_eq!(migrated.access_token.as_deref(), Some("token-4"));
        assert_eq!(migrated.extra_json.as_deref(), Some(r#"{"appid":"app"}"#));
    }
    #[test]
    fn stores_pioneer_accounts() {
        let dir = tempdir().unwrap();
        let repo = DeltaRepo::new(&dir.path().join("delta.db")).unwrap();

        let record = repo
            .upsert_account(DeltaAccountUpsert {
                kind: AccountKind::Pioneer,
                uin_or_openid: "10002".to_string(),
                cookie_json: r#"{"uin":"o10002"}"#.to_string(),
                openid: None,
                access_token: Some("key-1".to_string()),
                extra_json: Some(r#"{"source":"pioneer"}"#.to_string()),
                expires_at: None,
                now: 400,
            })
            .unwrap();

        assert_eq!(record.kind, AccountKind::Pioneer);
        assert_eq!(record.uin_or_openid, "10002");
    }
}
