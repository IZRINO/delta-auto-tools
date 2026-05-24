use std::{path::Path, sync::Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::delta::error::DeltaError;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    pub fn upsert_account(
        &self,
        payload: DeltaAccountUpsert,
    ) -> Result<DeltaAccountRecord, DeltaError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| DeltaError::Storage("db lock poisoned".to_string()))?;

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
                payload.cookie_json,
                payload.openid,
                payload.access_token,
                payload.extra_json,
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
    Ok(DeltaAccountRecord {
        id: row.get(0)?,
        kind: AccountKind::from_str(&kind)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        uin_or_openid: row.get(2)?,
        cookie_json: row.get(3)?,
        openid: row.get(4)?,
        access_token: row.get(5)?,
        extra_json: row.get(6)?,
        expires_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{AccountKind, DeltaAccountUpsert, DeltaRepo};

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
