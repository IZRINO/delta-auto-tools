import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { ACCOUNT_KIND_LABELS, CAPABILITY_LABELS } from "@/components/app/delta-types";
import { getCapabilities, getAccountDisplayName, getTokenStatus, getTokenStatusLabel } from "@/components/app/delta-utils";
import { Badge } from "@/components/ui/badge";
import { TacticalCard } from "@/components/app/app-ui";
import { TokenBadge } from "@/components/app/delta-token-badge";

type DeltaAccountCardProps = {
  account: DeltaAccountRecord;
  selected: boolean;
  onSelect: (id: number) => void;
};

export function DeltaAccountCard({ account, selected, onSelect }: DeltaAccountCardProps) {
  const tokenStatus = getTokenStatus(account);
  const tokenLabel = getTokenStatusLabel(tokenStatus, account.expiresAt);
  const capabilities = getCapabilities(account.kind);

  return (
    <TacticalCard
      active={selected}
      className="cursor-pointer p-0 transition-colors"
      onClick={() => onSelect(account.id)}
    >
      <div className="grid gap-px bg-[var(--ink)]">
        <div className="grid gap-px bg-[var(--ink)] sm:grid-cols-[9rem_minmax(0,1fr)]">
          <div className="bg-[var(--ink)] px-3 py-3 font-mono text-[0.62rem] font-black tracking-[0.18em] text-[var(--paper)] uppercase">
            账号档案 {String(account.id).padStart(2, "0")}
          </div>
          <div className="bg-[var(--paper)] px-3 py-3">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <Badge variant="outline" className="shrink-0 font-mono text-[0.62rem]">
                {ACCOUNT_KIND_LABELS[account.kind]}
              </Badge>
              <span className="min-w-0 truncate text-sm font-black uppercase text-[var(--ink)]">
                {getAccountDisplayName(account)}
              </span>
            </div>
          </div>
        </div>

        <div className="bg-[var(--bone)] px-3 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <TokenBadge status={tokenStatus} label={tokenLabel} />
            {capabilities.map((cap) => (
              <Badge key={cap} variant="secondary" className="text-[0.58rem]">
                {CAPABILITY_LABELS[cap]}
              </Badge>
            ))}
          </div>
          <p className="mt-3 font-mono text-[0.62rem] font-black tracking-[0.12em] text-[var(--steel)] uppercase">
            {selected ? "当前路由已锁定此账号" : "点击载入此账号，右键查看更多命令"}
          </p>
        </div>
      </div>
    </TacticalCard>
  );
}
