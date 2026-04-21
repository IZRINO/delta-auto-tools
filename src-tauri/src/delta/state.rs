use std::{collections::HashMap, sync::Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::delta::{
    client::http::HttpOptions,
    error::DeltaError,
    storage::{DeltaAccountRecord, DeltaRepo},
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
    pub repo: DeltaRepo,
    pub buckets: Mutex<HashMap<i64, DeltaAccountRecord>>,
    pub pending: Mutex<HashMap<String, PendingSession>>,
    pub http_options: Mutex<HttpOptions>,
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
            http_options: Mutex::new(HttpOptions::default()),
        })
    }
}
