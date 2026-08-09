use super::{
    build_schedule_with_profit, resolve_account_business_config, AccountStatus, SpecialOpsSettings,
    StationKind,
};
use std::collections::{HashMap, HashSet};

const SESSION_CHAIN_WINDOW_MS: i64 = 10 * 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AmmoProfitGate {
    Disabled,
    Qualified(HashSet<String>),
    QualifiedTargets(HashSet<(String, String)>),
    DisplayOnly,
}

impl AmmoProfitGate {
    pub(crate) fn allows(
        &self,
        account_id: &str,
        target_id: &str,
        profit_rule_id: Option<&str>,
    ) -> bool {
        match self {
            Self::Qualified(qualified_rule_ids) => {
                profit_rule_id.is_some_and(|rule_id| qualified_rule_ids.contains(rule_id))
            }
            Self::QualifiedTargets(targets) => {
                targets.contains(&(account_id.to_string(), target_id.to_string()))
            }
            Self::Disabled | Self::DisplayOnly => true,
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
    pub scheduled_at_ms: i64,
    pub stations: Vec<StationKind>,
    pub ammo_target_ids: Vec<String>,
    pub limited_supply_cycle_id: Option<String>,
    pub market_purchase_day: Option<String>,
}

pub(crate) fn can_chain_follow_up(
    current: &AccountRoundTask,
    next: &AccountRoundTask,
    now_ms: i64,
) -> bool {
    current.account_id == next.account_id && should_continue_round(current, next, now_ms)
}

pub(crate) fn should_continue_round(
    current: &AccountRoundTask,
    next: &AccountRoundTask,
    now_ms: i64,
) -> bool {
    next.scheduled_at_ms <= now_ms
        || next.scheduled_at_ms.saturating_sub(current.scheduled_at_ms) <= SESSION_CHAIN_WINDOW_MS
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
    let due_accounts = schedule
        .due_accounts
        .iter()
        .map(|due| (due.account_id.as_str(), due))
        .collect::<HashMap<_, _>>();
    let eligible_accounts = settings
        .accounts
        .iter()
        .filter(|account| {
            account.enabled && account.initialized && account.status == AccountStatus::Ready
        })
        .map(|account| (account.id.as_str(), account))
        .collect::<HashMap<_, _>>();
    let mut due_tasks_by_account = HashMap::<String, Vec<super::TimelineTask>>::new();
    let mut future_tasks = Vec::<super::TimelineTask>::new();

    for task in schedule.timeline_tasks {
        let Some(_account) = eligible_accounts.get(task.account_id.as_str()) else {
            continue;
        };
        let due = due_accounts.get(task.account_id.as_str());
        let is_due = task.scheduled_at_ms <= created_at_ms
            && match (&task.kind, &task.station_kind, &task.ammo_target_id) {
                (super::TimelineTaskKind::Craft, Some(kind), _) => due
                    .is_some_and(|due| due.station_kinds.iter().any(|candidate| candidate == kind)),
                (super::TimelineTaskKind::Ammo, _, Some(target_id)) => due.is_some_and(|due| {
                    due.ammo_target_ids
                        .iter()
                        .any(|candidate| candidate == target_id)
                }),
                (super::TimelineTaskKind::LimitedSupplyCheck, _, _) => {
                    due.is_some_and(|due| due.limited_supply_due)
                }
                (super::TimelineTaskKind::MarketPurchase, _, _) => {
                    due.is_some_and(|due| due.market_purchase_due)
                }
                _ => false,
            };
        let is_future_craft = task.scheduled_at_ms > created_at_ms
            && task.kind == super::TimelineTaskKind::Craft
            && task.manual_failure.is_none();
        if !is_due && !is_future_craft {
            continue;
        }
        if is_due {
            due_tasks_by_account
                .entry(task.account_id.clone())
                .or_default()
                .push(task);
        } else if is_future_craft {
            future_tasks.push(task);
        }
    }

    let mut accounts = Vec::new();
    for (account_id, tasks) in due_tasks_by_account {
        let Some(account) = eligible_accounts.get(account_id.as_str()) else {
            continue;
        };
        let business = resolve_account_business_config(settings, account)?;
        accounts.push(merge_account_tasks(account, business, tasks));
    }
    accounts.sort_by_key(|task| (task.account_order, task.scheduled_at_ms));

    if !accounts.is_empty() {
        let mut future_accounts = Vec::<AccountRoundTask>::new();
        for task in future_tasks {
            let Some(account) = eligible_accounts.get(task.account_id.as_str()) else {
                continue;
            };
            let same_bucket = future_accounts.last_mut().filter(|candidate| {
                candidate.account_id == task.account_id
                    && candidate.scheduled_at_ms == task.scheduled_at_ms
            });
            let entry = match same_bucket {
                Some(entry) => entry,
                None => {
                    future_accounts.push(AccountRoundTask {
                        account_id: task.account_id.clone(),
                        qq_account: task.qq_account.clone(),
                        account_order: account.order,
                        scheduled_at_ms: task.scheduled_at_ms,
                        stations: Vec::new(),
                        ammo_target_ids: Vec::new(),
                        limited_supply_cycle_id: None,
                        market_purchase_day: None,
                    });
                    future_accounts.last_mut().expect("刚插入未来轮次任务")
                }
            };
            if let (super::TimelineTaskKind::Craft, Some(kind), _) =
                (task.kind, task.station_kind, task.ammo_target_id)
            {
                entry.stations.push(kind);
            }
        }
        accounts.extend(future_accounts);
    }

    accounts.retain(|task| {
        !task.stations.is_empty()
            || !task.ammo_target_ids.is_empty()
            || task.limited_supply_cycle_id.is_some()
            || task.market_purchase_day.is_some()
    });
    Ok(RoundPlan {
        created_at_ms,
        trigger,
        accounts,
    })
}

fn merge_account_tasks(
    account: &super::AccountPlan,
    business: &super::BusinessConfig,
    tasks: Vec<super::TimelineTask>,
) -> AccountRoundTask {
    let scheduled_at_ms = tasks
        .iter()
        .map(|task| task.scheduled_at_ms)
        .min()
        .unwrap_or_default();
    let mut merged = AccountRoundTask {
        account_id: account.id.clone(),
        qq_account: account.qq_account.clone(),
        account_order: account.order,
        scheduled_at_ms,
        stations: Vec::new(),
        ammo_target_ids: Vec::new(),
        limited_supply_cycle_id: None,
        market_purchase_day: None,
    };
    for task in tasks {
        match (task.kind, task.station_kind, task.ammo_target_id) {
            (super::TimelineTaskKind::Craft, Some(kind), _) => merged.stations.push(kind),
            (super::TimelineTaskKind::Ammo, _, Some(target_id)) => {
                merged.ammo_target_ids.push(target_id)
            }
            (super::TimelineTaskKind::LimitedSupplyCheck, _, _) => {
                merged.limited_supply_cycle_id = task.limited_cycle_id;
            }
            (super::TimelineTaskKind::MarketPurchase, _, _) => {
                merged.market_purchase_day =
                    Some(super::local_day_and_minute(task.scheduled_at_ms).0);
            }
            _ => {}
        }
    }
    merged.stations.sort_by_key(|kind| {
        StationKind::all()
            .iter()
            .position(|candidate| candidate == kind)
            .unwrap_or(usize::MAX)
    });
    merged.ammo_target_ids.sort_by_key(|target_id| {
        business
            .ammo_targets
            .iter()
            .position(|target| target.id == *target_id)
            .unwrap_or(usize::MAX)
    });
    merged
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
            limited_supply: Default::default(),
            market: Default::default(),
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
            market: Default::default(),
        };
        settings
    }

    fn scheduled(account_id: &str, account_order: u32, scheduled_at_ms: i64) -> AccountRoundTask {
        AccountRoundTask {
            account_id: account_id.to_string(),
            qq_account: format!("100{account_order}"),
            account_order,
            scheduled_at_ms,
            stations: vec![StationKind::TechnicalCenter],
            ammo_target_ids: Vec::new(),
            limited_supply_cycle_id: None,
            market_purchase_day: None,
        }
    }

    fn shanghai_ms(value: &str) -> i64 {
        let offset = chrono::FixedOffset::east_opt(8 * 60 * 60).unwrap();
        chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")
            .unwrap()
            .and_local_timezone(offset)
            .single()
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn planner_freezes_limited_cycle_and_market_day() {
        let mut settings = settings();
        settings.accounts.truncate(1);
        settings.accounts[0].stations.clear();
        settings.paused = false;
        settings.limited_supply.enabled = true;
        settings.default_business_config.market.enabled = true;
        let now_ms = shanghai_ms("2026-08-08 02:30");

        let plan = build_round_plan(&settings, now_ms, RoundTrigger::Scheduled).unwrap();

        assert!(plan
            .accounts
            .iter()
            .any(|task| { task.limited_supply_cycle_id.as_deref() == Some("2026-08-07T20:00") }));
        assert!(plan
            .accounts
            .iter()
            .any(|task| { task.market_purchase_day.as_deref() == Some("2026-08-08") }));
    }

    #[test]
    fn planner_merges_all_due_business_into_one_account_bucket() {
        let mut settings = settings();
        settings.accounts.truncate(1);
        settings.paused = false;
        settings.daily_exchange_time = "02:00".to_string();
        settings.limited_supply.enabled = true;
        settings.default_business_config.market.enabled = true;
        settings.accounts[0].stations[0].finishes_at_ms = Some(0);
        settings.accounts[0].stations[0].status = StationStatus::Ready;
        let now_ms = shanghai_ms("2026-08-08 02:30");

        let plan = build_round_plan(&settings, now_ms, RoundTrigger::Scheduled).unwrap();

        let bucket = plan
            .accounts
            .iter()
            .find(|task| task.account_id == settings.accounts[0].id)
            .unwrap();
        assert!(!bucket.stations.is_empty());
        assert_eq!(bucket.ammo_target_ids, ["ignored-ammo"]);
        assert_eq!(
            bucket.limited_supply_cycle_id.as_deref(),
            Some("2026-08-07T20:00")
        );
        assert_eq!(bucket.market_purchase_day.as_deref(), Some("2026-08-08"));
    }

    #[test]
    fn same_account_follow_up_at_or_before_ten_minutes_can_chain() {
        let current = scheduled("account-1", 0, 10 * 60_000);
        let next = scheduled("account-1", 0, 20 * 60_000);

        assert!(can_chain_follow_up(&current, &next, 10 * 60_000));
    }

    #[test]
    fn same_account_follow_up_after_ten_minutes_requires_new_session() {
        let current = scheduled("account-1", 0, 10 * 60_000);
        let next = scheduled("account-1", 0, 20 * 60_000 + 1);

        assert!(!can_chain_follow_up(&current, &next, 10 * 60_000));
    }

    #[test]
    fn different_account_never_chains_even_when_due_within_ten_minutes() {
        let current = scheduled("account-1", 0, 10 * 60_000);
        let next = scheduled("account-2", 1, 11 * 60_000);

        assert!(!can_chain_follow_up(&current, &next, 10 * 60_000));
    }

    #[test]
    fn overdue_same_account_follow_up_reuses_session() {
        let current = scheduled("account-1", 0, 10 * 60_000);
        let next = scheduled("account-1", 0, 40 * 60_000);

        assert!(can_chain_follow_up(&current, &next, 50 * 60_000));
    }

    #[test]
    fn due_tasks_group_by_account_order() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets.clear();
        settings.accounts = vec![
            account(
                "account-1",
                0,
                AccountStatus::Ready,
                vec![
                    station(StationKind::TechnicalCenter, 10 * 60_000),
                    station(StationKind::Workbench, 14 * 60_000),
                    station(StationKind::Pharmacy, 18 * 60_000),
                ],
            ),
            account(
                "account-2",
                1,
                AccountStatus::Ready,
                vec![station(StationKind::ArmorBench, 12 * 60_000)],
            ),
        ];

        let plan = build_round_plan(&settings, 20 * 60_000, RoundTrigger::Scheduled).unwrap();

        assert_eq!(
            plan.accounts
                .iter()
                .map(|task| task.account_id.as_str())
                .collect::<Vec<_>>(),
            ["account-1", "account-2"]
        );
        assert_eq!(
            plan.accounts[0].stations,
            [
                StationKind::TechnicalCenter,
                StationKind::Workbench,
                StationKind::Pharmacy,
            ]
        );
        assert_eq!(plan.accounts[1].stations, [StationKind::ArmorBench]);
    }

    #[test]
    fn plans_due_account_buckets_then_future_work_in_time_order() {
        let plan = build_round_plan(&settings(), 1_000, RoundTrigger::Scheduled).unwrap();

        assert_eq!(
            plan.accounts
                .iter()
                .map(|account| account.account_id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "a"]
        );
        assert_eq!(
            plan.accounts[0].stations,
            [StationKind::TechnicalCenter, StationKind::ArmorBench]
        );
        assert_eq!(plan.accounts[0].ammo_target_ids, ["ignored-ammo"]);
        assert_eq!(plan.accounts[2].stations, [StationKind::Pharmacy]);
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
    fn round_plan_excludes_due_craft_from_non_ready_account() {
        let mut settings = settings();
        settings.default_business_config.ammo_targets.clear();
        settings.accounts[0].stations[0].finishes_at_ms = Some(500);
        settings.accounts[0].stations[0].status = StationStatus::Ready;
        settings.accounts[0].status = AccountStatus::Uncertain;

        let plan = build_round_plan(&settings, 1_000, RoundTrigger::Scheduled).unwrap();

        assert!(plan
            .accounts
            .iter()
            .all(|task| task.account_id != settings.accounts[0].id));
    }

    #[test]
    fn ready_station_with_due_time_enters_round() {
        let mut settings = settings();
        settings.accounts[1].stations[0].status = StationStatus::Ready;

        let plan = build_round_plan(&settings, 1_000, RoundTrigger::Manual).unwrap();

        assert!(plan
            .accounts
            .iter()
            .any(|task| task.stations.contains(&StationKind::ArmorBench)));
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
        let tasks = plan
            .accounts
            .iter()
            .filter(|item| item.account_id == "a")
            .collect::<Vec<_>>();
        assert!(tasks.iter().any(|task| !task.stations.is_empty()));
        assert!(tasks
            .iter()
            .any(|task| task.ammo_target_ids == ["ignored-ammo"]));
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
        let craft_task = plan
            .accounts
            .iter()
            .find(|task| task.account_id == "a" && !task.stations.is_empty())
            .unwrap();
        let ammo_task = plan
            .accounts
            .iter()
            .find(|task| task.account_id == "a" && !task.ammo_target_ids.is_empty())
            .unwrap();

        assert_eq!(
            craft_task.stations,
            [StationKind::TechnicalCenter, StationKind::ArmorBench,]
        );
        assert_eq!(ammo_task.ammo_target_ids, ["ammo-b"]);
    }

    #[test]
    fn profit_gate_filters_by_rule_before_cutoff_and_exact_target_after_cutoff() {
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
        let qualified_ammo = qualified
            .accounts
            .iter()
            .find(|task| task.account_id == "a" && !task.ammo_target_ids.is_empty())
            .unwrap();
        assert_eq!(qualified_ammo.ammo_target_ids, ["ammo-a"]);
        assert!(qualified
            .accounts
            .iter()
            .any(|task| task.account_id == "a" && !task.stations.is_empty()));

        let cutoff = build_round_plan_with_profit(
            &settings,
            1_000,
            RoundTrigger::Scheduled,
            AmmoProfitGate::QualifiedTargets(std::collections::HashSet::from([(
                "a".to_string(),
                "ammo-b".to_string(),
            )])),
        )
        .unwrap();
        assert_eq!(
            cutoff
                .accounts
                .iter()
                .find(|task| task.account_id == "a" && !task.ammo_target_ids.is_empty())
                .unwrap()
                .ammo_target_ids,
            ["ammo-b"]
        );
    }
}
