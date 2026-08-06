use super::{
    build_schedule_with_profit, resolve_account_business_config, AccountStatus, SpecialOpsSettings,
    StationKind,
};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AmmoProfitGate {
    Disabled,
    Qualified(HashSet<String>),
    CutoffBypass,
    DisplayOnly,
}

impl AmmoProfitGate {
    pub(crate) fn allows(&self, profit_rule_id: Option<&str>) -> bool {
        match self {
            Self::Qualified(qualified_rule_ids) => {
                profit_rule_id.is_some_and(|rule_id| qualified_rule_ids.contains(rule_id))
            }
            Self::Disabled | Self::CutoffBypass | Self::DisplayOnly => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoundTrigger {
    Manual,
    Scheduled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountRoundTask {
    pub account_id: String,
    pub qq_account: String,
    pub account_order: u32,
    pub stations: Vec<StationKind>,
    pub ammo_target_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RoundPlan {
    pub created_at_ms: i64,
    pub trigger: RoundTrigger,
    pub accounts: Vec<AccountRoundTask>,
}

#[cfg(test)]
pub(crate) fn build_round_plan(
    settings: &SpecialOpsSettings,
    created_at_ms: i64,
    trigger: RoundTrigger,
) -> Result<RoundPlan, String> {
    build_round_plan_with_profit(settings, created_at_ms, trigger, AmmoProfitGate::Disabled)
}

pub(crate) fn build_round_plan_with_profit(
    settings: &SpecialOpsSettings,
    created_at_ms: i64,
    trigger: RoundTrigger,
    gate: AmmoProfitGate,
) -> Result<RoundPlan, String> {
    for account in settings.accounts.iter().filter(|account| {
        account.enabled && account.initialized && account.status == AccountStatus::Ready
    }) {
        resolve_account_business_config(settings, account)?;
    }
    let schedule = build_schedule_with_profit(settings, created_at_ms, &gate);
    let mut accounts = schedule
        .due_accounts
        .into_iter()
        .map(|due| {
            let account = settings
                .accounts
                .iter()
                .find(|account| account.id == due.account_id)
                .ok_or_else(|| "到期任务账号不存在".to_string())?;
            Ok(AccountRoundTask {
                account_id: account.id.clone(),
                qq_account: account.qq_account.clone(),
                account_order: account.order,
                stations: due.station_kinds,
                ammo_target_ids: due.ammo_target_ids,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    accounts.sort_by(|left, right| {
        (left.account_order, &left.account_id).cmp(&(right.account_order, &right.account_id))
    });
    Ok(RoundPlan {
        created_at_ms,
        trigger,
        accounts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::{
        AccountFailure, AccountPlan, AccountStatus, AmmoBusinessTarget, AmmoTarget, BusinessConfig,
        CalibrationRect, ScrollDirection, SpecialOpsSettings, StationBusinessConfig, StationKind,
        StationPlan, StationStatus,
    };

    fn station(kind: StationKind, finishes_at_ms: i64) -> StationPlan {
        StationPlan {
            kind,
            enabled: true,
            item_name: String::new(),
            duration_minutes: 60,
            started_at_ms: Some(1),
            finishes_at_ms: Some(finishes_at_ms),
            status: StationStatus::Crafting,
        }
    }

    fn account(
        id: &str,
        order: u32,
        status: AccountStatus,
        stations: Vec<StationPlan>,
    ) -> AccountPlan {
        AccountPlan {
            id: id.to_string(),
            qq_account: format!("100{order}"),
            enabled: true,
            initialized: true,
            order,
            status,
            independent_settings_enabled: false,
            independent_business_config: None,
            stations,
            ammo_targets: Vec::new(),
            last_failure: None,
            login_trial_signature: None,
        }
    }

    fn settings() -> SpecialOpsSettings {
        let mut settings = SpecialOpsSettings {
            paused: false,
            accounts: vec![
                account(
                    "b",
                    2,
                    AccountStatus::Ready,
                    vec![station(StationKind::Workbench, 900)],
                ),
                account(
                    "a",
                    1,
                    AccountStatus::Ready,
                    vec![
                        station(StationKind::ArmorBench, 900),
                        station(StationKind::TechnicalCenter, 900),
                        station(StationKind::Pharmacy, 1_001),
                    ],
                ),
                account(
                    "isolated",
                    0,
                    AccountStatus::Isolated,
                    vec![station(StationKind::TechnicalCenter, 900)],
                ),
            ],
            ..SpecialOpsSettings::default()
        };
        settings.default_business_config = BusinessConfig {
            stations: StationKind::all()
                .into_iter()
                .map(|kind| StationBusinessConfig {
                    kind,
                    enabled: true,
                    duration_minutes: 60,
                    recipe_note: String::new(),
                })
                .collect(),
            recipe_points: Vec::new(),
            ammo_targets: vec![AmmoBusinessTarget {
                id: "ignored-ammo".to_string(),
                note: "不进入制作轮次".to_string(),
                enabled: true,
                seasonal: false,
                click_point: None,
                scroll_direction: ScrollDirection::Down,
                scroll_steps: 0,
                order: 0,
                profit_rule_id: None,
            }],
        };
        settings
    }

    #[test]
    fn plans_due_work_by_account_then_fixed_station_order() {
        let plan = build_round_plan(&settings(), 1_000, RoundTrigger::Scheduled).unwrap();

        assert_eq!(
            plan.accounts
                .iter()
                .map(|account| account.account_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            plan.accounts[0].stations,
            [StationKind::TechnicalCenter, StationKind::ArmorBench]
        );
        assert_eq!(plan.accounts[0].ammo_target_ids, ["ignored-ammo"]);
        assert_eq!(plan.created_at_ms, 1_000);
        assert_eq!(plan.trigger, RoundTrigger::Scheduled);
    }

    #[test]
    fn missing_independent_business_config_is_reported() {
        let mut settings = settings();
        settings.accounts[0].independent_settings_enabled = true;

        let error = build_round_plan(&settings, 1_000, RoundTrigger::Manual).unwrap_err();

        assert!(error.contains("独立业务配置缺失"));
    }

    #[test]
    fn schedule_next_wake_ignores_ineligible_accounts() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets.clear();
        settings.accounts[0].stations[0].finishes_at_ms = Some(2_000);
        settings.accounts[0].stations[0].status = StationStatus::Ready;
        settings.accounts[1].enabled = false;

        assert_eq!(
            crate::special_ops::build_schedule(&settings, 1_000).next_wake_at_ms,
            Some(2_000)
        );
        settings.accounts[0].status = AccountStatus::Uncertain;
        assert_eq!(
            crate::special_ops::build_schedule(&settings, 1_000).next_wake_at_ms,
            None
        );
    }

    #[test]
    fn ready_station_with_due_time_enters_round() {
        let mut settings = settings();
        settings.accounts[1].stations[0].status = StationStatus::Ready;

        let plan = build_round_plan(&settings, 1_000, RoundTrigger::Manual).unwrap();

        assert!(plan.accounts[0].stations.contains(&StationKind::ArmorBench));
    }

    #[test]
    fn round_plan_merges_due_craft_and_today_ammo_for_same_account() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets[0].click_point = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 1,
            height: 1,
        });
        settings.accounts[1].ammo_targets.push(AmmoTarget {
            id: "ignored-ammo".to_string(),
            name: String::new(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: None,
            retry_day: None,
            retry_count: 0,
            last_failure: None,
        });

        let plan = build_round_plan(&settings, 1_000, RoundTrigger::Scheduled).unwrap();
        let account = plan
            .accounts
            .iter()
            .find(|item| item.account_id == "a")
            .unwrap();
        assert!(!account.stations.is_empty());
        assert_eq!(account.ammo_target_ids, ["ignored-ammo"]);
    }

    #[test]
    fn round_plan_skips_ammo_succeeded_or_exhausted_today() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets[0].click_point = Some(CalibrationRect {
            x: 10,
            y: 20,
            width: 1,
            height: 1,
        });
        settings.accounts[1].ammo_targets = vec![AmmoTarget {
            id: "ignored-ammo".to_string(),
            name: String::new(),
            enabled: true,
            seasonal: false,
            scroll_steps: 0,
            order: 0,
            last_success_day: Some("1970-01-01".to_string()),
            retry_day: None,
            retry_count: 0,
            last_failure: None,
        }];

        let succeeded = build_round_plan(&settings, 0, RoundTrigger::Scheduled).unwrap();
        assert!(succeeded
            .accounts
            .iter()
            .find(|account| account.account_id == "a")
            .is_none_or(|account| account.ammo_target_ids.is_empty()));

        settings.accounts[1].ammo_targets[0].last_success_day = None;
        settings.accounts[1].ammo_targets[0].retry_day = Some("1970-01-01".to_string());
        settings.accounts[1].ammo_targets[0].retry_count = 2;
        let exhausted = build_round_plan(&settings, 0, RoundTrigger::Scheduled).unwrap();
        assert!(exhausted
            .accounts
            .iter()
            .find(|account| account.account_id == "a")
            .is_none_or(|account| account.ammo_target_ids.is_empty()));
    }

    #[test]
    fn failed_ammo_is_skipped_while_other_due_work_remains() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets = vec![
            AmmoBusinessTarget {
                id: "ammo-a".to_string(),
                note: "子弹 A".to_string(),
                enabled: true,
                seasonal: false,
                click_point: None,
                scroll_direction: ScrollDirection::Down,
                scroll_steps: 0,
                order: 0,
                profit_rule_id: None,
            },
            AmmoBusinessTarget {
                id: "ammo-b".to_string(),
                note: "子弹 B".to_string(),
                enabled: true,
                seasonal: false,
                click_point: None,
                scroll_direction: ScrollDirection::Down,
                scroll_steps: 0,
                order: 1,
                profit_rule_id: None,
            },
        ];
        settings.accounts[1].ammo_targets = vec![
            AmmoTarget {
                id: "ammo-a".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: Some(AccountFailure {
                    step: "ammo.success".to_string(),
                    message: "未确认完成".to_string(),
                    at_ms: 10,
                    station_kind: None,
                    ammo_target_id: Some("ammo-a".to_string()),
                }),
            },
            AmmoTarget {
                id: "ammo-b".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 1,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
        ];

        let plan = build_round_plan(&settings, 1_000, RoundTrigger::Scheduled).unwrap();
        let account = plan
            .accounts
            .iter()
            .find(|account| account.account_id == "a")
            .unwrap();

        assert_eq!(
            account.stations,
            [StationKind::TechnicalCenter, StationKind::ArmorBench,]
        );
        assert_eq!(account.ammo_target_ids, ["ammo-b"]);
    }

    #[test]
    fn profit_gate_filters_only_ammo_and_cutoff_bypasses_it() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets = vec![
            AmmoBusinessTarget {
                id: "ammo-a".to_string(),
                note: "子弹 A".to_string(),
                enabled: true,
                seasonal: false,
                click_point: Some(CalibrationRect {
                    x: 1,
                    y: 1,
                    width: 1,
                    height: 1,
                }),
                scroll_direction: ScrollDirection::Down,
                scroll_steps: 0,
                order: 0,
                profit_rule_id: Some("rule-a".to_string()),
            },
            AmmoBusinessTarget {
                id: "ammo-b".to_string(),
                note: "子弹 B".to_string(),
                enabled: true,
                seasonal: false,
                click_point: Some(CalibrationRect {
                    x: 2,
                    y: 1,
                    width: 1,
                    height: 1,
                }),
                scroll_direction: ScrollDirection::Down,
                scroll_steps: 0,
                order: 1,
                profit_rule_id: Some("rule-b".to_string()),
            },
        ];
        settings.accounts[1].ammo_targets = vec![
            AmmoTarget {
                id: "ammo-a".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 0,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
            AmmoTarget {
                id: "ammo-b".to_string(),
                name: String::new(),
                enabled: true,
                seasonal: false,
                scroll_steps: 0,
                order: 1,
                last_success_day: None,
                retry_day: None,
                retry_count: 0,
                last_failure: None,
            },
        ];

        let qualified = build_round_plan_with_profit(
            &settings,
            1_000,
            RoundTrigger::Scheduled,
            AmmoProfitGate::Qualified(std::collections::HashSet::from(["rule-a".to_string()])),
        )
        .unwrap();
        let qualified_account = qualified
            .accounts
            .iter()
            .find(|account| account.account_id == "a")
            .unwrap();
        assert_eq!(qualified_account.ammo_target_ids, ["ammo-a"]);
        assert!(!qualified_account.stations.is_empty());

        let cutoff = build_round_plan_with_profit(
            &settings,
            1_000,
            RoundTrigger::Scheduled,
            AmmoProfitGate::CutoffBypass,
        )
        .unwrap();
        assert_eq!(
            cutoff
                .accounts
                .iter()
                .find(|account| account.account_id == "a")
                .unwrap()
                .ammo_target_ids,
            ["ammo-a", "ammo-b"]
        );
    }
}
