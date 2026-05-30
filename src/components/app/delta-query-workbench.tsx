import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { QueryWorkbenchKind, ApiResponse } from "@/components/app/delta-types";
import { QUERY_WORKBENCH_LABELS, QUERY_WORKBENCH_KINDS } from "@/components/app/delta-types";
import { TacticalCard, SectionHeader, CardBody } from "@/components/app/app-ui";
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
import { RiSearchLine } from "@remixicon/react";

type DeltaQueryWorkbenchProps = {
  accountId: number;
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

export function DeltaQueryWorkbench({ accountId }: DeltaQueryWorkbenchProps) {
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
    <TacticalCard className="lg:col-span-2">
      <SectionHeader
        eyebrow="Query"
        icon={<RiSearchLine />}
        title="查询工作台"
        description="参数化查询游戏物品、物价、枪械等数据"
      />
      <CardBody className="space-y-4">
        <div className="flex items-end gap-3">
          <div className="flex-1">
            <label className="mb-1.5 block text-xs font-medium text-muted-foreground">查询类型</label>
            <Select value={kind} onValueChange={handleKindChange}>
              <SelectTrigger>
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
        </div>

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {kind === "items" && (
            <>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">类型 ID</label>
                <Input type="number" min="0" placeholder="0" value={typeId} onChange={(e) => setTypeId(e.target.value)} />
              </div>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">子类型</label>
                <Input type="number" min="0" placeholder="0" value={subType} onChange={(e) => setSubType(e.target.value)} />
              </div>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">物品 ID（可选）</label>
                <Input placeholder="留空查询全部" value={itemId} onChange={(e) => setItemId(e.target.value)} />
              </div>
            </>
          )}
          {kind === "price" && (
            <>
              <div className="sm:col-span-2">
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">物品 ID 列表（逗号分隔）</label>
                <Input placeholder="1,2,3" value={priceArgs} onChange={(e) => setPriceArgs(e.target.value)} />
              </div>
              <div className="flex items-center gap-2 pt-5">
                <Checkbox id="withRecent" checked={withRecent} onCheckedChange={(v) => setWithRecent(v === true)} />
                <label htmlFor="withRecent" className="text-xs text-muted-foreground">包含近期价格</label>
              </div>
            </>
          )}
          {kind === "guns" && (
            <div>
              <label className="mb-1.5 block text-xs font-medium text-muted-foreground">枪械 ID</label>
              <Input placeholder="输入枪械 ID" value={gunId} onChange={(e) => setGunId(e.target.value)} />
            </div>
          )}
          {kind === "logs" && (
            <>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">日志类型</label>
                <Input type="number" min="0" placeholder="0" value={logType} onChange={(e) => setLogType(e.target.value)} />
              </div>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">页码</label>
                <Input type="number" min="1" placeholder="1" value={logPage} onChange={(e) => setLogPage(e.target.value)} />
              </div>
            </>
          )}
          {kind === "firearm_mod_list" && (
            <>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">页码</label>
                <Input type="number" min="1" placeholder="1" value={fmPage} onChange={(e) => setFmPage(e.target.value)} />
              </div>
              <div>
                <label className="mb-1.5 block text-xs font-medium text-muted-foreground">每页数量</label>
                <Input type="number" min="1" placeholder="20" value={fmPageSize} onChange={(e) => setFmPageSize(e.target.value)} />
              </div>
            </>
          )}
          {kind === "recommendation" && (
            <div>
              <label className="mb-1.5 block text-xs font-medium text-muted-foreground">地图名称</label>
              <Input placeholder="输入地图名称" value={place} onChange={(e) => setPlace(e.target.value)} />
            </div>
          )}
        </div>

        <div className="flex items-center gap-3">
          <Button size="sm" disabled={loading || !canQuery} onClick={handleQuery}>
            {loading ? <Spinner className="mr-1.5 size-3.5" /> : <RiSearchLine data-icon="inline-start" className="mr-1.5 size-3.5" />}
            查询
          </Button>
          {needsAuth && !canQuery && (
            <span className="text-xs text-destructive">此查询需要登录账号</span>
          )}
        </div>

        {error && (
          <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        )}

        {result !== null && (
          <pre className="max-h-96 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
            {JSON.stringify(result, null, 2)}
          </pre>
        )}
      </CardBody>
    </TacticalCard>
  );
}
