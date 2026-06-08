import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RiAccountPinCircleLine, RiAddLine, RiDeleteBinLine, RiRefreshLine } from "@remixicon/react";
import { useDeltaAccounts } from "@/hooks/use-delta-accounts";
import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { canRefreshToken } from "@/components/app/delta-utils";
import { AppPage, PageHero, SignalTile, TacticalCard, SectionHeader, CardBody, InlineControl, TacticalEmptyState } from "@/components/app/app-ui";
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
          eyebrow="账号与凭据"
          title="账号管理"
          description="管理游戏账号登录状态与访问令牌"
        />
        <TacticalEmptyState icon={<RiAccountPinCircleLine />} title="需要桌面环境" description="需要桌面环境才能使用账号管理功能。" />
      </AppPage>
    );
  }

  return (
    <AppPage>
      <PageHero
        eyebrow="账号与凭据"
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

      <TacticalCard>
        <SectionHeader
          eyebrow="账号列表"
          icon={<RiAccountPinCircleLine />}
          title="账号列表"
          description="点击选中账号，右键查看更多操作"
          badge={accounts.length > 0 ? <Badge variant="secondary">{accounts.length}</Badge> : undefined}
        />
        <CardBody>
          {accounts.length === 0 ? (
            <InlineControl className="border-dashed px-4 py-8 text-center">
              <RiAccountPinCircleLine className="mx-auto mb-2 text-muted-foreground" />
              <p className="text-sm font-medium text-muted-foreground">暂无账号</p>
              <p className="mt-1 text-xs text-muted-foreground">点击上方“添加账号”按钮，扫描二维码登录游戏账号。</p>
            </InlineControl>
          ) : (
            <div className="grid gap-2 sm:grid-cols-2">
              {accounts.map((account) => (
                <ContextMenu key={account.id}>
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
              ))}
            </div>
          )}
        </CardBody>
      </TacticalCard>

      <DeltaLoginDialog
        open={loginOpen}
        onOpenChange={setLoginOpen}
        onLoginSuccess={handleLoginSuccess}
      />
    </AppPage>
  );
}
