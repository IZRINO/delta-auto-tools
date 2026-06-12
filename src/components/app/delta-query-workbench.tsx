import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { QueryWorkbenchKind, ApiResponse } from "@/components/app/delta-types";
import { QUERY_WORKBENCH_LABELS, QUERY_WORKBENCH_KINDS } from "@/components/app/delta-types";
import { CardBody, InlineNotice, JsonPreBlock, SectionHeader, TacticalCard } from "@/components/app/app-ui";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import { RiSearchLine } from "@remixicon/react";

type DeltaQueryWorkbenchProps = {
  accountId: number;
  className?: string;
};

// 各查询类型必需参数校验
function validateQuery(kind: QueryWorkbenchKind, params: Record<string, string>): string | null {
  switch (kind) {
    case "price":
      if (!params.priceArgs.trim()) return "请输入至少一个物品 ID";
      break;
    case "guns":
      if (!params.gunId.trim()) return "请输入枪械 ID";
      break;
    case "recommendation":
      if (!params.place.trim()) return "请输入地图名称";
      break;
  }
  return null;
}

export function DeltaQueryWorkbench({ accountId, className }: DeltaQueryWorkbenchProps) {
  const [kind, setKind] = useState<QueryWorkbenchKind>("items");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);

  const [typeId, setTypeId] = useState("");
  const [subType, setSubType] = useState("");
  const [itemId, setItemId] = useState("");
  const [priceArgs, setPriceArgs] = useState("");
  const [withRecent, setWithRecent] = useState(false);
  const [gunId, setGunId] = useState("");
  const [logType, setLogType] = useState("");
  const [logPage, setLogPage] = useState("1");
  const [fmPage, setFmPage] = useState("1");
  const [fmPageSize, setFmPageSize] = useState("20");
  const [place, setPlace] = useState("");

  const resetResult = useCallback(() => {
    setResult(null);
    setError(null);
  }, []);

  const handleKindChange = useCallback((v: string) => {
    setKind(v as QueryWorkbenchKind);
    resetResult();
  }, [resetResult]);

  const handleQuery = useCallback(async () => {
    const validationError = validateQuery(kind, { priceArgs, gunId, place });
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);
    setError(null);
    setResult(null);

    try {
      let res: ApiResponse<unknown>;

      switch (kind) {
        case "items": {
          const p: Record<string, unknown> = { typeId: Number(typeId) || 0, subType: Number(subType) || 0 };
          if (itemId) p.itemId = itemId;
          res = await invoke("delta_game_get_items", { request: p });
          break;
        }
        case "price": {
          const parsed = priceArgs.split(",").map((s) => Number(s.trim())).filter((n) => !isNaN(n) && n > 0);
          res = await invoke("delta_game_get_price", { request: { args: parsed, withRecent } });
          break;
        }
        case "guns":
          res = await invoke("delta_game_get_guns", { request: { gunId } });
          break;
        case "logs":
          res = await invoke("delta_game_get_logs", { request: { accountId, logType: Number(logType) || 0, page: Math.max(1, Number(logPage) || 1) } });
          break;
        case "firearm_mod_list":
          res = await invoke("delta_game_get_firearm_mod_list", { request: { page: Math.max(1, Number(fmPage) || 1), pageSize: Math.max(1, Number(fmPageSize) || 20) } });
          break;
        case "recommendation":
          res = await invoke("delta_game_get_recommendation", { request: { place } });
          break;
        default:
          return;
      }

      if (res.code === 0) {
        setResult(res.data);
      } else {
        setError(res.msg || "查询失败");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [kind, typeId, subType, itemId, priceArgs, withRecent, gunId, logType, logPage, fmPage, fmPageSize, place, accountId]);

  const needsAuth = kind === "logs";
  const canQuery = !needsAuth || accountId > 0;

  return (
    <TacticalCard className={cn("col-span-12 p-0", className)}>
      <SectionHeader
        eyebrow="参数查询"
        icon={<RiSearchLine />}
        title="查询工作台"
        description="参数化查询游戏物品、物价、枪械与日志数据"
      />
      <CardBody className="space-y-3">
        <div className="grid gap-px border-2 border-[var(--chalk)] bg-[var(--chalk)] xl:grid-cols-[minmax(0,18rem)_minmax(0,1fr)]">
          <div className="bg-[var(--carbon)] p-3">
            <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">查询类型</label>
            <Select value={kind} onValueChange={handleKindChange}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {QUERY_WORKBENCH_KINDS.map((k) => (
                  <SelectItem key={k} value={k}>
                    {QUERY_WORKBENCH_LABELS[k]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-px bg-[var(--chalk)] sm:grid-cols-3">
            <div className="bg-[var(--slate)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
              <div className="text-[var(--chalk)]">授权状态</div>
              <div className="mt-2 text-sm text-[var(--chalk)]">{needsAuth ? (canQuery ? "已接入账号" : "需要登录") : "公共查询"}</div>
            </div>
            <div className="bg-[var(--carbon)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
              <div>账号编号</div>
              <div className="mt-2 text-sm text-[var(--chalk)]">{accountId > 0 ? accountId : "未接入"}</div>
            </div>
            <div className="bg-[var(--carbon)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
              <div>执行状态</div>
              <div className="mt-2 text-sm text-[var(--chalk)]">{loading ? "查询中" : error ? "执行失败" : result !== null ? "已返回" : "待命"}</div>
            </div>
          </div>
        </div>

        <div className="grid gap-3 xl:grid-cols-12">
          {kind === "items" && (
            <>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">类型 ID</label>
                <Input type="number" min="0" placeholder="0" value={typeId} onChange={(e) => setTypeId(e.target.value)} />
              </div>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">子类型</label>
                <Input type="number" min="0" placeholder="0" value={subType} onChange={(e) => setSubType(e.target.value)} />
              </div>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">物品 ID（可选）</label>
                <Input placeholder="留空查询全部" value={itemId} onChange={(e) => setItemId(e.target.value)} />
              </div>
            </>
          )}

          {kind === "price" && (
            <>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-8">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">物品 ID 列表（逗号分隔）</label>
                <Input placeholder="1,2,3" value={priceArgs} onChange={(e) => setPriceArgs(e.target.value)} />
              </div>
              <div className="flex items-center gap-2 border-2 border-[var(--chalk)] bg-[var(--slate)] px-3 py-3 xl:col-span-4">
                <Checkbox id="withRecent" checked={withRecent} onCheckedChange={(v) => setWithRecent(v === true)} />
                <label htmlFor="withRecent" className="font-mono text-[0.68rem] font-bold tracking-[0.08em] text-[var(--chalk)] uppercase">包含近期价格</label>
              </div>
            </>
          )}

          {kind === "guns" && (
            <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
              <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">枪械 ID</label>
              <Input placeholder="输入枪械 ID" value={gunId} onChange={(e) => setGunId(e.target.value)} />
            </div>
          )}

          {kind === "logs" && (
            <>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">日志类型</label>
                <Input type="number" min="0" placeholder="0" value={logType} onChange={(e) => setLogType(e.target.value)} />
              </div>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">页码</label>
                <Input type="number" min="1" placeholder="1" value={logPage} onChange={(e) => setLogPage(e.target.value)} />
              </div>
            </>
          )}

          {kind === "firearm_mod_list" && (
            <>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">页码</label>
                <Input type="number" min="1" placeholder="1" value={fmPage} onChange={(e) => setFmPage(e.target.value)} />
              </div>
              <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
                <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">每页数量</label>
                <Input type="number" min="1" placeholder="20" value={fmPageSize} onChange={(e) => setFmPageSize(e.target.value)} />
              </div>
            </>
          )}

          {kind === "recommendation" && (
            <div className="border-2 border-[var(--chalk)] bg-[var(--carbon)] p-3 xl:col-span-4">
              <label className="mb-2 block font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">地图名称</label>
              <Input placeholder="输入地图名称" value={place} onChange={(e) => setPlace(e.target.value)} />
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-3 border-2 border-[var(--chalk)] bg-[var(--slate)] px-3 py-3">
          <Button size="sm" disabled={loading || !canQuery} onClick={handleQuery}>
            {loading ? <Spinner className="mr-1.5 size-3.5" /> : <RiSearchLine data-icon="inline-start" className="mr-1.5 size-3.5" />}
            执行查询
          </Button>
          {needsAuth && !canQuery ? (
            <span className="font-mono text-[0.66rem] font-black tracking-[0.12em] text-[var(--amber)] uppercase">此查询需要登录账号</span>
          ) : (
            <span className="font-mono text-[0.66rem] font-bold tracking-[0.08em] text-[var(--zinc)] uppercase">结果将写入下方数据井</span>
          )}
        </div>

        {error && <InlineNotice title="查询失败">{error}</InlineNotice>}

        {result !== null && <JsonPreBlock className="min-h-56" maxHeightClassName="max-h-[30rem]" data={result} />}
      </CardBody>
    </TacticalCard>
  );
}
