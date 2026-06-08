import { useEffect, useMemo } from "react";
import { useDeltaAccounts } from "@/hooks/use-delta-accounts";
import type { AccountKind } from "@/components/app/delta-types";
import { ACCOUNT_KIND_LABELS } from "@/components/app/delta-types";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { InlineControl } from "@/components/app/app-ui";

type DeltaAccountSelectorProps = {
  filterKinds: AccountKind[];
  emptyText: string;
};

export function DeltaAccountSelector({ filterKinds, emptyText }: DeltaAccountSelectorProps) {
  const { accounts, selectedAccountId, selectAccount } = useDeltaAccounts();

  const filtered = useMemo(
    () => accounts.filter((a) => filterKinds.includes(a.kind)),
    [accounts, filterKinds],
  );

  const current = useMemo(
    () => filtered.find((a) => a.id === selectedAccountId),
    [filtered, selectedAccountId],
  );

  // 选中账号不在过滤范围时，自动切换到第一个匹配账号
  useEffect(() => {
    if (filtered.length > 0 && !current) {
      selectAccount(filtered[0].id);
    }
  }, [filtered, current, selectAccount]);

  if (filtered.length === 0) {
    return (
      <InlineControl className="px-4 py-3 text-sm text-muted-foreground">
        {emptyText}
      </InlineControl>
    );
  }

  const value = current ? String(current.id) : String(filtered[0].id);

  return (
    <div className="flex items-center gap-3">
      <span className="text-sm font-medium text-muted-foreground">当前账号</span>
      <Select
        value={value}
        onValueChange={(v) => selectAccount(Number(v))}
      >
        <SelectTrigger className="w-64">
          <SelectValue placeholder="选择账号" />
        </SelectTrigger>
        <SelectContent>
          {filtered.map((account) => (
            <SelectItem key={account.id} value={String(account.id)}>
              <span className="flex items-center gap-2">
                <span className="font-mono text-[0.62rem] text-muted-foreground">
                  {ACCOUNT_KIND_LABELS[account.kind]}
                </span>
                <span className="truncate">{account.uinOrOpenid}</span>
              </span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
