use std::{collections::HashMap, sync::Mutex};

use tauri::{AppHandle, Manager, Runtime};

use crate::delta::{
    error::DeltaError,
    services::{game::GameAuth, wegame_auth::WegameTicket},
    storage::{AccountKind, DeltaAccountRecord, DeltaAccountUpsert, DeltaAccountView, DeltaRepo},
    utils::time::current_millis,
};

const PENDING_SESSION_TTL_MS: i64 = 5 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct PendingSession {
    pub kind: String,
    pub cookie_json: String,
    pub created_at: i64,
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
        repo.migrate_plaintext_secrets()?;
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

    pub fn list_accounts(&self) -> Result<Vec<DeltaAccountView>, DeltaError> {
        Ok(self
            .repo
            .list_accounts()?
            .iter()
            .map(DeltaAccountView::from)
            .collect())
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
        prune_expired_pending(&mut pending, current_millis());
        pending.insert(
            session_key,
            PendingSession {
                kind: kind.to_string(),
                cookie_json,
                created_at: current_millis(),
            },
        );
        Ok(())
    }

    pub fn pending_cookie(&self, kind: &str, session_key: &str) -> Result<String, DeltaError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| DeltaError::Storage("pending lock poisoned".to_string()))?;
        let now = current_millis();
        prune_expired_pending(&mut pending, now);
        let session = pending
            .get(session_key)
            .ok_or_else(|| DeltaError::InvalidInput("登录会话已过期，请重新扫码".to_string()))?;
        if session.kind != kind {
            return Err(DeltaError::InvalidInput("登录会话类型不匹配".to_string()));
        }
        Ok(session.cookie_json.clone())
    }

    pub fn forget_pending(&self, kind: &str, session_key: &str) -> Result<(), DeltaError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| DeltaError::Storage("pending lock poisoned".to_string()))?;
        if pending
            .get(session_key)
            .is_some_and(|session| session.kind == kind)
        {
            pending.remove(session_key);
        }
        Ok(())
    }
    pub fn consume_pending_cookie(
        &self,
        kind: &str,
        session_key: &str,
    ) -> Result<String, DeltaError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| DeltaError::Storage("pending lock poisoned".to_string()))?;
        let now = current_millis();
        prune_expired_pending(&mut pending, now);
        let session = pending
            .remove(session_key)
            .ok_or_else(|| DeltaError::InvalidInput("登录会话已过期，请重新扫码".to_string()))?;
        if session.kind != kind {
            return Err(DeltaError::InvalidInput("登录会话类型不匹配".to_string()));
        }
        Ok(session.cookie_json)
    }

    pub fn get_account_cookie(&self, account_id: i64) -> Result<String, DeltaError> {
        self.repo
            .get_account(account_id)?
            .map(|record| record.cookie_json)
            .ok_or(DeltaError::AccountNotFound)
    }

    pub fn game_auth(&self, account_id: i64) -> Result<GameAuth, DeltaError> {
        let record = self
            .repo
            .get_account(account_id)?
            .ok_or(DeltaError::AccountNotFound)?;
        if !matches!(record.kind, AccountKind::Qq | AccountKind::Wechat) {
            return Err(DeltaError::InvalidInput(
                "当前账号不能查询游戏数据".to_string(),
            ));
        }
        let openid = record
            .openid
            .filter(|openid| !openid.is_empty())
            .ok_or_else(|| DeltaError::InvalidInput("账号缺少 openid，请重新登录".to_string()))?;
        let access_token = record
            .access_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| DeltaError::InvalidInput("账号缺少访问令牌，请重新登录".to_string()))?;
        Ok(GameAuth {
            openid,
            access_token,
            acctype: if record.kind == AccountKind::Wechat {
                "wx".to_string()
            } else {
                "qc".to_string()
            },
        })
    }

    pub fn wegame_ticket(&self, account_id: i64) -> Result<WegameTicket, DeltaError> {
        let record = self
            .repo
            .get_account(account_id)?
            .ok_or(DeltaError::AccountNotFound)?;
        if !matches!(
            record.kind,
            AccountKind::WegameQq | AccountKind::WegameWechat
        ) {
            return Err(DeltaError::InvalidInput(
                "当前账号不能使用 Wegame 工具".to_string(),
            ));
        }
        let ticket = record
            .access_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                DeltaError::InvalidInput("账号缺少 Wegame 票据，请重新登录".to_string())
            })?;
        Ok(WegameTicket {
            id: record.uin_or_openid,
            ticket,
        })
    }

    pub fn qq_safe_access(&self, account_id: i64) -> Result<(String, String, String), DeltaError> {
        let record = self
            .repo
            .get_account(account_id)?
            .ok_or(DeltaError::AccountNotFound)?;
        if record.kind != AccountKind::QqSafe {
            return Err(DeltaError::InvalidInput(
                "当前账号不能使用 QQ安全中心".to_string(),
            ));
        }
        let openid = record
            .openid
            .filter(|openid| !openid.is_empty())
            .ok_or_else(|| DeltaError::InvalidInput("账号缺少 openid，请重新登录".to_string()))?;
        let access_token = record
            .access_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| DeltaError::InvalidInput("账号缺少访问令牌，请重新登录".to_string()))?;
        let code = record
            .extra_json
            .and_then(|extra| serde_json::from_str::<serde_json::Value>(&extra).ok())
            .and_then(|value| {
                value
                    .get("code")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .filter(|code| !code.is_empty())
            .ok_or_else(|| {
                DeltaError::InvalidInput("账号缺少 QQ安全中心 code，请重新登录".to_string())
            })?;
        Ok((openid, access_token, code))
    }

    pub fn pioneer_key(&self, account_id: i64) -> Result<String, DeltaError> {
        let record = self
            .repo
            .get_account(account_id)?
            .ok_or(DeltaError::AccountNotFound)?;
        if record.kind != AccountKind::Pioneer {
            return Err(DeltaError::InvalidInput(
                "当前账号不能使用先遣服工具".to_string(),
            ));
        }
        record
            .access_token
            .filter(|token| !token.is_empty())
            .ok_or_else(|| DeltaError::InvalidInput("账号缺少先遣服 key，请重新登录".to_string()))
    }
}

fn prune_expired_pending(pending: &mut HashMap<String, PendingSession>, now: i64) {
    pending.retain(|_, session| now.saturating_sub(session.created_at) <= PENDING_SESSION_TTL_MS);
}
