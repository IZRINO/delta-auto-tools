import {useEffect, useMemo, useRef, useState} from "react";
import {
    RiAddLine,
    RiDeleteBinLine,
    RiRefreshLine,
    RiSaveLine,
    RiSearchLine,
} from "@remixicon/react";

import {HelpHint, SoftAlert} from "@/components/app/app-ui";
import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {Switch} from "@/components/ui/switch";
import {
    buildProfitConfigurationDraft,
    deleteProfitRuleFromDraft,
    listProfitBindings,
    parseMinimumProfit,
    profitConfigurationFingerprint,
    ruleReferenceCounts,
    type ProfitConfigurationDraft,
} from "@/components/app/special-ops-profit-utils";
import type {
    AmmoProfitAudit,
    AmmoProfitRule,
    MoligodBindingValidation,
    ProfitCatalogSnapshot,
    ProfitConfigurationUpdate,
    SpecialOpsBootstrap,
} from "@/components/app/special-ops-types";
import {invokeLogged as invoke} from "@/lib/logging";

const runtimePhaseLabels = {
    disabled: "利润筛选未启用",
    waitingExchange: "等待每日兑换时间",
    querying: "正在查询利润",
    waitingNextQuery: "等待下轮利润查询",
    activeRound: "当前轮次正在执行",
    cutoffBypass: "截止后按最低利润门槛执行",
    cutoffQuerying: "截止利润查询中",
    waitingCutoffRetry: "等待截止利润补查",
    cutoffComplete: "截止利润查询完成",
    paused: "自动化已暂停",
} as const;

const auditOutcomeLabels = {
    qualified: "已达标",
    belowThreshold: "未达标",
    targetMissing: "目标缺失",
    sourceFailure: "查询失败",
    unconfigured: "未配置",
} as const;

function latestAudit(audits: AmmoProfitAudit[], ruleId: string): AmmoProfitAudit | null {
    return audits
        .filter((audit) => audit.ruleId === ruleId)
        .sort((left, right) => right.queriedAtMs - left.queriedAtMs)[0] ?? null;
}

function auditBadgeClass(audit: AmmoProfitAudit | null): string {
    if (!audit) return "badge-ghost";
    if (audit.outcome === "qualified") return "badge-success badge-soft";
    if (audit.outcome === "belowThreshold") return "badge-warning badge-soft";
    return "badge-error badge-soft";
}

function formatProfit(value: number | null): string {
    return value === null ? "-" : value.toLocaleString("zh-CN");
}

export function formatProfitCatalogError(cause: unknown): string {
    const message = String(cause);
    return message.includes("code -101") || message.includes("系统繁忙")
        ? "KKRB 暂时繁忙，名称列表未更新。可直接手工填写并保存“KKRB 精确名称”。"
        : message;
}

function newRule(): AmmoProfitRule {
    return {
        id: crypto.randomUUID(),
        displayName: "",
        kkrbMatchName: "",
        moligodMatchName: null,
        minimumProfit: 0,
    };
}

type Props = {
    bootstrap: SpecialOpsBootstrap;
    isNativeShell: boolean;
    onSave: (update: ProfitConfigurationUpdate) => Promise<SpecialOpsBootstrap>;
};

export function SpecialOpsProfitFilter({bootstrap, isNativeShell, onSave}: Props) {
    const fingerprint = profitConfigurationFingerprint(bootstrap.settings);
    const [draft, setDraft] = useState<ProfitConfigurationDraft>(() => buildProfitConfigurationDraft(bootstrap.settings));
    const [baseFingerprint, setBaseFingerprint] = useState(fingerprint);
    const [dirty, setDirty] = useState(false);
    const [conflict, setConflict] = useState(false);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [catalog, setCatalog] = useState<ProfitCatalogSnapshot | null>(null);
    const [catalogLoading, setCatalogLoading] = useState(false);
    const [validatingRuleId, setValidatingRuleId] = useState<string | null>(null);
    const [validation, setValidation] = useState<MoligodBindingValidation | null>(null);
    const [deleteRuleId, setDeleteRuleId] = useState<string | null>(null);
    const deleteDialogRef = useRef<HTMLDialogElement>(null);

    useEffect(() => {
        if (fingerprint === baseFingerprint) return;
        if (dirty) {
            setConflict(true);
            return;
        }
        setDraft(buildProfitConfigurationDraft(bootstrap.settings));
        setBaseFingerprint(fingerprint);
        setConflict(false);
    }, [baseFingerprint, bootstrap.settings, dirty, fingerprint]);

    const bindings = useMemo(() => listProfitBindings(bootstrap.settings), [bootstrap.settings]);
    const bindingLookup = useMemo(
        () => new Map(bindings.map((binding) => [`${binding.accountId ?? "default"}:${binding.targetId}`, binding])),
        [bindings],
    );
    const referenceCounts = useMemo(() => ruleReferenceCounts(draft), [draft]);
    const deleteRule = deleteRuleId ? draft.rules.find((rule) => rule.id === deleteRuleId) ?? null : null;
    const activeRound = bootstrap.profitRuntime.phase === "activeRound";

    const updateDraft = (next: ProfitConfigurationDraft) => {
        setDraft(next);
        setDirty(true);
        setError(null);
        setValidation(null);
    };
    const updateRule = (ruleId: string, patch: Partial<AmmoProfitRule>) => {
        updateDraft({...draft, rules: draft.rules.map((rule) => rule.id === ruleId ? {...rule, ...patch} : rule)});
    };
    const updateBinding = (accountId: string | null, targetId: string, profitRuleId: string | null) => {
        updateDraft({
            ...draft,
            bindings: draft.bindings.map((binding) => binding.accountId === accountId && binding.targetId === targetId
                ? {...binding, profitRuleId}
                : binding),
        });
    };
    const refreshCatalog = async () => {
        if (!isNativeShell) return;
        setCatalogLoading(true);
        setError(null);
        try {
            setCatalog(await invoke<ProfitCatalogSnapshot>("special_ops_fetch_profit_catalog"));
        } catch (cause) {
            setError(formatProfitCatalogError(cause));
        } finally {
            setCatalogLoading(false);
        }
    };
    const validateMoligod = async (rule: AmmoProfitRule) => {
        if (!isNativeShell || activeRound || !rule.moligodMatchName?.trim()) return;
        setValidatingRuleId(rule.id);
        setError(null);
        try {
            setValidation(await invoke<MoligodBindingValidation>("special_ops_validate_moligod_binding", {
                exactName: rule.moligodMatchName,
            }));
        } catch (cause) {
            setError(String(cause));
        } finally {
            setValidatingRuleId(null);
        }
    };
    const save = async () => {
        if (!isNativeShell || saving || conflict) return;
        setSaving(true);
        setError(null);
        try {
            const next = await onSave(draft);
            setDraft(buildProfitConfigurationDraft(next.settings));
            setBaseFingerprint(profitConfigurationFingerprint(next.settings));
            setDirty(false);
            setConflict(false);
        } catch (cause) {
            setError(String(cause));
        } finally {
            setSaving(false);
        }
    };
    const requestDelete = (ruleId: string) => {
        if ((referenceCounts.get(ruleId) ?? 0) === 0) {
            updateDraft(deleteProfitRuleFromDraft(draft, ruleId));
            return;
        }
        setDeleteRuleId(ruleId);
        deleteDialogRef.current?.showModal();
    };
    const confirmDelete = () => {
        if (deleteRuleId) updateDraft(deleteProfitRuleFromDraft(draft, deleteRuleId));
        setDeleteRuleId(null);
        deleteDialogRef.current?.close();
    };
    const reloadDraft = () => {
        setDraft(buildProfitConfigurationDraft(bootstrap.settings));
        setBaseFingerprint(fingerprint);
        setDirty(false);
        setConflict(false);
        setError(null);
    };

    return <section className="card card-border bg-base-100">
        <div className="card-body gap-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <h2 className="card-title text-lg inline-flex items-center gap-1">联网利润筛选<HelpHint content="KKRB 主源；仅 KKRB 整体失败时使用 Moligod 备用。"/></h2>
                </div>
                <label className="flex items-center gap-2 text-sm"><Switch checked={draft.enabled} onCheckedChange={(enabled) => updateDraft({...draft, enabled})}/>启用</label>
            </div>

            <div className="grid gap-3 md:grid-cols-[12rem_minmax(0,1fr)_auto_auto] md:items-end">
                <label className="form-control gap-1"><span className="label-text text-xs">利润截止时间</span><Input type="time" value={draft.cutoffTime} onChange={(event) => updateDraft({...draft, cutoffTime: event.target.value})}/></label>
                <div className="text-sm text-base-content/70">{runtimePhaseLabels[bootstrap.profitRuntime.phase]}{bootstrap.profitRuntime.nextQueryAtMs ? ` · 下次 ${new Date(bootstrap.profitRuntime.nextQueryAtMs).toLocaleTimeString("zh-CN", {hour: "2-digit", minute: "2-digit", hour12: false})}` : ""}</div>
                <Button disabled={!isNativeShell || catalogLoading} size="sm" variant="outline" onClick={() => void refreshCatalog()}><RiRefreshLine data-icon="inline-start"/>{catalogLoading ? "读取中" : "刷新 KKRB 名称"}</Button>
                <Button disabled={!isNativeShell || saving || conflict || !dirty} size="sm" onClick={() => void save()}><RiSaveLine data-icon="inline-start"/>{saving ? "保存中" : "保存利润配置"}</Button>
            </div>

            {(error || conflict || bootstrap.profitRuntime.configurationError || bootstrap.profitRuntime.lastSummary) && (
                <SoftAlert tone={error || conflict || bootstrap.profitRuntime.configurationError ? "warning" : "info"}>
                    <span>{error ?? (conflict ? "权威利润配置已更新，当前草稿未丢失；请重新载入后再编辑。" : bootstrap.profitRuntime.configurationError ?? bootstrap.profitRuntime.lastSummary)}</span>
                    {conflict && <Button size="xs" variant="outline" onClick={reloadDraft}>重新载入</Button>}
                </SoftAlert>
            )}
            {validation && <SoftAlert tone="success">Moligod 已验证“{validation.exactName}”，当前总利润 {formatProfit(validation.profit)}。</SoftAlert>}

            <details className="rounded-box border border-base-300">
                <summary className="cursor-pointer px-4 py-3 font-medium">利润规则</summary>
                <div className="space-y-3 border-t border-base-300 p-4">
                    <div className="overflow-x-auto">
                        <table className="table table-sm">
                            <thead><tr><th>规则</th><th>KKRB 精确名称</th><th>Moligod 精确名称</th><th>最低总利润</th><th>最近结果</th><th>操作</th></tr></thead>
                            <tbody>
                                {draft.rules.map((rule) => {
                                    const audit = latestAudit(bootstrap.settings.profitFilter.audits, rule.id);
                                    return <tr key={rule.id}>
                                        <td><Input className="min-w-28" value={rule.displayName} placeholder="显示名称" aria-label="规则显示名称" onChange={(event) => updateRule(rule.id, {displayName: event.target.value})}/><div className="mt-1 text-xs text-base-content/60">引用 {referenceCounts.get(rule.id) ?? 0}</div></td>
                                        <td><Input className="min-w-40" list="special-ops-kkrb-catalog" value={rule.kkrbMatchName} placeholder="精确名称" aria-label="KKRB 精确名称" onChange={(event) => updateRule(rule.id, {kkrbMatchName: event.target.value})}/></td>
                                        <td><div className="flex min-w-52 gap-1"><Input value={rule.moligodMatchName ?? ""} placeholder="可选备用名称" aria-label="Moligod 精确名称" onChange={(event) => updateRule(rule.id, {moligodMatchName: event.target.value || null})}/><Button disabled={!rule.moligodMatchName?.trim() || activeRound || validatingRuleId === rule.id || !isNativeShell} size="icon-sm" title="验证 Moligod 精确名称" aria-label="验证 Moligod 精确名称" variant="outline" onClick={() => void validateMoligod(rule)}><RiSearchLine/></Button></div></td>
                                        <td><Input className="min-w-28" inputMode="numeric" value={String(rule.minimumProfit)} aria-label="最低总利润" onChange={(event) => {
                                            const minimumProfit = parseMinimumProfit(event.target.value);
                                            if (minimumProfit !== null) updateRule(rule.id, {minimumProfit});
                                        }}/></td>
                                        <td>{audit ? <><span className={`badge badge-sm ${auditBadgeClass(audit)}`}>{auditOutcomeLabels[audit.outcome]}</span><div className="mt-1 text-xs text-base-content/60">{formatProfit(audit.profit)} · {audit.source ?? "-"}</div></> : <span className="text-xs text-base-content/60">尚无查询</span>}</td>
                                        <td><Button size="icon-sm" title="删除规则" aria-label="删除规则" variant="ghost" onClick={() => requestDelete(rule.id)}><RiDeleteBinLine/></Button></td>
                                    </tr>;
                                })}
                                {draft.rules.length === 0 && <tr><td colSpan={6} className="text-center text-sm text-base-content/60">尚未添加利润规则</td></tr>}
                            </tbody>
                        </table>
                    </div>
                    <datalist id="special-ops-kkrb-catalog">{catalog?.names.map((name) => <option key={name} value={name}/>)}</datalist>
                    {catalog && <p className="text-xs text-base-content/60">KKRB 名称 {catalog.names.length} 个{catalog.sourceVersion ? ` · 版本 ${catalog.sourceVersion}` : ""}</p>}
                    <div><Button size="sm" variant="outline" onClick={() => updateDraft({...draft, rules: [...draft.rules, newRule()]})}><RiAddLine data-icon="inline-start"/>添加利润规则</Button></div>
                </div>
            </details>

            <details className="rounded-box border border-base-300">
                <summary className="cursor-pointer px-4 py-3 font-medium">业务目标</summary>
                <div className="border-t border-base-300 p-4">
                    <div className="overflow-x-auto">
                        <table className="table table-sm">
                            <thead><tr><th>业务目标</th><th>状态</th><th>利润规则</th></tr></thead>
                            <tbody>{draft.bindings.map((binding) => {
                                const source = bindingLookup.get(`${binding.accountId ?? "default"}:${binding.targetId}`);
                                return <tr key={`${binding.accountId ?? "default"}:${binding.targetId}`}>
                                    <td>{source?.ownerLabel ?? "已删除配置"} · {source?.targetNote || binding.targetId}</td>
                                    <td><span className={`badge badge-sm ${source?.targetEnabled ? "badge-success badge-soft" : "badge-ghost"}`}>{source?.targetEnabled ? "启用" : "停用"}</span></td>
                                    <td><select className="select select-sm min-w-48" value={binding.profitRuleId ?? ""} onChange={(event) => updateBinding(binding.accountId, binding.targetId, event.target.value || null)}><option value="">不绑定利润规则</option>{draft.rules.map((rule) => <option key={rule.id} value={rule.id}>{rule.displayName || rule.kkrbMatchName || rule.id}</option>)}</select></td>
                                </tr>;
                            })}</tbody>
                        </table>
                    </div>
                </div>
            </details>
        </div>
        <dialog ref={deleteDialogRef} className="modal">
            <div className="modal-box">
                <h3 className="text-lg font-semibold">删除利润规则</h3>
                <p className="py-3 text-sm">“{deleteRule?.displayName || deleteRule?.kkrbMatchName}”被 {deleteRule ? referenceCounts.get(deleteRule.id) ?? 0 : 0} 个业务目标引用。确认后会清空这些目标的利润规则绑定，不改点击点、顺序或当天兑换状态。</p>
                <div className="modal-action"><Button variant="outline" onClick={() => { setDeleteRuleId(null); deleteDialogRef.current?.close(); }}>取消</Button><Button variant="destructive" onClick={confirmDelete}>删除</Button></div>
            </div>
            <form method="dialog" className="modal-backdrop"><button onClick={() => setDeleteRuleId(null)}>关闭</button></form>
        </dialog>
    </section>;
}
