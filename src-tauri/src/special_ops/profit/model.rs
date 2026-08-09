use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::super::{daily_exchange_minutes, AccountPlan, BusinessConfig, SpecialOpsSettings};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfitFilterSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_profit_cutoff_time")]
    pub cutoff_time: String,
    #[serde(default)]
    pub rules: Vec<AmmoProfitRule>,
    #[serde(default)]
    pub audits: Vec<AmmoProfitAudit>,
    #[serde(default)]
    pub cutoff_state: Option<ProfitCutoffState>,
}

impl Default for ProfitFilterSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            cutoff_time: default_profit_cutoff_time(),
            rules: Vec::new(),
            audits: Vec::new(),
            cutoff_state: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfitCutoffState {
    pub day: String,
    pub targets: Vec<ProfitCutoffTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfitCutoffTarget {
    pub account_id: String,
    pub target_id: String,
    #[serde(default)]
    pub rule_id: Option<String>,
    #[serde(default)]
    pub skip_reason: Option<ProfitCutoffSkipReason>,
    #[serde(default)]
    pub decided_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfitCutoffSkipReason {
    BelowThreshold,
    QueryUnavailable,
    Unconfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AmmoProfitRule {
    pub id: String,
    pub display_name: String,
    pub kkrb_match_name: String,
    #[serde(default)]
    pub moligod_match_name: Option<String>,
    pub minimum_profit: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfitSource {
    Kkrb,
    Moligod,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfitAuditOutcome {
    Qualified,
    BelowThreshold,
    TargetMissing,
    SourceFailure,
    Unconfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AmmoProfitAudit {
    pub rule_id: String,
    pub day: String,
    pub queried_at_ms: i64,
    pub source: Option<ProfitSource>,
    pub attempted_sources: Vec<ProfitSource>,
    pub source_data_at: Option<String>,
    pub source_version: Option<String>,
    pub profit: Option<i64>,
    pub threshold: u64,
    pub outcome: ProfitAuditOutcome,
    pub detail: String,
    pub next_query_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfitConfigurationUpdate {
    pub enabled: bool,
    pub cutoff_time: String,
    pub rules: Vec<AmmoProfitRule>,
    pub bindings: Vec<ProfitTargetBinding>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfitTargetBinding {
    pub account_id: Option<String>,
    pub target_id: String,
    pub profit_rule_id: Option<String>,
}

pub fn default_profit_cutoff_time() -> String {
    "17:00".to_string()
}

pub(crate) fn profit_qualifies(actual_profit: i64, minimum_profit: u64) -> bool {
    u64::try_from(actual_profit).is_ok_and(|profit| profit >= minimum_profit)
}

pub(crate) fn validate_profit_rules(rules: &[AmmoProfitRule]) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut kkrb_names = HashSet::new();
    let mut moligod_names = HashSet::new();
    for rule in rules {
        let id = rule.id.trim();
        if id.is_empty() || !ids.insert(id) {
            return Err("利润规则 ID 必须非空且唯一".to_string());
        }
        if rule.display_name.trim().is_empty() {
            return Err(format!("利润规则 {id} 的显示名称不能为空"));
        }
        let kkrb_name = rule.kkrb_match_name.trim();
        if kkrb_name.is_empty() {
            return Err(format!("利润规则 {id} 的 KKRB 精确名称不能为空"));
        }
        if !kkrb_names.insert(kkrb_name) {
            return Err(format!("KKRB 精确名称重复：{kkrb_name}"));
        }
        if let Some(moligod_name) = rule.moligod_match_name.as_deref() {
            let moligod_name = moligod_name.trim();
            if moligod_name.is_empty() {
                return Err(format!("利润规则 {id} 的 Moligod 精确名称不能为空"));
            }
            if !moligod_names.insert(moligod_name) {
                return Err(format!("Moligod 精确名称重复：{moligod_name}"));
            }
        }
    }
    Ok(())
}

pub(crate) fn normalize_profit_settings(settings: &mut ProfitFilterSettings) -> Result<(), String> {
    settings.cutoff_time = settings.cutoff_time.trim().to_string();
    for rule in &mut settings.rules {
        rule.id = rule.id.trim().to_string();
        rule.display_name = rule.display_name.trim().to_string();
        rule.kkrb_match_name = rule.kkrb_match_name.trim().to_string();
        rule.moligod_match_name = rule
            .moligod_match_name
            .take()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
    }
    let rule_ids = settings
        .rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect::<HashSet<_>>();
    for audit in &mut settings.audits {
        audit.rule_id = audit.rule_id.trim().to_string();
        audit.day = audit.day.trim().to_string();
        audit.source_data_at = audit
            .source_data_at
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        audit.source_version = audit
            .source_version
            .take()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        audit.detail = audit.detail.trim().to_string();
    }
    settings
        .audits
        .retain(|audit| rule_ids.contains(audit.rule_id.as_str()));
    if let Some(state) = settings.cutoff_state.as_mut() {
        state.day = state.day.trim().to_string();
        let mut seen = HashSet::new();
        for target in &mut state.targets {
            target.account_id = target.account_id.trim().to_string();
            target.target_id = target.target_id.trim().to_string();
            target.rule_id = target
                .rule_id
                .take()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            if target.decided_at_ms.is_none()
                && target
                    .rule_id
                    .as_deref()
                    .is_none_or(|rule_id| !rule_ids.contains(rule_id))
            {
                target.skip_reason = Some(ProfitCutoffSkipReason::Unconfigured);
                target.decided_at_ms = Some(0);
            }
        }
        state.targets.retain(|target| {
            !target.account_id.is_empty()
                && !target.target_id.is_empty()
                && seen.insert((target.account_id.clone(), target.target_id.clone()))
        });
    }
    Ok(())
}

pub(crate) fn validate_profit_configuration(
    filter: &ProfitFilterSettings,
    daily_exchange_time: &str,
    default_business_config: &BusinessConfig,
    accounts: &[AccountPlan],
) -> Result<(), String> {
    if !filter.enabled {
        return Ok(());
    }
    let exchange_minutes = daily_exchange_minutes(daily_exchange_time)
        .ok_or_else(|| "每日兑换时间必须是 HH:mm，范围 00:00-23:59".to_string())?;
    let cutoff_minutes = daily_exchange_minutes(&filter.cutoff_time)
        .ok_or_else(|| "利润截止时间必须是 HH:mm，范围 00:00-23:59".to_string())?;
    if cutoff_minutes <= exchange_minutes {
        return Err("利润截止时间必须晚于每日兑换时间".to_string());
    }
    validate_profit_rules(&filter.rules)?;
    let rule_ids = filter
        .rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect::<HashSet<_>>();
    validate_business_references(default_business_config, &rule_ids, "默认配置")?;
    for account in accounts
        .iter()
        .filter(|account| account.independent_settings_enabled)
    {
        let business = account
            .independent_business_config
            .as_ref()
            .ok_or_else(|| {
                format!(
                    "账号 {} 已开启独立设置，但独立业务配置缺失",
                    account.qq_account
                )
            })?;
        validate_business_references(business, &rule_ids, &format!("账号 {}", account.qq_account))?;
    }
    Ok(())
}

fn validate_business_references(
    business: &BusinessConfig,
    rule_ids: &HashSet<&str>,
    owner: &str,
) -> Result<(), String> {
    if let Some(target) = business.ammo_targets.iter().find(|target| {
        target
            .profit_rule_id
            .as_deref()
            .is_some_and(|rule_id| !rule_ids.contains(rule_id))
    }) {
        return Err(format!(
            "{owner} 的子弹目标 {} 引用了不存在的利润规则",
            target.id
        ));
    }
    Ok(())
}

pub(crate) fn apply_profit_configuration(
    current: &SpecialOpsSettings,
    update: ProfitConfigurationUpdate,
    validated_moligod_names: &HashSet<String>,
) -> Result<SpecialOpsSettings, String> {
    let mut next = current.clone();
    let retained_rule_ids = update
        .rules
        .iter()
        .map(|rule| rule.id.trim().to_string())
        .collect::<HashSet<_>>();
    let deleted_rule_ids = current
        .profit_filter
        .rules
        .iter()
        .map(|rule| rule.id.trim())
        .filter(|rule_id| !retained_rule_ids.contains(*rule_id))
        .map(str::to_string)
        .collect::<HashSet<_>>();

    next.profit_filter.enabled = update.enabled;
    next.profit_filter.cutoff_time = update.cutoff_time;
    next.profit_filter.rules = update.rules;
    normalize_profit_settings(&mut next.profit_filter)?;
    validate_new_moligod_names(
        &current.profit_filter.rules,
        &next.profit_filter.rules,
        validated_moligod_names,
    )?;
    clear_deleted_profit_references(&mut next, &deleted_rule_ids);
    apply_profit_binding_updates(&mut next, update.bindings)?;
    validate_profit_configuration_for_save(&next)?;
    Ok(next)
}

fn validate_new_moligod_names(
    current_rules: &[AmmoProfitRule],
    next_rules: &[AmmoProfitRule],
    validated_moligod_names: &HashSet<String>,
) -> Result<(), String> {
    let current_names = current_rules
        .iter()
        .map(|rule| {
            (
                rule.id.trim(),
                rule.moligod_match_name.as_deref().map(str::trim),
            )
        })
        .collect::<HashMap<_, _>>();
    for rule in next_rules {
        let Some(name) = rule.moligod_match_name.as_deref() else {
            continue;
        };
        if current_names.get(rule.id.as_str()).copied().flatten() == Some(name) {
            continue;
        }
        if !validated_moligod_names.contains(name) {
            return Err(format!(
                "Moligod 精确名称尚未验证：{}",
                rule.moligod_match_name.as_deref().unwrap_or_default()
            ));
        }
    }
    Ok(())
}

fn clear_deleted_profit_references(
    settings: &mut SpecialOpsSettings,
    deleted_rule_ids: &HashSet<String>,
) {
    clear_business_references(&mut settings.default_business_config, deleted_rule_ids);
    for account in &mut settings.accounts {
        if let Some(business) = account.independent_business_config.as_mut() {
            clear_business_references(business, deleted_rule_ids);
        }
    }
    settings
        .profit_filter
        .audits
        .retain(|audit| !deleted_rule_ids.contains(&audit.rule_id));
}

fn clear_business_references(business: &mut BusinessConfig, deleted_rule_ids: &HashSet<String>) {
    for target in &mut business.ammo_targets {
        if target
            .profit_rule_id
            .as_ref()
            .is_some_and(|rule_id| deleted_rule_ids.contains(rule_id))
        {
            target.profit_rule_id = None;
        }
    }
}

fn apply_profit_binding_updates(
    settings: &mut SpecialOpsSettings,
    bindings: Vec<ProfitTargetBinding>,
) -> Result<(), String> {
    let rule_ids = settings
        .profit_filter
        .rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect::<HashSet<_>>();
    let mut binding_keys = HashSet::new();
    for binding in bindings {
        let account_id = binding.account_id.map(|value| value.trim().to_string());
        if account_id.as_deref() == Some("") {
            return Err("利润目标绑定的账号 ID 不能为空".to_string());
        }
        let target_id = binding.target_id.trim().to_string();
        if target_id.is_empty() {
            return Err("利润目标绑定的子弹目标 ID 不能为空".to_string());
        }
        let key = (account_id.clone(), target_id.clone());
        if !binding_keys.insert(key) {
            return Err("利润目标绑定键重复".to_string());
        }
        let profit_rule_id = binding
            .profit_rule_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if profit_rule_id
            .as_deref()
            .is_some_and(|rule_id| !rule_ids.contains(rule_id))
        {
            return Err(format!("子弹目标 {target_id} 引用了不存在的利润规则"));
        }

        let business = if let Some(account_id) = account_id.as_deref() {
            let account = settings
                .accounts
                .iter_mut()
                .find(|account| account.id == account_id)
                .ok_or_else(|| format!("利润目标绑定账号不存在：{account_id}"))?;
            if !account.independent_settings_enabled {
                return Err(format!("账号 {} 未开启独立设置", account.qq_account));
            }
            account
                .independent_business_config
                .as_mut()
                .ok_or_else(|| {
                    format!(
                        "账号 {} 已开启独立设置，但独立业务配置缺失",
                        account.qq_account
                    )
                })?
        } else {
            &mut settings.default_business_config
        };
        let target = business
            .ammo_targets
            .iter_mut()
            .find(|target| target.id == target_id)
            .ok_or_else(|| format!("利润目标绑定的子弹目标不存在：{target_id}"))?;
        target.profit_rule_id = profit_rule_id;
    }
    Ok(())
}

pub(crate) fn validate_profit_configuration_for_save(
    settings: &SpecialOpsSettings,
) -> Result<(), String> {
    let mut filter = settings.profit_filter.clone();
    filter.enabled = true;
    validate_profit_configuration(
        &filter,
        &settings.daily_exchange_time,
        &settings.default_business_config,
        &settings.accounts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::{
        AccountPlan, AccountStatus, AmmoBusinessTarget, AmmoTarget, BusinessConfig,
        CalibrationRect, ScrollDirection,
    };

    fn rule(
        id: &str,
        kkrb_match_name: &str,
        moligod_match_name: Option<&str>,
        minimum_profit: u64,
    ) -> AmmoProfitRule {
        AmmoProfitRule {
            id: id.to_string(),
            display_name: format!("规则 {id}"),
            kkrb_match_name: kkrb_match_name.to_string(),
            moligod_match_name: moligod_match_name.map(str::to_string),
            minimum_profit,
        }
    }

    fn audit(rule_id: &str, queried_at_ms: i64) -> AmmoProfitAudit {
        AmmoProfitAudit {
            rule_id: rule_id.to_string(),
            day: "2026-08-02".to_string(),
            queried_at_ms,
            source: Some(ProfitSource::Kkrb),
            attempted_sources: vec![ProfitSource::Kkrb],
            source_data_at: None,
            source_version: Some("v1".to_string()),
            profit: Some(100),
            threshold: 100,
            outcome: ProfitAuditOutcome::Qualified,
            detail: "达标".to_string(),
            next_query_at_ms: None,
        }
    }

    fn business_target(id: &str, profit_rule_id: Option<&str>) -> AmmoBusinessTarget {
        AmmoBusinessTarget {
            id: id.to_string(),
            note: format!("目标 {id}"),
            enabled: true,
            seasonal: false,
            click_point: Some(CalibrationRect {
                x: 10,
                y: 20,
                width: 1,
                height: 1,
            }),
            scroll_direction: ScrollDirection::Down,
            scroll_steps: 3,
            order: 0,
            profit_rule_id: profit_rule_id.map(str::to_string),
        }
    }

    fn runtime_target(id: &str) -> AmmoTarget {
        AmmoTarget {
            id: id.to_string(),
            name: format!("目标 {id}"),
            enabled: true,
            seasonal: false,
            scroll_steps: 3,
            order: 0,
            last_success_day: Some("2026-08-02".to_string()),
            retry_day: Some("2026-08-02".to_string()),
            retry_count: 1,
            last_failure: None,
        }
    }

    fn settings_with_profit_configuration() -> SpecialOpsSettings {
        let mut settings = SpecialOpsSettings::default();
        settings.default_business_config.stations[0].duration_minutes = 123;
        settings.default_business_config.stations[0].recipe_note = "保留制作配置".to_string();
        settings.default_business_config.ammo_targets =
            vec![business_target("default-a", Some("rule-a"))];
        settings.profit_filter = ProfitFilterSettings {
            enabled: true,
            cutoff_time: "17:00".to_string(),
            rules: vec![
                rule("rule-a", "KKRB A", Some("Moligod A"), 100),
                rule("rule-b", "KKRB B", None, 200),
            ],
            audits: vec![audit("rule-a", 1), audit("rule-b", 2)],
            cutoff_state: None,
        };
        let independent = BusinessConfig {
            ammo_targets: vec![business_target("independent-a", Some("rule-a"))],
            ..BusinessConfig::default()
        };
        settings.accounts.push(AccountPlan {
            id: "account-a".to_string(),
            qq_account: "10001".to_string(),
            enabled: true,
            initialized: true,
            order: 0,
            status: AccountStatus::Ready,
            independent_settings_enabled: true,
            independent_business_config: Some(independent),
            stations: Vec::new(),
            ammo_targets: vec![runtime_target("independent-a")],
            last_failure: None,
            login_trial_signature: None,
            limited_supply: Default::default(),
            market: Default::default(),
        });
        settings
    }

    #[test]
    fn equal_profit_qualifies_but_negative_profit_never_does() {
        assert!(profit_qualifies(100, 100));
        assert!(profit_qualifies(101, 100));
        assert!(!profit_qualifies(99, 100));
        assert!(!profit_qualifies(-1, 0));
    }

    #[test]
    fn old_settings_default_to_disabled_profit_filter_and_unbound_targets() {
        let mut legacy = serde_json::to_value(SpecialOpsSettings::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("profitFilter");
        legacy["defaultBusinessConfig"]["ammoTargets"] = serde_json::json!([{
            "id": "ammo-a",
            "note": "目标 A",
            "enabled": true,
            "seasonal": false,
            "clickPoint": null,
            "scrollDirection": "down",
            "scrollSteps": 0,
            "order": 0
        }]);

        let settings: SpecialOpsSettings = serde_json::from_value(legacy).unwrap();

        assert!(!settings.profit_filter.enabled);
        assert_eq!(settings.profit_filter.cutoff_time, "17:00");
        assert!(settings.default_business_config.ammo_targets[0]
            .profit_rule_id
            .is_none());
    }

    #[test]
    fn duplicate_exact_names_are_rejected() {
        let error = validate_profit_rules(&[
            rule("a", "同名", Some("Moligod A"), 1),
            rule("b", "同名", Some("Moligod B"), 1),
        ])
        .unwrap_err();
        assert!(error.contains("KKRB 精确名称重复"));

        let error = validate_profit_rules(&[
            rule("a", "KKRB A", Some("Moligod 同名"), 1),
            rule("b", "KKRB B", Some("Moligod 同名"), 1),
        ])
        .unwrap_err();
        assert!(error.contains("Moligod 精确名称重复"));
    }

    #[test]
    fn rule_ids_and_required_names_must_be_non_empty_and_unique() {
        assert!(validate_profit_rules(&[rule("", "目标 A", None, 1)])
            .unwrap_err()
            .contains("规则 ID"));
        assert!(validate_profit_rules(&[
            rule("same", "目标 A", None, 1),
            rule("same", "目标 B", None, 1),
        ])
        .unwrap_err()
        .contains("规则 ID"));
        assert!(validate_profit_rules(&[rule("a", " ", None, 1)])
            .unwrap_err()
            .contains("KKRB 精确名称"));
    }

    #[test]
    fn cutoff_must_be_valid_and_strictly_after_exchange_time() {
        let business = BusinessConfig::default();
        let accounts = Vec::new();
        let mut filter = ProfitFilterSettings {
            enabled: true,
            cutoff_time: "08:00".to_string(),
            rules: Vec::new(),
            audits: Vec::new(),
            cutoff_state: None,
        };

        assert!(
            validate_profit_configuration(&filter, "08:00", &business, &accounts)
                .unwrap_err()
                .contains("必须晚于每日兑换时间")
        );
        filter.cutoff_time = "17:0".to_string();
        assert!(
            validate_profit_configuration(&filter, "08:00", &business, &accounts)
                .unwrap_err()
                .contains("HH:mm")
        );
        filter.cutoff_time = "17:00".to_string();
        assert!(validate_profit_configuration(&filter, "08:00", &business, &accounts).is_ok());
    }

    #[test]
    fn dangling_profit_rule_reference_is_rejected() {
        let mut business = BusinessConfig::default();
        business.ammo_targets.push(AmmoBusinessTarget {
            id: "ammo-a".to_string(),
            note: "目标 A".to_string(),
            enabled: true,
            seasonal: false,
            click_point: None,
            scroll_direction: ScrollDirection::Down,
            scroll_steps: 0,
            order: 0,
            profit_rule_id: Some("missing".to_string()),
        });
        let filter = ProfitFilterSettings {
            enabled: true,
            cutoff_time: "17:00".to_string(),
            rules: vec![rule("existing", "目标 A", None, 1)],
            audits: Vec::new(),
            cutoff_state: None,
        };

        assert!(
            validate_profit_configuration(&filter, "08:00", &business, &[])
                .unwrap_err()
                .contains("不存在的利润规则")
        );
    }

    #[test]
    fn normalization_trims_rules_prunes_deleted_audits_and_is_idempotent() {
        let mut filter = ProfitFilterSettings {
            enabled: true,
            cutoff_time: " 17:00 ".to_string(),
            rules: vec![AmmoProfitRule {
                id: " rule-a ".to_string(),
                display_name: " 规则 A ".to_string(),
                kkrb_match_name: " KKRB A ".to_string(),
                moligod_match_name: Some(" Moligod A ".to_string()),
                minimum_profit: 100,
            }],
            audits: vec![audit("rule-a", 2), audit("deleted", 3)],
            cutoff_state: None,
        };

        normalize_profit_settings(&mut filter).unwrap();
        let once = filter.clone();
        normalize_profit_settings(&mut filter).unwrap();

        assert_eq!(filter, once);
        assert_eq!(filter.cutoff_time, "17:00");
        assert_eq!(filter.rules[0].id, "rule-a");
        assert_eq!(filter.rules[0].display_name, "规则 A");
        assert_eq!(filter.rules[0].kkrb_match_name, "KKRB A");
        assert_eq!(
            filter.rules[0].moligod_match_name.as_deref(),
            Some("Moligod A")
        );
        assert_eq!(filter.audits.len(), 1);
        assert_eq!(filter.audits[0].rule_id, "rule-a");
    }

    #[test]
    fn apply_profit_configuration_changes_only_profit_fields_and_bindings() {
        let current = settings_with_profit_configuration();
        let update = ProfitConfigurationUpdate {
            enabled: true,
            cutoff_time: "18:00".to_string(),
            rules: vec![
                rule("rule-a", "KKRB A", Some("Moligod A"), 150),
                rule("rule-b", "KKRB B", None, 200),
            ],
            bindings: vec![
                ProfitTargetBinding {
                    account_id: None,
                    target_id: "default-a".to_string(),
                    profit_rule_id: Some("rule-b".to_string()),
                },
                ProfitTargetBinding {
                    account_id: Some("account-a".to_string()),
                    target_id: "independent-a".to_string(),
                    profit_rule_id: None,
                },
            ],
        };

        let next =
            apply_profit_configuration(&current, update, &HashSet::from(["Moligod A".to_string()]))
                .unwrap();

        assert_eq!(next.profit_filter.cutoff_time, "18:00");
        assert_eq!(next.profit_filter.rules[0].minimum_profit, 150);
        assert_eq!(
            next.default_business_config.ammo_targets[0]
                .profit_rule_id
                .as_deref(),
            Some("rule-b")
        );
        assert!(next.accounts[0]
            .independent_business_config
            .as_ref()
            .unwrap()
            .ammo_targets[0]
            .profit_rule_id
            .is_none());
        assert_eq!(
            next.default_business_config.stations,
            current.default_business_config.stations
        );
        assert_eq!(
            next.default_business_config.ammo_targets[0].click_point,
            current.default_business_config.ammo_targets[0].click_point
        );
        assert_eq!(
            next.accounts[0].ammo_targets,
            current.accounts[0].ammo_targets
        );
    }

    #[test]
    fn deleting_rule_clears_all_references_and_its_audit_atomically() {
        let current = settings_with_profit_configuration();
        let update = ProfitConfigurationUpdate {
            enabled: true,
            cutoff_time: "17:00".to_string(),
            rules: vec![rule("rule-b", "KKRB B", None, 200)],
            bindings: Vec::new(),
        };

        let next = apply_profit_configuration(&current, update, &HashSet::new()).unwrap();

        assert!(next.default_business_config.ammo_targets[0]
            .profit_rule_id
            .is_none());
        assert!(next.accounts[0]
            .independent_business_config
            .as_ref()
            .unwrap()
            .ammo_targets[0]
            .profit_rule_id
            .is_none());
        assert_eq!(next.profit_filter.audits.len(), 1);
        assert_eq!(next.profit_filter.audits[0].rule_id, "rule-b");
    }

    #[test]
    fn invalid_or_duplicate_binding_rejects_update_without_touching_current() {
        let current = settings_with_profit_configuration();
        let duplicate = ProfitConfigurationUpdate {
            enabled: true,
            cutoff_time: "17:00".to_string(),
            rules: current.profit_filter.rules.clone(),
            bindings: vec![
                ProfitTargetBinding {
                    account_id: None,
                    target_id: "default-a".to_string(),
                    profit_rule_id: Some("rule-b".to_string()),
                },
                ProfitTargetBinding {
                    account_id: None,
                    target_id: "default-a".to_string(),
                    profit_rule_id: None,
                },
            ],
        };

        assert!(
            apply_profit_configuration(&current, duplicate, &HashSet::new())
                .unwrap_err()
                .contains("重复")
        );
        assert_eq!(
            current.default_business_config.ammo_targets[0]
                .profit_rule_id
                .as_deref(),
            Some("rule-a")
        );

        for invalid in [
            ProfitTargetBinding {
                account_id: None,
                target_id: "missing-target".to_string(),
                profit_rule_id: Some("rule-a".to_string()),
            },
            ProfitTargetBinding {
                account_id: Some("missing-account".to_string()),
                target_id: "independent-a".to_string(),
                profit_rule_id: Some("rule-a".to_string()),
            },
            ProfitTargetBinding {
                account_id: None,
                target_id: "default-a".to_string(),
                profit_rule_id: Some("missing-rule".to_string()),
            },
        ] {
            let update = ProfitConfigurationUpdate {
                enabled: true,
                cutoff_time: "17:00".to_string(),
                rules: current.profit_filter.rules.clone(),
                bindings: vec![invalid],
            };
            assert!(apply_profit_configuration(&current, update, &HashSet::new()).is_err());
        }
    }

    #[test]
    fn changed_moligod_name_requires_current_process_validation() {
        let current = settings_with_profit_configuration();
        let changed = ProfitConfigurationUpdate {
            enabled: true,
            cutoff_time: "17:00".to_string(),
            rules: vec![
                rule("rule-a", "KKRB A", Some("Moligod 新名称"), 100),
                rule("rule-b", "KKRB B", None, 200),
            ],
            bindings: Vec::new(),
        };

        assert!(
            apply_profit_configuration(&current, changed.clone(), &HashSet::new())
                .unwrap_err()
                .contains("尚未验证")
        );
        assert!(apply_profit_configuration(
            &current,
            changed,
            &HashSet::from(["Moligod 新名称".to_string()]),
        )
        .is_ok());

        let unchanged = ProfitConfigurationUpdate {
            enabled: true,
            cutoff_time: "17:00".to_string(),
            rules: current.profit_filter.rules.clone(),
            bindings: Vec::new(),
        };
        assert!(apply_profit_configuration(&current, unchanged, &HashSet::new()).is_ok());
    }

    #[test]
    fn cutoff_state_defaults_normalizes_and_survives_configuration_changes() {
        let mut filter: ProfitFilterSettings =
            serde_json::from_str(r#"{"enabled":true,"cutoffTime":"17:00","rules":[],"audits":[]}"#)
                .unwrap();
        assert_eq!(filter.cutoff_state, None);
        filter.cutoff_state = Some(ProfitCutoffState {
            day: " 2026-08-06 ".to_string(),
            targets: vec![
                ProfitCutoffTarget {
                    account_id: " account-a ".to_string(),
                    target_id: " ammo-a ".to_string(),
                    rule_id: Some(" rule-a ".to_string()),
                    skip_reason: None,
                    decided_at_ms: None,
                },
                ProfitCutoffTarget {
                    account_id: "account-a".to_string(),
                    target_id: "ammo-a".to_string(),
                    rule_id: Some("rule-a".to_string()),
                    skip_reason: Some(ProfitCutoffSkipReason::BelowThreshold),
                    decided_at_ms: Some(1),
                },
                ProfitCutoffTarget {
                    account_id: "".to_string(),
                    target_id: "ammo-b".to_string(),
                    rule_id: None,
                    skip_reason: None,
                    decided_at_ms: None,
                },
            ],
        });

        normalize_profit_settings(&mut filter).unwrap();

        let state = filter.cutoff_state.as_ref().unwrap();
        assert_eq!(state.day, "2026-08-06");
        assert_eq!(state.targets.len(), 1);
        assert_eq!(state.targets[0].account_id, "account-a");
        assert_eq!(state.targets[0].target_id, "ammo-a");
        assert_eq!(
            state.targets[0].skip_reason,
            Some(ProfitCutoffSkipReason::Unconfigured)
        );
        assert_eq!(state.targets[0].decided_at_ms, Some(0));

        let mut current = settings_with_profit_configuration();
        current.profit_filter.cutoff_state = filter.cutoff_state.clone();
        let update = ProfitConfigurationUpdate {
            enabled: true,
            cutoff_time: "18:00".to_string(),
            rules: current.profit_filter.rules.clone(),
            bindings: Vec::new(),
        };
        let next = apply_profit_configuration(&current, update, &HashSet::new()).unwrap();
        assert_eq!(
            next.profit_filter.cutoff_state,
            current.profit_filter.cutoff_state
        );
    }
}
