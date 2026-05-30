import { describe, it, expect } from "vitest";
import {
  getTokenStatus,
  getTokenStatusLabel,
  getCapabilities,
  canRefreshToken,
  getAccountDisplayName,
  getAcctypeForKind,
} from "@/components/app/delta-utils";
import type { DeltaAccountRecord } from "@/components/app/delta-types";
import { EXPIRING_THRESHOLD_MS } from "@/components/app/delta-types";

const now = Date.now();

function makeAccount(overrides: Partial<DeltaAccountRecord> = {}): DeltaAccountRecord {
  return {
    id: 1,
    kind: "qq",
    uinOrOpenid: "1234567890",
    hasAccessToken: true,
    expiresAt: null,
    createdAt: 1700000000,
    updatedAt: 1700000000,
    ...overrides,
  };
}

describe("getTokenStatus", () => {
  it("returns 'none' when account has no access token", () => {
    expect(getTokenStatus(makeAccount({ hasAccessToken: false }))).toBe("none");
  });

  it("returns 'none' when expiresAt is null", () => {
    expect(getTokenStatus(makeAccount({ expiresAt: null }))).toBe("none");
  });

  it("returns 'expired' when expiresAt is in the past", () => {
    expect(getTokenStatus(makeAccount({ expiresAt: Math.floor((now - 1000) / 1000) }))).toBe("expired");
  });

  it("returns 'expired' when expiresAt equals now (boundary)", () => {
    expect(getTokenStatus(makeAccount({ expiresAt: Math.floor(now / 1000) }))).toBe("expired");
  });

  it("returns 'expiring_soon' when within threshold (1ms before threshold)", () => {
    const expiresAt = Math.floor((now + EXPIRING_THRESHOLD_MS - 1) / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt }))).toBe("expiring_soon");
  });

  it("returns 'valid' when exactly at threshold boundary", () => {
    const expiresAt = Math.ceil((now + EXPIRING_THRESHOLD_MS + 1000) / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt }))).toBe("valid");
  });

  it("returns 'valid' when far in the future", () => {
    const expiresAt = Math.floor((now + EXPIRING_THRESHOLD_MS * 2) / 1000);
    expect(getTokenStatus(makeAccount({ expiresAt }))).toBe("valid");
  });
});

describe("getTokenStatusLabel", () => {
  it("returns label for valid token", () => {
    expect(getTokenStatusLabel("valid", null)).toBe("令牌有效");
  });

  it("returns label with days for expiring token", () => {
    const expiresAt = Math.floor((Date.now() + 2 * 86400000) / 1000);
    expect(getTokenStatusLabel("expiring_soon", expiresAt)).toMatch(/^即将过期 \d+天$/);
  });

  it("returns fallback label for expiring token without expiry", () => {
    expect(getTokenStatusLabel("expiring_soon", null)).toBe("即将过期");
  });

  it("returns expired label", () => {
    expect(getTokenStatusLabel("expired", null)).toBe("已过期");
  });

  it("returns none label", () => {
    expect(getTokenStatusLabel("none", null)).toBe("无令牌");
  });
});

describe("getCapabilities", () => {
  it("returns game_data for qq", () => {
    expect(getCapabilities("qq")).toContain("game_data");
  });

  it("returns game_data for wechat", () => {
    expect(getCapabilities("wechat")).toContain("game_data");
  });

  it("returns qqsafe for qqSafe", () => {
    expect(getCapabilities("qqSafe")).toEqual(["qqsafe"]);
  });

  it("returns wegame for Wegame account kinds", () => {
    expect(getCapabilities("wegameQq")).toEqual(["wegame"]);
    expect(getCapabilities("wegameWechat")).toEqual(["wegame"]);
  });

  it("returns pioneer for pioneer", () => {
    expect(getCapabilities("pioneer")).toEqual(["pioneer"]);
  });
});

describe("canRefreshToken", () => {
  it("allows qq and wechat", () => {
    expect(canRefreshToken("qq")).toBe(true);
    expect(canRefreshToken("wechat")).toBe(true);
  });

  it("does not allow one-off tool account kinds", () => {
    expect(canRefreshToken("qqSafe")).toBe(false);
    expect(canRefreshToken("wegameQq")).toBe(false);
    expect(canRefreshToken("wegameWechat")).toBe(false);
    expect(canRefreshToken("pioneer")).toBe(false);
  });
});

describe("getAccountDisplayName", () => {
  it("returns uinOrOpenid for short identifiers", () => {
    expect(getAccountDisplayName(makeAccount({ uinOrOpenid: "123456" }))).toBe("123456");
  });

  it("truncates long identifiers", () => {
    expect(getAccountDisplayName(makeAccount({ uinOrOpenid: "openid_1234567890" }))).toBe("openid_1...");
  });
});

describe("getAcctypeForKind", () => {
  it("returns wx for wechat kinds", () => {
    expect(getAcctypeForKind("wechat")).toBe("wx");
    expect(getAcctypeForKind("wegameWechat")).toBe("wx");
  });

  it("returns qc for non-wechat kinds", () => {
    expect(getAcctypeForKind("qq")).toBe("qc");
    expect(getAcctypeForKind("qqSafe")).toBe("qc");
    expect(getAcctypeForKind("wegameQq")).toBe("qc");
    expect(getAcctypeForKind("pioneer")).toBe("qc");
  });
});
