import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RiBarChartBoxLine, RiSwordLine, RiTrophyLine, RiShieldLine, RiKey2Line, RiUserLine } from "@remixicon/react";
import { useDeltaAccounts } from "@/hooks/use-delta-accounts";
import { ACCOUNT_KIND_LABELS } from "@/components/app/delta-types";
import type { AccountKind, ApiResponse, DeltaAccountRecord } from "@/components/app/delta-types";
import { buildGameAuth, getCapabilities } from "@/components/app/delta-utils";
import { AppPage, PageHero, SignalTile, TacticalCard, CardBody } from "@/components/app/app-ui";
import { DeltaAccountSelector } from "@/components/app/delta-account-selector";
import { DeltaDataCard } from "@/components/app/delta-data-card";
import { DeltaQueryWorkbench } from "@/components/app/delta-query-workbench";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

const GAME_AUTH_KINDS: AccountKind[] = ["qq", "wechat"];

type DataState<T> = {
  data: T | null;
  loading: boolean;
  error: string | null;
};

function useGameData<T>(account: DeltaAccountRecord | null, cmd: string | null, deps: unknown[] = []) {
  const [state, setState] = useState<DataState<T>>({ data: null, loading: false, error: null });
  const versionRef = useRef(0);

  const load = useCallback(async () => {
    if (!account || !cmd) return;
    const auth = buildGameAuth(account);
    if (!auth) return;

    const ver = ++versionRef.current;
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const res = await invoke<ApiResponse<T>>(cmd, { auth });
      if (ver !== versionRef.current) return;
      if (res.code === 0) {
        setState({ data: res.data, loading: false, error: null });
      } else {
        setState({ data: null, loading: false, error: res.msg || "请求失败" });
      }
    } catch (e) {
      if (ver !== versionRef.current) return;
      setState({ data: null, loading: false, error: String(e) });
    }
  }, [account, cmd]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    versionRef.current++;
    if (account && cmd && buildGameAuth(account)) {
      load();
    } else {
      setState({ data: null, loading: false, error: null });
    }
  }, [account?.id, cmd, ...deps]); // eslint-disable-line react-hooks/exhaustive-deps

  return { ...state, reload: load };
}

function JsonBlock({ data }: { data: unknown }) {
  return (
    <pre className="max-h-64 overflow-auto rounded-lg border border-[var(--surface-border)] bg-[var(--surface-tile)] p-3 text-xs text-muted-foreground">
      {JSON.stringify(data, null, 2)}
    </pre>
  );
}

export function DeltaGamePage() {
  const { selectedAccount, isNativeShell } = useDeltaAccounts();
  const hasCapability = selectedAccount && getCapabilities(selectedAccount.kind).includes("game_data");

  const auth = useMemo(() => selectedAccount ? buildGameAuth(selectedAccount) : null, [selectedAccount]);

  const player = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_player");
  const record = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_record");
  const assets = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_assets");
  const recent = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_recent");
  const achievement = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_achievement");
  const password = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_password");
  const bind = useGameData<unknown>(hasCapability ? selectedAccount : null, "delta_game_get_bind");

  if (!isNativeShell) {
    return (
      <AppPage>
        <PageHero
          eyebrow="三角洲行动"
          title="游戏数据"
          description="查看游戏内角色数据与资产信息"
        />
        <TacticalCard className="min-h-72">
          <CardBody className="flex h-full items-center justify-center">
            <p className="text-sm text-muted-foreground">需要桌面环境才能使用游戏数据功能</p>
          </CardBody>
        </TacticalCard>
      </AppPage>
    );
  }

  return (
    <AppPage>
      <PageHero
        eyebrow="三角洲行动"
        title="游戏数据"
        description="查看游戏内角色数据与资产信息"
        stats={
          hasCapability ? (
            <>
              <SignalTile label="等级" value={player.loading ? "—" : (player.data ? "已加载" : "—")} icon={<RiUserLine />} />
              <SignalTile label="烽火地带" value={record.loading ? "—" : (record.data ? "已加载" : "—")} icon={<RiSwordLine />} />
              <SignalTile label="全面战场" value={record.loading ? "—" : (record.data ? "已加载" : "—")} icon={<RiTrophyLine />} />
            </>
          ) : undefined
        }
      />

      <DeltaAccountSelector
        filterKinds={GAME_AUTH_KINDS}
        emptyText="请先在账号管理中添加 QQ 或微信账号"
      />

      {!selectedAccount && (
        <TacticalCard className="min-h-48">
          <CardBody className="flex h-full items-center justify-center">
            <Empty className="border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))]">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <RiBarChartBoxLine />
                </EmptyMedia>
                <EmptyTitle>选择账号查看数据</EmptyTitle>
                <EmptyDescription>在上方选择一个 QQ 或微信账号，即可查看游戏数据。</EmptyDescription>
              </EmptyHeader>
            </Empty>
          </CardBody>
        </TacticalCard>
      )}

      {selectedAccount && !hasCapability && (
        <TacticalCard className="min-h-48">
          <CardBody className="flex h-full items-center justify-center">
            <div className="text-center text-sm text-muted-foreground">
              <p>当前账号为 {ACCOUNT_KIND_LABELS[selectedAccount.kind]} 类型</p>
              <p className="mt-1">无法查询游戏数据，请选择 QQ 或微信账号</p>
            </div>
          </CardBody>
        </TacticalCard>
      )}

      {hasCapability && (
        <div className="grid gap-5 lg:grid-cols-2">
          <DeltaDataCard
            eyebrow="Player"
            title="角色信息"
            icon={<RiUserLine />}
            loading={player.loading}
            error={player.error}
            onRetry={player.reload}
          >
            {player.data ? <JsonBlock data={player.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaDataCard
            eyebrow="Record"
            title="战绩记录"
            icon={<RiSwordLine />}
            loading={record.loading}
            error={record.error}
            onRetry={record.reload}
          >
            {record.data ? <JsonBlock data={record.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaDataCard
            eyebrow="Assets"
            title="资产概览"
            icon={<RiTrophyLine />}
            loading={assets.loading}
            error={assets.error}
            onRetry={assets.reload}
          >
            {assets.data ? <JsonBlock data={assets.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaDataCard
            eyebrow="Recent"
            title="近期对局"
            icon={<RiSwordLine />}
            loading={recent.loading}
            error={recent.error}
            onRetry={recent.reload}
          >
            {recent.data ? <JsonBlock data={recent.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaDataCard
            eyebrow="Achievement"
            title="成就进度"
            icon={<RiTrophyLine />}
            loading={achievement.loading}
            error={achievement.error}
            onRetry={achievement.reload}
          >
            {achievement.data ? <JsonBlock data={achievement.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaDataCard
            eyebrow="Password"
            title="地图密码"
            icon={<RiKey2Line />}
            loading={password.loading}
            error={password.error}
            onRetry={password.reload}
          >
            {password.data ? <JsonBlock data={password.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaDataCard
            eyebrow="Bind"
            title="角色绑定"
            icon={<RiShieldLine />}
            loading={bind.loading}
            error={bind.error}
            onRetry={bind.reload}
          >
            {bind.data ? <JsonBlock data={bind.data} /> : <p className="py-4 text-sm text-muted-foreground">暂无数据</p>}
          </DeltaDataCard>

          <DeltaQueryWorkbench auth={auth} />
        </div>
      )}
    </AppPage>
  );
}
