import type {
    AmmoBusinessTarget,
    ProfitConfigurationUpdate,
    ProfitTargetBinding,
    SpecialOpsSettings,
} from "@/components/app/special-ops-types";

export type ProfitBindingView = ProfitTargetBinding & {
    ownerLabel: string;
    targetNote: string;
    targetEnabled: boolean;
};

export type ProfitConfigurationDraft = ProfitConfigurationUpdate;

function targetsToBindings(
    targets: AmmoBusinessTarget[],
    accountId: string | null,
    ownerLabel: string,
): ProfitBindingView[] {
    return targets.map((target) => ({
        accountId,
        targetId: target.id,
        profitRuleId: target.profitRuleId,
        ownerLabel,
        targetNote: target.note,
        targetEnabled: target.enabled,
    }));
}

export function listProfitBindings(settings: SpecialOpsSettings): ProfitBindingView[] {
    const bindings = targetsToBindings(
        settings.defaultBusinessConfig.ammoTargets,
        null,
        "默认配置",
    );
    const independent = [...settings.accounts]
        .filter((account) => account.independentSettingsEnabled && account.independentBusinessConfig)
        .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));
    for (const account of independent) {
        bindings.push(...targetsToBindings(
            account.independentBusinessConfig!.ammoTargets,
            account.id,
            `账号 ${account.qqAccount || account.id}`,
        ));
    }
    return bindings;
}

export function buildProfitConfigurationDraft(settings: SpecialOpsSettings): ProfitConfigurationDraft {
    return {
        enabled: settings.profitFilter.enabled,
        cutoffTime: settings.profitFilter.cutoffTime,
        rules: structuredClone(settings.profitFilter.rules),
        bindings: listProfitBindings(settings).map(({ownerLabel, targetNote, targetEnabled, ...binding}) => binding),
    };
}

export function ruleReferenceCounts(draft: ProfitConfigurationDraft): Map<string, number> {
    const counts = new Map(draft.rules.map((rule) => [rule.id, 0]));
    for (const binding of draft.bindings) {
        if (binding.profitRuleId && counts.has(binding.profitRuleId)) {
            counts.set(binding.profitRuleId, (counts.get(binding.profitRuleId) ?? 0) + 1);
        }
    }
    return counts;
}

export function deleteProfitRuleFromDraft(
    draft: ProfitConfigurationDraft,
    ruleId: string,
): ProfitConfigurationDraft {
    return {
        ...draft,
        rules: draft.rules.filter((rule) => rule.id !== ruleId),
        bindings: draft.bindings.map((binding) => binding.profitRuleId === ruleId
            ? {...binding, profitRuleId: null}
            : binding),
    };
}

export function parseMinimumProfit(value: string): number | null {
    if (!/^\d+$/.test(value)) return null;
    const parsed = Number(value);
    return Number.isSafeInteger(parsed) ? parsed : null;
}

export function profitConfigurationFingerprint(settings: SpecialOpsSettings): string {
    const draft = buildProfitConfigurationDraft(settings);
    return JSON.stringify(draft);
}
