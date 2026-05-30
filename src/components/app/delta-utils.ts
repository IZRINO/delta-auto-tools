import type { AccountKind, Capability, TokenStatus, DeltaAccountRecord } from "@/components/app/delta-types";
import { EXPIRING_THRESHOLD_MS, ACCOUNT_KIND_CAPABILITIES } from "@/components/app/delta-types";

export function getTokenStatus(account: DeltaAccountRecord): TokenStatus {
  if (!account.hasAccessToken) return "none";
  if (account.expiresAt === null) return "none";
  const now = Date.now();
  const expires = account.expiresAt * 1000;
  if (expires <= now) return "expired";
  if (expires <= now + EXPIRING_THRESHOLD_MS) return "expiring_soon";
  return "valid";
}

export function getTokenStatusLabel(status: TokenStatus, expiresAt: number | null): string {
  switch (status) {
    case "valid": return "令牌有效";
    case "expiring_soon": {
      if (expiresAt === null) return "即将过期";
      const days = Math.max(0, Math.ceil((expiresAt * 1000 - Date.now()) / 86400000));
      return `即将过期 ${days}天`;
    }
    case "expired": return "已过期";
    case "none": return "无令牌";
  }
}

export function getCapabilities(kind: AccountKind): Capability[] {
  return ACCOUNT_KIND_CAPABILITIES[kind] ?? [];
}

export function canRefreshToken(kind: AccountKind): boolean {
  return kind === "qq" || kind === "wechat";
}

export function getAccountDisplayName(account: DeltaAccountRecord): string {
  const id = account.uinOrOpenid;
  return id.length > 12 ? `${id.slice(0, 8)}...` : id;
}

export function getAcctypeForKind(kind: AccountKind): "qc" | "wx" {
  return kind === "wechat" || kind === "wegameWechat" ? "wx" : "qc";
}
