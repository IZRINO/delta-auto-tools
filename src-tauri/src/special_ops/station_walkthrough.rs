use serde::{Deserialize, Serialize};

use super::AccountPlan;

pub(crate) const NEXT_ACCOUNT_HOTKEY_SCOPE: &str = "special-ops-next-account";

pub(crate) const ERR_TOOL_DISABLED: &str = "请先打开特勤处总开关";
pub(crate) const ERR_NOT_PAUSED: &str = "请先暂停轮换";
pub(crate) const ERR_GLOBAL_DISABLED: &str = "全局自动化已关闭";
pub(crate) const ERR_RUN_ACTIVE: &str = "特勤处试运行尚未完成清理";
pub(crate) const ERR_EMPTY_HOTKEY: &str = "请先录制下一账号快捷键";
pub(crate) const ERR_HOTKEY_CONFLICT: &str = "下一账号快捷键不能与紧急停止相同";
pub(crate) const ERR_NO_ACCOUNTS: &str = "没有启用账号";
pub(crate) const ERR_UNPAUSE_WHILE_OPEN: &str = "请先关闭多账号制作台更改";
pub(crate) const ERR_TRIAL_WHILE_OPEN: &str = "多账号制作台更改打开期间不能启动试运行";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StationWalkthroughSnapshot {
    pub account_index: usize,
    pub account_total: usize,
    pub qq_account: Option<String>,
    pub exhausted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextAccountAction {
    Ignore,
    Login { account_id: String },
    CloseOnly,
}

pub struct WalkthroughEnableGate<'a> {
    pub tool_enabled: bool,
    pub paused: bool,
    pub global_automation_enabled: bool,
    pub run_active: bool,
    pub emergency_hotkey: &'a str,
    pub next_account_hotkey: &'a str,
    pub accounts: &'a [AccountPlan],
}

#[derive(Debug, Default)]
pub struct StationWalkthroughSession {
    enabled: bool,
    current_account_id: Option<String>,
    current_order: u32,
    current_qq: Option<String>,
    waiting: bool,
    exhausted: bool,
}

pub fn eligible_accounts(accounts: &[AccountPlan]) -> Vec<&AccountPlan> {
    let mut eligible: Vec<&AccountPlan> = accounts
        .iter()
        .filter(|account| account.enabled && is_pure_digit_qq(&account.qq_account))
        .collect();
    eligible.sort_by_key(|account| account.order);
    eligible
}

pub fn enable_error(gate: WalkthroughEnableGate<'_>) -> Option<String> {
    if !gate.tool_enabled {
        return Some(ERR_TOOL_DISABLED.to_string());
    }
    if !gate.global_automation_enabled {
        return Some(ERR_GLOBAL_DISABLED.to_string());
    }
    if !gate.paused {
        return Some(ERR_NOT_PAUSED.to_string());
    }
    if gate.run_active {
        return Some(ERR_RUN_ACTIVE.to_string());
    }
    let hotkey = gate.next_account_hotkey.trim();
    if hotkey.is_empty() {
        return Some(ERR_EMPTY_HOTKEY.to_string());
    }
    if hotkey == gate.emergency_hotkey.trim() {
        return Some(ERR_HOTKEY_CONFLICT.to_string());
    }
    if eligible_accounts(gate.accounts).is_empty() {
        return Some(ERR_NO_ACCOUNTS.to_string());
    }
    None
}

pub fn next_account_hotkey_conflicts(next: &str, emergency: &str) -> bool {
    let next = next.trim();
    !next.is_empty() && next == emergency.trim()
}

impl StationWalkthroughSession {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn current_account_id(&self) -> Option<&str> {
        self.current_account_id.as_deref()
    }

    pub fn snapshot(&self, accounts: &[AccountPlan]) -> Option<StationWalkthroughSnapshot> {
        if !self.enabled {
            return None;
        }
        let eligible = eligible_accounts(accounts);
        let account_total = eligible.len();
        if self.exhausted {
            return Some(StationWalkthroughSnapshot {
                account_index: account_total,
                account_total,
                qq_account: self.current_qq.clone(),
                exhausted: true,
            });
        }
        let account_index = eligible
            .iter()
            .position(|account| Some(account.id.as_str()) == self.current_account_id.as_deref())
            .map(|index| index + 1)
            .unwrap_or(1);
        Some(StationWalkthroughSnapshot {
            account_index,
            account_total,
            qq_account: self.current_qq.clone(),
            exhausted: false,
        })
    }

    pub fn enable_from_first(&mut self, accounts: &[AccountPlan]) -> Result<String, String> {
        let eligible = eligible_accounts(accounts);
        let first = eligible
            .first()
            .ok_or_else(|| ERR_NO_ACCOUNTS.to_string())?;
        self.enabled = true;
        self.waiting = false;
        self.exhausted = false;
        self.set_current(first);
        Ok(first.id.clone())
    }

    pub fn restore_enabled_exhausted(&mut self) {
        self.enabled = true;
        self.waiting = false;
        self.exhausted = true;
    }

    pub fn mark_arrived(&mut self) {
        if self.enabled && !self.exhausted {
            self.waiting = true;
        }
    }

    pub fn disable(&mut self) {
        *self = Self::default();
    }

    pub fn next_action(&self, run_active: bool, accounts: &[AccountPlan]) -> NextAccountAction {
        if !self.enabled || run_active || self.exhausted || !self.waiting {
            return NextAccountAction::Ignore;
        }
        let eligible = eligible_accounts(accounts);
        if let Some(position) = eligible
            .iter()
            .position(|account| Some(account.id.as_str()) == self.current_account_id.as_deref())
        {
            if let Some(next) = eligible.get(position + 1) {
                return NextAccountAction::Login {
                    account_id: next.id.clone(),
                };
            }
            return NextAccountAction::CloseOnly;
        }
        eligible
            .iter()
            .find(|account| account.order > self.current_order)
            .map(|account| NextAccountAction::Login {
                account_id: account.id.clone(),
            })
            .unwrap_or(NextAccountAction::CloseOnly)
    }

    pub fn begin_login(
        &mut self,
        accounts: &[AccountPlan],
        account_id: &str,
    ) -> Result<(), String> {
        let eligible = eligible_accounts(accounts);
        let account = eligible
            .iter()
            .find(|item| item.id == account_id)
            .ok_or_else(|| ERR_NO_ACCOUNTS.to_string())?;
        self.enabled = true;
        self.waiting = false;
        self.exhausted = false;
        self.set_current(account);
        Ok(())
    }

    pub fn mark_exhausted(&mut self) {
        self.waiting = false;
        self.exhausted = true;
    }

    fn set_current(&mut self, account: &AccountPlan) {
        self.current_account_id = Some(account.id.clone());
        self.current_order = account.order;
        self.current_qq = Some(account.qq_account.clone());
    }
}

fn is_pure_digit_qq(qq: &str) -> bool {
    !qq.is_empty() && qq.chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::super::AccountStatus;
    use super::*;

    fn account(id: &str, qq: &str, enabled: bool, order: u32) -> AccountPlan {
        AccountPlan {
            id: id.to_string(),
            qq_account: qq.to_string(),
            enabled,
            initialized: true,
            order,
            status: AccountStatus::Ready,
            independent_settings_enabled: false,
            independent_business_config: None,
            stations: Vec::new(),
            ammo_targets: Vec::new(),
            last_failure: None,
            login_trial_signature: None,
            limited_supply: Default::default(),
            market: Default::default(),
        }
    }

    fn gate<'a>(
        accounts: &'a [AccountPlan],
        next_hotkey: &'a str,
        paused: bool,
    ) -> WalkthroughEnableGate<'a> {
        WalkthroughEnableGate {
            tool_enabled: true,
            paused,
            global_automation_enabled: true,
            run_active: false,
            emergency_hotkey: "Ctrl+Shift+F12",
            next_account_hotkey: next_hotkey,
            accounts,
        }
    }

    #[test]
    fn settings_default_walkthrough_off_when_fields_missing() {
        let settings: super::super::SpecialOpsSettings =
            serde_json::from_value(serde_json::json!({
                "enabled": true,
                "paused": true,
                "dailyExchangeTime": "08:00",
                "emergencyHotkey": "Ctrl+Shift+F12",
                "accounts": []
            }))
            .unwrap();
        assert!(!settings.station_walkthrough_enabled);
        assert!(settings.next_account_hotkey.is_empty());
    }

    #[test]
    fn eligible_accounts_skip_disabled_and_invalid_qq_then_sort_by_order() {
        let accounts = vec![
            account("c", "10003", true, 2),
            account("disabled", "10001", false, 0),
            account("bad", "qq", true, 1),
            account("empty", "", true, 1),
            account("a", "10002", true, 0),
        ];
        let ids: Vec<&str> = eligible_accounts(&accounts)
            .iter()
            .map(|account| account.id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "c"]);
    }

    #[test]
    fn enable_error_covers_gates() {
        let accounts = vec![account("a", "10001", true, 0)];
        assert_eq!(
            enable_error(WalkthroughEnableGate {
                tool_enabled: false,
                ..gate(&accounts, "F8", true)
            }),
            Some(ERR_TOOL_DISABLED.to_string())
        );
        assert_eq!(
            enable_error(WalkthroughEnableGate {
                global_automation_enabled: false,
                ..gate(&accounts, "F8", true)
            }),
            Some(ERR_GLOBAL_DISABLED.to_string())
        );
        assert_eq!(
            enable_error(gate(&accounts, "F8", false)),
            Some(ERR_NOT_PAUSED.to_string())
        );
        assert_eq!(
            enable_error(WalkthroughEnableGate {
                run_active: true,
                ..gate(&accounts, "F8", true)
            }),
            Some(ERR_RUN_ACTIVE.to_string())
        );
        assert_eq!(
            enable_error(gate(&accounts, "", true)),
            Some(ERR_EMPTY_HOTKEY.to_string())
        );
        assert_eq!(
            enable_error(gate(&accounts, "  Ctrl+Shift+F12  ", true)),
            Some(ERR_HOTKEY_CONFLICT.to_string())
        );
        assert_eq!(
            enable_error(gate(&[], "F8", true)),
            Some(ERR_NO_ACCOUNTS.to_string())
        );
        assert_eq!(enable_error(gate(&accounts, "F8", true)), None);
        assert!(next_account_hotkey_conflicts("F8", "F8"));
        assert!(!next_account_hotkey_conflicts("F8", "F9"));
        assert!(!next_account_hotkey_conflicts("", "F8"));
        assert_eq!(NEXT_ACCOUNT_HOTKEY_SCOPE, "special-ops-next-account");
        assert!(!ERR_UNPAUSE_WHILE_OPEN.is_empty());
        assert!(!ERR_TRIAL_WHILE_OPEN.is_empty());
    }

    #[test]
    fn session_enable_starts_first_account_and_next_hotkey_advances() {
        let accounts = vec![
            account("disabled", "1", false, 0),
            account("a", "10001", true, 1),
            account("b", "10002", true, 2),
        ];
        let mut session = StationWalkthroughSession::default();
        assert_eq!(session.enable_from_first(&accounts).unwrap(), "a");
        assert_eq!(session.current_account_id(), Some("a"));
        assert_eq!(
            session.next_action(false, &accounts),
            NextAccountAction::Ignore
        );
        session.mark_arrived();
        assert_eq!(
            session.next_action(true, &accounts),
            NextAccountAction::Ignore
        );
        assert_eq!(
            session.next_action(false, &accounts),
            NextAccountAction::Login {
                account_id: "b".to_string()
            }
        );
        session.begin_login(&accounts, "b").unwrap();
        session.mark_arrived();
        assert_eq!(
            session.next_action(false, &accounts),
            NextAccountAction::CloseOnly
        );
        session.mark_exhausted();
        assert_eq!(
            session.next_action(false, &accounts),
            NextAccountAction::Ignore
        );
        assert_eq!(
            session.snapshot(&accounts),
            Some(StationWalkthroughSnapshot {
                account_index: 2,
                account_total: 2,
                qq_account: Some("10002".to_string()),
                exhausted: true,
            })
        );
    }

    #[test]
    fn disable_clears_session_and_reenable_starts_from_first() {
        let accounts = vec![
            account("a", "10001", true, 0),
            account("b", "10002", true, 1),
        ];
        let mut session = StationWalkthroughSession::default();
        session.enable_from_first(&accounts).unwrap();
        session.mark_arrived();
        session.begin_login(&accounts, "b").unwrap();
        session.disable();
        assert!(!session.is_enabled());
        assert_eq!(session.snapshot(&accounts), None);
        assert_eq!(session.enable_from_first(&accounts).unwrap(), "a");
    }

    #[test]
    fn restore_enabled_is_exhausted_until_toggled() {
        let accounts = vec![account("a", "10001", true, 0)];
        let mut session = StationWalkthroughSession::default();
        session.restore_enabled_exhausted();
        assert!(session.is_enabled());
        assert_eq!(
            session.next_action(false, &accounts),
            NextAccountAction::Ignore
        );
        assert_eq!(
            session.snapshot(&accounts),
            Some(StationWalkthroughSnapshot {
                account_index: 1,
                account_total: 1,
                qq_account: None,
                exhausted: true,
            })
        );
    }

    #[test]
    fn next_skips_account_disabled_while_waiting() {
        let mut accounts = vec![
            account("a", "10001", true, 0),
            account("b", "10002", true, 1),
            account("c", "10003", true, 2),
        ];
        let mut session = StationWalkthroughSession::default();
        session.enable_from_first(&accounts).unwrap();
        session.mark_arrived();
        accounts[1].enabled = false;
        assert_eq!(
            session.next_action(false, &accounts),
            NextAccountAction::Login {
                account_id: "c".to_string()
            }
        );
    }
}
