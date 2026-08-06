use super::kkrb::{KkrbAdapter, KkrbFailureKind, KkrbSnapshot, KkrbSourceError};
use super::model::{
    profit_qualifies, AmmoProfitAudit, AmmoProfitRule, ProfitAuditOutcome, ProfitSource,
};
use super::moligod::{MoligodAdapter, MoligodRequestTarget, MoligodRuleResult, MoligodRuleStatus};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[allow(async_fn_in_trait)]
pub(crate) trait KkrbProfitSource: Send + Sync {
    async fn fetch(&self) -> Result<KkrbSnapshot, KkrbSourceError>;
}

#[allow(async_fn_in_trait)]
pub(crate) trait MoligodProfitSource: Send + Sync {
    async fn fetch(
        &self,
        generation: u64,
        rules: &[AmmoProfitRule],
    ) -> Result<Vec<MoligodRuleResult>, String>;

    async fn fetch_with_cancel(
        &self,
        generation: u64,
        rules: &[AmmoProfitRule],
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<MoligodRuleResult>, String> {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("Moligod 查询已取消".to_string());
        }
        self.fetch(generation, rules).await
    }
}

impl KkrbProfitSource for KkrbAdapter {
    async fn fetch(&self) -> Result<KkrbSnapshot, KkrbSourceError> {
        KkrbAdapter::fetch(self).await
    }
}

impl MoligodProfitSource for MoligodAdapter {
    async fn fetch(
        &self,
        generation: u64,
        rules: &[AmmoProfitRule],
    ) -> Result<Vec<MoligodRuleResult>, String> {
        self.fetch_with_cancel(generation, rules, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn fetch_with_cancel(
        &self,
        generation: u64,
        rules: &[AmmoProfitRule],
        cancelled: Arc<AtomicBool>,
    ) -> Result<Vec<MoligodRuleResult>, String> {
        let targets = rules
            .iter()
            .map(|rule| {
                rule.moligod_match_name
                    .as_ref()
                    .map(|exact_name| MoligodRequestTarget {
                        rule_id: rule.id.clone(),
                        exact_name: exact_name.clone(),
                    })
                    .ok_or_else(|| format!("规则 {} 未配置 Moligod 精确名称", rule.id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        MoligodAdapter::fetch(self, generation, targets, cancelled)
            .await
            .map(|snapshot| snapshot.results)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfitQueryContext {
    pub(crate) generation: u64,
    pub(crate) day: String,
    pub(crate) queried_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfitQueryOutcome {
    pub(crate) audits: Vec<AmmoProfitAudit>,
    pub(crate) qualified_rule_ids: HashSet<String>,
    pub(crate) summary: String,
}

#[cfg(test)]
pub(crate) async fn query_profit_rules<K, M>(
    kkrb: &K,
    moligod: &M,
    rules: &[AmmoProfitRule],
    context: &ProfitQueryContext,
) -> Result<ProfitQueryOutcome, String>
where
    K: KkrbProfitSource + ?Sized,
    M: MoligodProfitSource + ?Sized,
{
    query_profit_rules_with_cancel(
        kkrb,
        moligod,
        rules,
        context,
        Arc::new(AtomicBool::new(false)),
    )
    .await
}

pub(crate) async fn query_profit_rules_with_cancel<K, M>(
    kkrb: &K,
    moligod: &M,
    rules: &[AmmoProfitRule],
    context: &ProfitQueryContext,
    cancelled: Arc<AtomicBool>,
) -> Result<ProfitQueryOutcome, String>
where
    K: KkrbProfitSource + ?Sized,
    M: MoligodProfitSource + ?Sized,
{
    let mut qualified_rule_ids = HashSet::new();
    let audits = match kkrb.fetch().await {
        Ok(snapshot) => rules
            .iter()
            .map(|rule| {
                let (profit, outcome, detail) = match snapshot.exact_profit(&rule.kkrb_match_name) {
                    Ok(Some(profit)) => {
                        profit_result(rule, profit, &mut qualified_rule_ids, "KKRB")
                    }
                    Ok(None) => (
                        None,
                        ProfitAuditOutcome::TargetMissing,
                        format!("KKRB 正常响应中未找到精确名称“{}”", rule.kkrb_match_name),
                    ),
                    Err(message) => (None, ProfitAuditOutcome::SourceFailure, message),
                };
                audit(
                    rule,
                    context,
                    Some(ProfitSource::Kkrb),
                    vec![ProfitSource::Kkrb],
                    snapshot.source_data_at.clone(),
                    snapshot.source_version.clone(),
                    profit,
                    outcome,
                    detail,
                )
            })
            .collect(),
        Err(error) => match error.kind {
            KkrbFailureKind::WholeSource => {
                build_fallback_audits(
                    moligod,
                    rules,
                    context,
                    &error.message,
                    &mut qualified_rule_ids,
                    Arc::clone(&cancelled),
                )
                .await
            }
        },
    };
    let summary = format!(
        "利润查询完成：{} 个规则，{} 个达标",
        rules.len(),
        qualified_rule_ids.len()
    );
    Ok(ProfitQueryOutcome {
        audits,
        qualified_rule_ids,
        summary,
    })
}

async fn build_fallback_audits<M: MoligodProfitSource + ?Sized>(
    moligod: &M,
    rules: &[AmmoProfitRule],
    context: &ProfitQueryContext,
    kkrb_error: &str,
    qualified_rule_ids: &mut HashSet<String>,
    cancelled: Arc<AtomicBool>,
) -> Vec<AmmoProfitAudit> {
    let bound_rules = rules
        .iter()
        .filter(|rule| rule.moligod_match_name.is_some())
        .cloned()
        .collect::<Vec<_>>();
    let moligod_result = if bound_rules.is_empty() {
        None
    } else {
        Some(
            moligod
                .fetch_with_cancel(context.generation, &bound_rules, cancelled)
                .await,
        )
    };

    rules
        .iter()
        .map(|rule| {
            let Some(expected_name) = rule.moligod_match_name.as_deref() else {
                return audit(
                    rule,
                    context,
                    None,
                    vec![ProfitSource::Kkrb],
                    None,
                    None,
                    None,
                    ProfitAuditOutcome::SourceFailure,
                    format!("KKRB 整体失败：{kkrb_error}；未配置 Moligod 备用名称"),
                );
            };
            match moligod_result
                .as_ref()
                .expect("存在绑定规则时必须查询备用源")
            {
                Err(message) => audit(
                    rule,
                    context,
                    None,
                    vec![ProfitSource::Kkrb, ProfitSource::Moligod],
                    None,
                    None,
                    None,
                    ProfitAuditOutcome::SourceFailure,
                    format!("KKRB 整体失败：{kkrb_error}；Moligod 失败：{message}"),
                ),
                Ok(results) => {
                    let matches = results
                        .iter()
                        .filter(|result| result.rule_id == rule.id)
                        .collect::<Vec<_>>();
                    if matches.len() != 1 {
                        return audit(
                            rule,
                            context,
                            Some(ProfitSource::Moligod),
                            vec![ProfitSource::Kkrb, ProfitSource::Moligod],
                            None,
                            None,
                            None,
                            ProfitAuditOutcome::SourceFailure,
                            format!("Moligod ruleId {} 返回数量异常：{}", rule.id, matches.len()),
                        );
                    }
                    moligod_audit(rule, expected_name, matches[0], context, qualified_rule_ids)
                }
            }
        })
        .collect()
}

fn moligod_audit(
    rule: &AmmoProfitRule,
    expected_name: &str,
    result: &MoligodRuleResult,
    context: &ProfitQueryContext,
    qualified_rule_ids: &mut HashSet<String>,
) -> AmmoProfitAudit {
    let (profit, outcome, detail) = if result.exact_name != expected_name {
        (
            None,
            ProfitAuditOutcome::SourceFailure,
            format!("Moligod 精确名称不匹配：{}", result.exact_name),
        )
    } else {
        match result.status {
            MoligodRuleStatus::Matched => match result.profit {
                Some(profit) => profit_result(rule, profit, qualified_rule_ids, "Moligod"),
                None => (
                    None,
                    ProfitAuditOutcome::SourceFailure,
                    "Moligod 命中结果缺少利润".to_string(),
                ),
            },
            MoligodRuleStatus::SourceFailure => (
                None,
                ProfitAuditOutcome::SourceFailure,
                result
                    .detail
                    .clone()
                    .unwrap_or_else(|| "Moligod 规则查询失败".to_string()),
            ),
        }
    };
    audit(
        rule,
        context,
        Some(ProfitSource::Moligod),
        vec![ProfitSource::Kkrb, ProfitSource::Moligod],
        None,
        None,
        profit,
        outcome,
        detail,
    )
}

fn profit_result(
    rule: &AmmoProfitRule,
    profit: i64,
    qualified_rule_ids: &mut HashSet<String>,
    source_label: &str,
) -> (Option<i64>, ProfitAuditOutcome, String) {
    if profit_qualifies(profit, rule.minimum_profit) {
        qualified_rule_ids.insert(rule.id.clone());
        (
            Some(profit),
            ProfitAuditOutcome::Qualified,
            format!(
                "{source_label} 总利润 {profit}，达到门槛 {}",
                rule.minimum_profit
            ),
        )
    } else {
        (
            Some(profit),
            ProfitAuditOutcome::BelowThreshold,
            format!(
                "{source_label} 总利润 {profit}，低于门槛 {}",
                rule.minimum_profit
            ),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn audit(
    rule: &AmmoProfitRule,
    context: &ProfitQueryContext,
    source: Option<ProfitSource>,
    attempted_sources: Vec<ProfitSource>,
    source_data_at: Option<String>,
    source_version: Option<String>,
    profit: Option<i64>,
    outcome: ProfitAuditOutcome,
    detail: String,
) -> AmmoProfitAudit {
    AmmoProfitAudit {
        rule_id: rule.id.clone(),
        day: context.day.clone(),
        queried_at_ms: context.queried_at_ms,
        source,
        attempted_sources,
        source_data_at,
        source_version,
        profit,
        threshold: rule.minimum_profit,
        outcome,
        detail,
        next_query_at_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::profit::kkrb::{parse_kkrb_response, KkrbSnapshot, KkrbSourceError};
    use crate::special_ops::profit::model::{AmmoProfitRule, ProfitAuditOutcome, ProfitSource};
    use crate::special_ops::profit::moligod::{MoligodRuleResult, MoligodRuleStatus};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct FakeKkrb {
        result: Result<KkrbSnapshot, KkrbSourceError>,
    }

    impl KkrbProfitSource for FakeKkrb {
        async fn fetch(&self) -> Result<KkrbSnapshot, KkrbSourceError> {
            self.result.clone()
        }
    }

    struct FakeMoligod {
        result: Result<Vec<MoligodRuleResult>, String>,
        calls: AtomicUsize,
        requested_rule_ids: Mutex<Vec<String>>,
    }

    struct CancelAwareMoligod {
        saw_cancelled: AtomicBool,
    }

    impl FakeMoligod {
        fn new(result: Result<Vec<MoligodRuleResult>, String>) -> Self {
            Self {
                result,
                calls: AtomicUsize::new(0),
                requested_rule_ids: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn requested_rule_ids(&self) -> Vec<String> {
            self.requested_rule_ids.lock().unwrap().clone()
        }
    }

    impl MoligodProfitSource for FakeMoligod {
        async fn fetch(
            &self,
            _generation: u64,
            rules: &[AmmoProfitRule],
        ) -> Result<Vec<MoligodRuleResult>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.requested_rule_ids.lock().unwrap() =
                rules.iter().map(|rule| rule.id.clone()).collect();
            self.result.clone()
        }
    }

    impl MoligodProfitSource for CancelAwareMoligod {
        async fn fetch(
            &self,
            _generation: u64,
            _rules: &[AmmoProfitRule],
        ) -> Result<Vec<MoligodRuleResult>, String> {
            Ok(Vec::new())
        }

        async fn fetch_with_cancel(
            &self,
            _generation: u64,
            _rules: &[AmmoProfitRule],
            cancelled: Arc<AtomicBool>,
        ) -> Result<Vec<MoligodRuleResult>, String> {
            self.saw_cancelled
                .store(cancelled.load(Ordering::SeqCst), Ordering::SeqCst);
            Err("已取消".to_string())
        }
    }

    fn rule(
        id: &str,
        kkrb_name: &str,
        moligod_name: Option<&str>,
        minimum_profit: u64,
    ) -> AmmoProfitRule {
        AmmoProfitRule {
            id: id.to_string(),
            display_name: format!("规则 {id}"),
            kkrb_match_name: kkrb_name.to_string(),
            moligod_match_name: moligod_name.map(str::to_string),
            minimum_profit,
        }
    }

    fn context() -> ProfitQueryContext {
        ProfitQueryContext {
            generation: 7,
            day: "2026-08-02".to_string(),
            queried_at_ms: 1_000,
        }
    }

    fn kkrb_snapshot(rows: serde_json::Value, version: Option<&str>) -> KkrbSnapshot {
        let mut root = json!({"code": 0, "data": {"cn": rows}});
        if let Some(version) = version {
            root["version"] = json!(version);
        }
        parse_kkrb_response(&serde_json::to_vec(&root).unwrap()).unwrap()
    }

    fn whole_kkrb_failure() -> KkrbSourceError {
        parse_kkrb_response(r#"{"code":-101,"msg":"系统繁忙，请稍后再试"}"#.as_bytes()).unwrap_err()
    }

    fn matched(rule_id: &str, exact_name: &str, profit: i64) -> MoligodRuleResult {
        MoligodRuleResult {
            rule_id: rule_id.to_string(),
            exact_name: exact_name.to_string(),
            profit: Some(profit),
            status: MoligodRuleStatus::Matched,
            detail: None,
        }
    }

    #[tokio::test]
    async fn kkrb_target_missing_never_calls_moligod() {
        let kkrb = FakeKkrb {
            result: Ok(kkrb_snapshot(
                json!([{"itemName": "其他目标", "profit": 1}]),
                Some("v1"),
            )),
        };
        let moligod = FakeMoligod::new(Ok(vec![matched("rule-a", "Moligod A", 100)]));

        let result = query_profit_rules(
            &kkrb,
            &moligod,
            &[rule("rule-a", "目标 A", Some("Moligod A"), 100)],
            &context(),
        )
        .await
        .unwrap();

        assert_eq!(moligod.calls(), 0);
        assert_eq!(result.audits[0].outcome, ProfitAuditOutcome::TargetMissing);
        assert!(result.qualified_rule_ids.is_empty());
    }

    #[tokio::test]
    async fn whole_kkrb_failure_uses_moligod_only_for_bound_rules() {
        let kkrb = FakeKkrb {
            result: Err(whole_kkrb_failure()),
        };
        let moligod = FakeMoligod::new(Ok(vec![matched("rule-a", "Moligod A", 100)]));
        let rules = [
            rule("rule-a", "KKRB A", Some("Moligod A"), 100),
            rule("rule-b", "KKRB B", None, 100),
        ];

        let result = query_profit_rules(&kkrb, &moligod, &rules, &context())
            .await
            .unwrap();

        assert_eq!(moligod.calls(), 1);
        assert_eq!(moligod.requested_rule_ids(), ["rule-a"]);
        assert!(result.qualified_rule_ids.contains("rule-a"));
        assert_eq!(result.audits[1].outcome, ProfitAuditOutcome::SourceFailure);
    }

    #[tokio::test]
    async fn kkrb_rule_error_does_not_trigger_fallback_and_equal_profit_qualifies() {
        let kkrb = FakeKkrb {
            result: Ok(kkrb_snapshot(
                json!([
                    {"itemName": "重复目标", "profit": 1},
                    {"itemName": "重复目标", "profit": 2},
                    {"itemName": "正常目标", "profit": 100}
                ]),
                Some("v2"),
            )),
        };
        let moligod = FakeMoligod::new(Err("不应调用".to_string()));
        let rules = [
            rule("rule-a", "重复目标", Some("Moligod A"), 1),
            rule("rule-b", "正常目标", Some("Moligod B"), 100),
        ];

        let result = query_profit_rules(&kkrb, &moligod, &rules, &context())
            .await
            .unwrap();

        assert_eq!(moligod.calls(), 0);
        assert_eq!(result.audits[0].outcome, ProfitAuditOutcome::SourceFailure);
        assert_eq!(result.audits[1].outcome, ProfitAuditOutcome::Qualified);
        assert!(result.qualified_rule_ids.contains("rule-b"));
    }

    #[tokio::test]
    async fn two_source_failures_still_return_normal_query_outcome() {
        let kkrb = FakeKkrb {
            result: Err(whole_kkrb_failure()),
        };
        let moligod = FakeMoligod::new(Err("Moligod 页面超时".to_string()));

        let result = query_profit_rules(
            &kkrb,
            &moligod,
            &[rule("rule-a", "KKRB A", Some("Moligod A"), 100)],
            &context(),
        )
        .await
        .unwrap();

        assert_eq!(result.audits[0].outcome, ProfitAuditOutcome::SourceFailure);
        assert_eq!(
            result.audits[0].attempted_sources,
            [ProfitSource::Kkrb, ProfitSource::Moligod]
        );
        assert!(result.qualified_rule_ids.is_empty());
    }

    #[tokio::test]
    async fn audit_keeps_source_version_threshold_and_below_detail() {
        let kkrb = FakeKkrb {
            result: Ok(kkrb_snapshot(
                json!([{"itemName": "目标 A", "profit": 99}]),
                Some("v42"),
            )),
        };
        let moligod = FakeMoligod::new(Err("不应调用".to_string()));
        let rules = [rule("rule-a", "目标 A", None, 100)];
        let original = rules.clone();

        let result = query_profit_rules(&kkrb, &moligod, &rules, &context())
            .await
            .unwrap();

        assert_eq!(rules, original);
        assert_eq!(result.audits[0].source, Some(ProfitSource::Kkrb));
        assert_eq!(result.audits[0].source_version.as_deref(), Some("v42"));
        assert_eq!(result.audits[0].threshold, 100);
        assert_eq!(result.audits[0].outcome, ProfitAuditOutcome::BelowThreshold);
        assert!(result.audits[0].detail.contains("99"));
        assert!(result.audits[0].next_query_at_ms.is_none());
    }

    #[tokio::test]
    async fn cancellation_token_reaches_moligod_fallback() {
        let kkrb = FakeKkrb {
            result: Err(whole_kkrb_failure()),
        };
        let moligod = CancelAwareMoligod {
            saw_cancelled: AtomicBool::new(false),
        };
        let cancelled = Arc::new(AtomicBool::new(true));
        let result = query_profit_rules_with_cancel(
            &kkrb,
            &moligod,
            &[rule("rule-a", "KKRB A", Some("Moligod A"), 100)],
            &context(),
            cancelled,
        )
        .await
        .unwrap();

        assert!(moligod.saw_cancelled.load(Ordering::SeqCst));
        assert_eq!(result.audits[0].outcome, ProfitAuditOutcome::SourceFailure);
    }
}
