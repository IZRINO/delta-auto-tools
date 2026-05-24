import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RiAccountPinCircleLine, RiAddLine, RiDeleteBinLine, RiRefreshLine } from "@remixicon/react";
import { useDeltaAccounts } from "@/hooks/use-delta-accounts";
import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { canRefreshToken } from "@/components/app/delta-utils";
import { AppPage, PageHero, SignalTile, TacticalCard, SectionHeader, CardBody } from "@/components/app/app-ui";
import { DeltaAccountCard } from "@/components/app/delta-account-card";
import { DeltaLoginDialog } from "@/components/app/delta-login-dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
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
    const valid = accounts.filter((a) => a.accessToken && a.expiresAt !== null && a.expiresAt * 1000 > now).length;
    const expiring = accounts.filter((a) => {
      if (!a.accessToken || a.expiresAt === null) return false;
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
    if (!canRefreshToken(account.kind) || !account.openid || !account.accessToken) return;
    setRefreshingId(account.id);
    try {
      const cmdMap: Record<string, string> = {
        qq: "delta_qq_update_access_token",
        wechat: "delta_wechat_update_access_token",
      };
      const cmd = cmdMap[account.kind];
      if (!cmd) return;
      await invoke(cmd, {
        openid: account.openid,
        accessToken: account.accessToken,
        cookie: account.kind === "qq" ? account.cookieJson || undefined : undefined,
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
          eyebrow="三角洲行动"
          title="账号管理"
          description="管理游戏账号登录状态与访问令牌"
        />
        <TacticalCard className="min-h-72">
          <CardBody className="flex h-full items-center justify-center">
            <p className="text-sm text-muted-foreground">需要桌面环境才能使用账号管理功能</p>
          </CardBody>
        </TacticalCard>
      </AppPage>
    );
  }

  return (
    <AppPage>
      <PageHero
        eyebrow="三角洲行动"
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
          eyebrow="Accounts"
          icon={<RiAccountPinCircleLine />}
          title="账号列表"
          description="点击选中账号，右键查看更多操作"
          badge={accounts.length > 0 ? <Badge variant="secondary">{accounts.length}</Badge> : undefined}
        />
        <CardBody>
          {accounts.length === 0 ? (
            <Empty className="border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_34%,transparent))]">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <RiAccountPinCircleLine />
                </EmptyMedia>
                <EmptyTitle>暂无账号</EmptyTitle>
                <EmptyDescription>点击上方"添加账号"按钮，扫描二维码登录游戏账号。</EmptyDescription>
              </EmptyHeader>
            </Empty>
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
                    {canRefreshToken(account.kind) && account.accessToken && account.openid && (
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
