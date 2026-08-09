use std::collections::{HashMap, HashSet};

use super::model::{AmmoProfitAudit, ProfitAuditOutcome, ProfitCutoffSkipReason};

pub(crate) const FINAL_MINIMUM_PROFIT: u64 = 10_000;
pub(crate) const FINAL_RETRY_DELAY_MS: i64 = 5 * 60_000;

pub(crate) struct CutoffClassification {
    pub qualified_rule_ids: HashSet<String>,
    pub retry_rule_ids: HashSet<String>,
    pub skipped: HashMap<String, ProfitCutoffSkipReason>,
}

pub(crate) fn classify_cutoff_audits(
    audits: &[AmmoProfitAudit],
    attempt: u8,
) -> CutoffClassification {
    let mut result = CutoffClassification {
        qualified_rule_ids: HashSet::new(),
        retry_rule_ids: HashSet::new(),
        skipped: HashMap::new(),
    };
    for audit in audits {
        let reason = if audit.outcome == ProfitAuditOutcome::Qualified
            && audit
                .profit
                .is_some_and(|profit| profit >= FINAL_MINIMUM_PROFIT as i64)
        {
            result.qualified_rule_ids.insert(audit.rule_id.clone());
            continue;
        } else if audit.outcome == ProfitAuditOutcome::BelowThreshold
            || audit
                .profit
                .is_some_and(|profit| profit < FINAL_MINIMUM_PROFIT as i64)
        {
            ProfitCutoffSkipReason::BelowThreshold
        } else if audit.outcome == ProfitAuditOutcome::Unconfigured {
            ProfitCutoffSkipReason::Unconfigured
        } else if attempt == 1 {
            result.retry_rule_ids.insert(audit.rule_id.clone());
            continue;
        } else {
            ProfitCutoffSkipReason::QueryUnavailable
        };
        result.skipped.insert(audit.rule_id.clone(), reason);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_ops::profit::model::{AmmoProfitAudit, ProfitAuditOutcome, ProfitSource};

    fn audit(rule_id: &str, profit: Option<i64>, outcome: ProfitAuditOutcome) -> AmmoProfitAudit {
        AmmoProfitAudit {
            rule_id: rule_id.to_string(),
            day: "2026-08-06".to_string(),
            queried_at_ms: 1,
            source: Some(ProfitSource::Kkrb),
            attempted_sources: vec![ProfitSource::Kkrb],
            source_data_at: None,
            source_version: None,
            profit,
            threshold: FINAL_MINIMUM_PROFIT,
            outcome,
            detail: String::new(),
            next_query_at_ms: None,
        }
    }

    #[test]
    fn first_cutoff_query_skips_low_profit_and_retries_transient_results() {
        let result = classify_cutoff_audits(
            &[
                audit("qualified", Some(10_000), ProfitAuditOutcome::Qualified),
                audit("low", Some(9_999), ProfitAuditOutcome::BelowThreshold),
                audit("missing", None, ProfitAuditOutcome::TargetMissing),
                audit("failed", None, ProfitAuditOutcome::SourceFailure),
            ],
            1,
        );

        assert_eq!(FINAL_MINIMUM_PROFIT, 10_000);
        assert!(result.qualified_rule_ids.contains("qualified"));
        assert_eq!(
            result.skipped.get("low"),
            Some(&ProfitCutoffSkipReason::BelowThreshold)
        );
        assert_eq!(
            result.retry_rule_ids,
            HashSet::from(["missing".to_string(), "failed".to_string()])
        );
    }

    #[test]
    fn second_cutoff_query_turns_transient_and_invalid_profit_into_skips() {
        let result = classify_cutoff_audits(
            &[
                audit("missing", None, ProfitAuditOutcome::TargetMissing),
                audit("failed", None, ProfitAuditOutcome::SourceFailure),
                audit("invalid", None, ProfitAuditOutcome::Qualified),
            ],
            2,
        );

        assert!(result.retry_rule_ids.is_empty());
        assert!(result.qualified_rule_ids.is_empty());
        assert!(result
            .skipped
            .values()
            .all(|reason| *reason == ProfitCutoffSkipReason::QueryUnavailable));
    }
}
