import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RiGiftLine, RiShieldLine, RiRocketLine, RiArrowDownSLine } from "@remixicon/react";
import { useDeltaAccounts } from "@/hooks/use-delta-accounts";
import type { AccountKind, ApiResponse } from "@/components/app/delta-types";
import { ACCOUNT_KIND_LABELS } from "@/components/app/delta-types";
import { buildWegameTicket, extractQqSafeCode, getCapabilities } from "@/components/app/delta-utils";
import { AppPage, PageHero, TacticalCard, SectionHeader, CardBody } from "@/components/app/app-ui";
import { DeltaAccountSelector } from "@/components/app/delta-account-selector";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";

const WEGAME_KINDS: AccountKind[] = ["wegameQq", "wegameWechat"];
const QQSAFE_KINDS: AccountKind[] = ["qqSafe"];
const PIONEER_KINDS: AccountKind[] = ["pioneer"];
const ALL_TOOLBOX_KINDS: AccountKind[] = [...WEGAME_KINDS, ...QQSAFE_KINDS, ...PIONEER_KINDS];

export function DeltaToolboxPage() {
  const { selectedAccount, isNativeShell } = useDeltaAccounts();
  const capabilities = selectedAccount ? getCapabilities(selectedAccount.kind) : [];

  // Wegame 状态
  const [giftLoading, setGiftLoading] = useState(false);
  const [giftResult, setGiftResult] = useState<unknown>(null);
  const [giftError, setGiftError] = useState<string | null>(null);
  const [cardLoading, setCardLoading] = useState(false);
  const [cardResult, setCardResult] = useState<unknown>(null);
  const [cardError, setCardError] = useState<string | null>(null);

  // QQ 安全中心状态
  const [bannedLoading, setBannedLoading] = useState(false);
  const [bannedResult, setBannedResult] = useState<unknown>(null);
  const [bannedError, setBannedError] = useState<string | null>(null);
  const [reportExpanded, setReportExpanded] = useState(false);
  const [reportUserId, setReportUserId] = useState("");
  const [reportLoading, setReportLoading] = useState(false);
  const [reportResult, setReportResult] = useState<unknown>(null);
  const [reportError, setReportError] = useState<string | null>(null);

  // Pioneer 状态
  const [pioneerLoading, setPioneerLoading] = useState(false);
  const [pioneerResult, setPioneerResult] = useState<unknown>(null);
  const [pioneerError, setPioneerError] = useState<string | null>(null);
  const [pioneerListType, setPioneerListType] = useState<"pc" | "mobile">("pc");

  // 切换账号时重置操作结果
  useEffect(() => {
    setGiftResult(null); setGiftError(null);
    setCardResult(null); setCardError(null);
    setBannedResult(null); setBannedError(null);
    setReportResult(null); setReportError(null); setReportUserId("");
    setPioneerResult(null); setPioneerError(null);
  }, [selectedAccount?.id]);

  const hasWegame = capabilities.includes("wegame");
  const hasQqSafe = capabilities.includes("qqsafe");
  const hasPioneer = capabilities.includes("pioneer");

  // Wegame 操作
  const handleOpenGift = useCallback(async () => {
    if (!selectedAccount) return;
    const ticket = buildWegameTicket(selectedAccount);
    if (!ticket) return;
    setGiftLoading(true);
    setGiftError(null);
    setGiftResult(null);
    try {
      const res = await invoke<ApiResponse<unknown>>("delta_wegame_open_treasure_gift", { request: { ticket } });
      if (res.code === 0) setGiftResult(res.data);
      else setGiftError(res.msg || "领取失败");
    } catch (e) {
      setGiftError(String(e));
    } finally {
      setGiftLoading(false);
    }
  }, [selectedAccount]);

  const handleDrawCard = useCallback(async () => {
    if (!selectedAccount) return;
    const ticket = buildWegameTicket(selectedAccount);
    if (!ticket) return;
    setCardLoading(true);
    setCardError(null);
    setCardResult(null);
    try {
      const res = await invoke<ApiResponse<unknown>>("delta_wegame_draw_daily_card", { request: { ticket } });
      if (res.code === 0) setCardResult(res.data);
      else setCardError(res.msg || "抽卡失败");
    } catch (e) {
      setCardError(String(e));
    } finally {
      setCardLoading(false);
    }
  }, [selectedAccount]);

  // QQ 安全中心封禁查询
  const handleLoadBanned = useCallback(async () => {
    if (!selectedAccount || !selectedAccount.openid || !selectedAccount.accessToken) return;
    const code = extractQqSafeCode(selectedAccount.extraJson);
    if (!code) {
      setBannedError("令牌信息不完整，请重新登录");
      return;
    }
    setBannedLoading(true);
    setBannedError(null);
    try {
      const res = await invoke<ApiResponse<unknown>>("delta_qqsafe_get_banned_list", {
        req: {
          openid: selectedAccount.openid,
          accessToken: selectedAccount.accessToken,
          code,
        },
      });
      if (res.code === 0) setBannedResult(res.data);
      else setBannedError(res.msg || "查询失败");
    } catch (e) {
      setBannedError(String(e));
    } finally {
      setBannedLoading(false);
    }
  }, [selectedAccount]);

  // QQ 安全中心举报
  const handleReport = useCallback(async () => {
    if (!selectedAccount || !selectedAccount.openid || !selectedAccount.accessToken || !reportUserId) return;
    setReportLoading(true);
    setReportError(null);
    try {
      const res = await invoke<ApiResponse<unknown>>("delta_qqsafe_report", {
        req: {
          openid: selectedAccount.openid,
          accessToken: selectedAccount.accessToken,
          userId: reportUserId,
        },
      });
      if (res.code === 0) setReportResult(res.data);
      else setReportError(res.msg || "查询失败");
    } catch (e) {
      setReportError(String(e));
    } finally {
      setReportLoading(false);
    }
  }, [selectedAccount, reportUserId]);

  // Pioneer 测试列表
  const handleLoadPioneer = useCallback(async () => {
    if (!selectedAccount || !selectedAccount.accessToken) return;
    setPioneerLoading(true);
    setPioneerError(null);
    try {
      const res = await invoke<ApiResponse<unknown>>("delta_pioneer_get_game_test_list", {
        req: {
          key: selectedAccount.accessToken,
          listType: pioneerListType,
        },
      });
      if (res.code === 0) setPioneerResult(res.data);
      else setPioneerError(res.msg || "查询失败");
    } catch (e) {
      setPioneerError(String(e));
    } finally {
      setPioneerLoading(false);
    }
  }, [selectedAccount, pioneerListType]);

  if (!isNativeShell) {
    return (
      <AppPage>
        <PageHero
          eyebrow="三角洲行动"
          title="工具箱"
          description="Wegame 运营、安全查询与先遣服测试"
        />
        <TacticalCard className="min-h-72">
          <CardBody className="flex h-full items-center justify-center">
            <p className="text-sm text-muted-foreground">需要桌面环境才能使用工具箱功能</p>
          </CardBody>
        </TacticalCard>
      </AppPage>
    );
  }

  return (
    <AppPage>
      <PageHero
        eyebrow="三角洲行动"
        title="工具箱"
        description="Wegame 运营、安全查询与先遣服测试"
      />

      <DeltaAccountSelector
        filterKinds={ALL_TOOLBOX_KINDS}
        emptyText="请先在账号管理中添加 Wegame、QQ安全中心或先遣服账号"
      />

      {!selectedAccount && (
        <TacticalCard className="min-h-48">
          <CardBody className="flex h-full items-center justify-center text-sm text-muted-foreground">
            选择一个账号以查看可用工具
          </CardBody>
        </TacticalCard>
      )}

      {selectedAccount && !hasWegame && !hasQqSafe && !hasPioneer && (
        <TacticalCard className="min-h-48">
          <CardBody className="flex h-full items-center justify-center text-sm text-muted-foreground">
            当前账号为 {ACCOUNT_KIND_LABELS[selectedAccount.kind]} 类型，无法使用工具箱功能。请选择 Wegame、QQ安全中心或先遣服账号。
          </CardBody>
        </TacticalCard>
      )}

      {hasWegame && (
        <TacticalCard>
          <SectionHeader
            eyebrow="Wegame"
            icon={<RiGiftLine />}
            title="Wegame 运营"
            description="领取保险箱礼包与每日抽卡"
          />
          <CardBody>
            <div className="flex flex-wrap gap-3">
              <Button size="sm" disabled={giftLoading} onClick={handleOpenGift}>
                {giftLoading && <Spinner className="mr-1.5 size-3.5" />}
                领取保险箱礼包
              </Button>
              <Button size="sm" disabled={cardLoading} onClick={handleDrawCard}>
                {cardLoading && <Spinner className="mr-1.5 size-3.5" />}
                每日抽卡
              </Button>
            </div>

            {giftError && <div className="mt-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{giftError}</div>}
            {giftResult !== null && (
              <pre className="mt-3 max-h-48 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
                {JSON.stringify(giftResult, null, 2)}
              </pre>
            )}

            {cardError && <div className="mt-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{cardError}</div>}
            {cardResult !== null && (
              <pre className="mt-3 max-h-48 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
                {JSON.stringify(cardResult, null, 2)}
              </pre>
            )}
          </CardBody>
        </TacticalCard>
      )}

      {hasQqSafe && (
        <TacticalCard>
          <SectionHeader
            eyebrow="QQ安全中心"
            icon={<RiShieldLine />}
            title="QQ安全中心查询"
            description="查询封禁记录与游戏报告"
          />
          <CardBody className="space-y-4">
            {/* 封禁记录 */}
            <div>
              <div className="flex items-center gap-3">
                <Button size="sm" disabled={bannedLoading} onClick={handleLoadBanned}>
                  {bannedLoading && <Spinner className="mr-1.5 size-3.5" />}
                  查询封禁记录
                </Button>
              </div>
              {bannedError && <div className="mt-2 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{bannedError}</div>}
              {bannedResult !== null && (
                <pre className="mt-2 max-h-48 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
                  {JSON.stringify(bannedResult, null, 2)}
                </pre>
              )}
            </div>

            {/* 举报折叠区 */}
            <div className="rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))]">
              <button
                type="button"
                className="flex w-full items-center justify-between px-3 py-2.5 text-sm font-medium text-foreground"
                onClick={() => setReportExpanded(!reportExpanded)}
              >
                <span className="flex items-center gap-2">
                  游戏报告
                  <Badge variant="outline" className="text-[0.58rem]">已弃用</Badge>
                </span>
                <RiArrowDownSLine className={`size-4 transition-transform ${reportExpanded ? "rotate-180" : ""}`} />
              </button>
              {reportExpanded && (
                <div className="border-t border-[var(--surface-border)] px-3 py-3 space-y-3">
                  <div>
                    <label className="mb-1.5 block text-xs font-medium text-muted-foreground">用户 QQ 号</label>
                    <Input
                      placeholder="输入 QQ 号"
                      value={reportUserId}
                      onChange={(e) => setReportUserId(e.target.value.replace(/\D/g, ""))}
                    />
                  </div>
                  <Button size="sm" disabled={reportLoading || !reportUserId} onClick={handleReport}>
                    {reportLoading && <Spinner className="mr-1.5 size-3.5" />}
                    查询
                  </Button>
                  {reportError && <div className="mt-2 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{reportError}</div>}
                  {reportResult !== null && (
                    <pre className="mt-2 max-h-48 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
                      {JSON.stringify(reportResult, null, 2)}
                    </pre>
                  )}
                </div>
              )}
            </div>
          </CardBody>
        </TacticalCard>
      )}

      {hasPioneer && (
        <TacticalCard>
          <SectionHeader
            eyebrow="Pioneer"
            icon={<RiRocketLine />}
            title="先遣服测试"
            description="查看先遣服测试游戏列表"
          />
          <CardBody className="space-y-3">
            <div className="flex items-center gap-3">
              <div className="flex rounded-lg border border-[var(--surface-border)] overflow-hidden">
                <button
                  type="button"
                  className={`px-3 py-1.5 text-xs font-medium transition-colors ${pioneerListType === "pc" ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-background/42"}`}
                  onClick={() => setPioneerListType("pc")}
                >
                  PC
                </button>
                <button
                  type="button"
                  className={`px-3 py-1.5 text-xs font-medium transition-colors ${pioneerListType === "mobile" ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-background/42"}`}
                  onClick={() => setPioneerListType("mobile")}
                >
                  手机
                </button>
              </div>
              <Button size="sm" disabled={pioneerLoading} onClick={handleLoadPioneer}>
                {pioneerLoading && <Spinner className="mr-1.5 size-3.5" />}
                查询测试列表
              </Button>
            </div>
            {pioneerError && <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">{pioneerError}</div>}
            {pioneerResult !== null && (
              <pre className="max-h-64 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
                {JSON.stringify(pioneerResult, null, 2)}
              </pre>
            )}
          </CardBody>
        </TacticalCard>
      )}
    </AppPage>
  );
}
