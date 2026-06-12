import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RiAccountPinCircleLine, RiAddLine, RiDeleteBinLine, RiRefreshLine } from "@remixicon/react";
import { useDeltaAccounts } from "@/hooks/use-delta-accounts";
import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { canRefreshToken } from "@/components/app/delta-utils";
import { AppPage, PageHero, SignalTile, TacticalCard, SectionHeader, CardBody, TacticalEmptyState } from "@/components/app/app-ui";
import { DeltaAccountCard } from "@/components/app/delta-account-card";
import { DeltaLoginDialog } from "@/components/app/delta-login-dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";

export function DeltaAccountsPage() {
  const { accounts, selectedAccountId, selectAccount, refreshAccounts, isNativeShell } = useDeltaAccounts();
  const [loginOpen, setLoginOpen] = useState(false);
  const [refreshingId, setRefreshingId] = useState<number | null>(null);

  const stats = useMemo(() => {
    const now = Date.now();
    const threeDaysMs = 3 * 24 * 60 * 60 * 1000;
    const valid = accounts.filter((a) => a.hasAccessToken && a.expiresAt !== null && a.expiresAt * 1000 > now).length;
    const expiring = accounts.filter((a) => {
      if (!a.hasAccessToken || a.expiresAt === null) return false;
      const exp = a.expiresAt * 1000;
      return exp > now && exp <= now + threeDaysMs;
    }).length;
    return { total: accounts.length, valid, expiring };
  }, [accounts]);

  const handleLoginSuccess = useCallback(() => {
    refreshAccounts();
  }, [refreshAccounts]);

  const handleDelete = useCallback(async (id: number) => {
    try {
      await invoke("delta_delete_account", { accountId: id });
      if (selectedAccountId === id) selectAccount(null);
      await refreshAccounts();
    } catch {
      // 错误处理
    }
  }, [selectedAccountId, selectAccount, refreshAccounts]);

  const handleRefreshToken = useCallback(async (account: DeltaAccountRecord) => {
    if (!canRefreshToken(account.kind) || !account.hasAccessToken) return;
    setRefreshingId(account.id);
    try {
      const cmdMap: Partial<Record<typeof account.kind, string>> = {
        qq: "delta_qq_update_access_token",
        wechat: "delta_wechat_update_access_token",
      };
      const cmd = cmdMap[account.kind];
      if (!cmd) return;
      await invoke(cmd, {
        req: { accountId: account.id },
      });
      await refreshAccounts();
    } catch {
      // 刷新失败，账号管理页显示状态即可
    } finally {
      setRefreshingId(null);
    }
  }, [refreshAccounts]);

  if (!isNativeShell) {
    return (
      <AppPage>
        <PageHero
          eyebrow="D1 / CREDENTIALS"
          title="账号管理"
          description="管理游戏账号登录状态与访问令牌"
        />
        <TacticalEmptyState className="col-span-12" icon={<RiAccountPinCircleLine />} title="需要桌面环境" description="需要桌面环境才能使用账号管理功能。" />
      </AppPage>
    );
  }

  return (
    <AppPage>
      <PageHero
        eyebrow="D1 / CREDENTIALS"
        title="账号管理"
        description="管理游戏账号登录状态与访问令牌"
        actions={
          <Button size="sm" onClick={() => setLoginOpen(true)}>
            <RiAddLine data-icon="inline-start" />
            添加账号
          </Button>
        }
        stats={
          <>
            <SignalTile label="总账号" value={stats.total} icon={<RiAccountPinCircleLine />} />
            <SignalTile label="令牌有效" value={stats.valid} />
            <SignalTile label="即将过期" value={stats.expiring} />
          </>
        }
      />

      <div className="col-span-12 grid gap-3">
        <div className="grid gap-px border-2 border-[var(--ink)] bg-[var(--ink)] xl:grid-cols-[14rem_minmax(0,1fr)]">
          <div className="bg-[var(--ink)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.22em] text-[var(--paper)] uppercase">
            账号路由
          </div>
          <div className="grid gap-px bg-[var(--ink)] sm:grid-cols-3">
            <div className="bg-[var(--paper)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">
              <div>当前选中</div>
              <div className="mt-2 text-sm text-[var(--ink)]">{selectedAccountId ?? "未选择"}</div>
            </div>
            <div className="bg-[var(--bone)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">
              <div>有效令牌</div>
              <div className="mt-2 text-sm text-[var(--ink)]">{stats.valid} / {stats.total}</div>
            </div>
            <div className="bg-[var(--paper)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">
              <div>风险提示</div>
              <div className="mt-2 text-sm text-[var(--ink)]">{stats.expiring > 0 ? "存在即将过期" : "暂无"}</div>
            </div>
          </div>
        </div>

        <TacticalCard className="p-0">
          <div className="bg-[var(--ink)] px-3 py-1.5 font-mono text-[0.58rem] font-black tracking-[0.22em] text-[var(--paper)]/60 uppercase text-center">[ UNIT 01 ] — 身份凭据档案柜</div>
          <SectionHeader
            eyebrow="账号列表"
            icon={<RiAccountPinCircleLine />}
            title="账号档案矩阵"
            description="单击载入账号；右键执行刷新令牌或删除。"
            badge={accounts.length > 0 ? <Badge variant="secondary">{accounts.length}</Badge> : undefined}
          />
          <CardBody>
            {accounts.length === 0 ? (
              <div className="flex min-h-40 flex-col items-center justify-center gap-3 border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
                <RiAccountPinCircleLine className="size-5 text-[var(--alert-red)]" />
                <p className="text-sm font-black uppercase text-[var(--ink)]">暂无账号</p>
                <p className="max-w-xl font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.08em] text-[var(--steel)] uppercase">点击上方“添加账号”按钮，扫描二维码登录游戏账号。</p>
              </div>
            ) : (
              <div className="grid gap-3 xl:grid-cols-12">
                {accounts.map((account) => (
                  <div key={account.id} className="col-span-12 xl:col-span-4">
                    <ContextMenu>
                      <ContextMenuTrigger>
                        <DeltaAccountCard
                          account={account}
                          selected={selectedAccountId === account.id}
                          onSelect={selectAccount}
                        />
                      </ContextMenuTrigger>
                      <ContextMenuContent>
                        {canRefreshToken(account.kind) && account.hasAccessToken && (
                          <ContextMenuItem
                            onClick={() => handleRefreshToken(account)}
                            disabled={refreshingId === account.id}
                          >
                            <RiRefreshLine data-icon="inline-start" />
                            {refreshingId === account.id ? "刷新中..." : "刷新令牌"}
                          </ContextMenuItem>
                        )}
                        <ContextMenuItem
                          onClick={() => handleDelete(account.id)}
                          className="text-destructive focus:text-destructive"
                        >
                          <RiDeleteBinLine data-icon="inline-start" />
                          删除账号
                        </ContextMenuItem>
                      </ContextMenuContent>
                    </ContextMenu>
                  </div>
                ))}
              </div>
            )}
          </CardBody>
        </TacticalCard>
      </div>

      <DeltaLoginDialog
        open={loginOpen}
        onOpenChange={setLoginOpen}
        onLoginSuccess={handleLoginSuccess}
      />
    </AppPage>
  );
}
