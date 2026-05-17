use std::{collections::HashMap, sync::Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::delta::{
    error::DeltaError,
    storage::{AccountKind, DeltaAccountRecord, DeltaAccountUpsert, DeltaRepo},
    utils::time::current_millis,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSession {
    pub kind: String,
    pub session_key: String,
    pub cookie_json: String,
}

#[derive(Debug)]
pub struct DeltaState {
    repo: DeltaRepo,
    buckets: Mutex<HashMap<i64, DeltaAccountRecord>>,
    pending: Mutex<HashMap<String, PendingSession>>,
}

impl DeltaState {
    pub fn initialize<R: Runtime>(app: &AppHandle<R>) -> Result<Self, DeltaError> {
        let app_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| DeltaError::Storage(error.to_string()))?;
        std::fs::create_dir_all(&app_dir)?;
        let repo = DeltaRepo::new(&app_dir.join("delta_accounts.db"))?;
        let buckets = repo
            .list_accounts()?
            .into_iter()
            .map(|record| (record.id, record))
            .collect();

        Ok(Self {
            repo,
            buckets: Mutex::new(buckets),
            pending: Mutex::new(HashMap::new()),
        })
    }

    pub fn list_accounts(&self) -> Result<Vec<DeltaAccountRecord>, DeltaError> {
        self.repo.list_accounts()
    }

    pub fn delete_account(&self, account_id: i64) -> Result<bool, DeltaError> {
        let deleted = self.repo.delete_account(account_id)?;
        if deleted {
            if let Ok(mut buckets) = self.buckets.lock() {
                buckets.remove(&account_id);
            }
        }
        Ok(deleted)
    }

    pub fn persist_account(
        &self,
        kind: AccountKind,
        uin_or_openid: String,
        cookie_json: String,
        openid: Option<String>,
        access_token: Option<String>,
        expires_in: Option<i64>,
        extra_json: Option<String>,
    ) -> Result<DeltaAccountRecord, DeltaError> {
        let now = current_millis();
        let record = self.repo.upsert_account(DeltaAccountUpsert {
            kind,
            uin_or_openid,
            cookie_json,
            openid,
            access_token,
            extra_json,
            expires_at: expires_in.map(|expires_in| now + expires_in * 1000),
            now,
        })?;
        if let Ok(mut buckets) = self.buckets.lock() {
            buckets.insert(record.id, record.clone());
        }
        Ok(record)
    }

    pub fn remember_pending(
        &self,
        kind: &str,
        session_key: String,
        cookie_json: String,
    ) -> Result<(), DeltaError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| DeltaError::Storage("pending lock poisoned".to_string()))?;
        pending.insert(
            session_key.clone(),
            PendingSession {
                kind: kind.to_string(),
                session_key,
                cookie_json,
            },
        );
        Ok(())
    }

    pub fn get_account_cookie(
        &self,
        account_id: Option<i64>,
        cookie: Option<String>,
    ) -> Result<String, DeltaError> {
        if let Some(cookie) = cookie {
            return Ok(cookie);
        }
        let account_id = account_id.ok_or_else(|| DeltaError::InvalidInput("cookie 或 accountId 必填".to_string()))?;
        self.repo
            .get_account(account_id)?
            .map(|record| record.cookie_json)
            .ok_or(DeltaError::AccountNotFound)
    }
}
