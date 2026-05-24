import { describe, it, expect } from "vitest";
import {
  EXPIRING_THRESHOLD_MS,
  ACCOUNT_KIND_LABELS,
  ACCOUNT_KIND_CAPABILITIES,
  CAPABILITY_LABELS,
  LOGIN_FLOW_KINDS,
  LOGIN_FLOW_MODE_MAP,
  QUERY_WORKBENCH_KINDS,
  QUERY_WORKBENCH_LABELS,
} from "@/components/app/delta-types";
import type { AccountKind, Capability } from "@/components/app/delta-types";

describe("EXPIRING_THRESHOLD_MS", () => {
  it("equals 3 days in milliseconds", () => {
    expect(EXPIRING_THRESHOLD_MS).toBe(3 * 24 * 60 * 60 * 1000);
  });
});

describe("ACCOUNT_KIND_LABELS", () => {
  const kinds: AccountKind[] = ["qq", "wechat", "qqSafe", "wegameQq", "wegameWechat", "pioneer"];

  it("has a label for every AccountKind variant", () => {
    for (const kind of kinds) {
      expect(ACCOUNT_KIND_LABELS[kind]).toBeDefined();
      expect(ACCOUNT_KIND_LABELS[kind].length).toBeGreaterThan(0);
    }
  });

  it("has no extra keys beyond AccountKind variants", () => {
    const keys = Object.keys(ACCOUNT_KIND_LABELS);
    expect(keys).toHaveLength(kinds.length);
    for (const kind of kinds) {
      expect(keys).toContain(kind);
    }
  });
});

describe("ACCOUNT_KIND_CAPABILITIES", () => {
  it("matches serde camelCase AccountKind keys", () => {
    expect(ACCOUNT_KIND_CAPABILITIES["qqSafe"]).toBeDefined();
    expect(ACCOUNT_KIND_CAPABILITIES["wegameQq"]).toBeDefined();
    expect(ACCOUNT_KIND_CAPABILITIES["wegameWechat"]).toBeDefined();
    expect(ACCOUNT_KIND_CAPABILITIES.pioneer).toBeDefined();
  });

  it("does NOT contain snake_case keys (would break with Rust serde)", () => {
    expect(ACCOUNT_KIND_CAPABILITIES).not.toHaveProperty("qqsafe");
    expect(ACCOUNT_KIND_CAPABILITIES).not.toHaveProperty("wegame_qq");
    expect(ACCOUNT_KIND_CAPABILITIES).not.toHaveProperty("wegame_wechat");
  });

  it("every capability in each array is a valid Capability", () => {
    const validCapabilities: Capability[] = ["game_data", "wegame", "qqsafe", "pioneer"];
    for (const kind of Object.keys(ACCOUNT_KIND_CAPABILITIES) as AccountKind[]) {
      for (const cap of ACCOUNT_KIND_CAPABILITIES[kind]) {
        expect(validCapabilities).toContain(cap);
      }
    }
  });

  it("qq and wechat both have game_data capability", () => {
    expect(ACCOUNT_KIND_CAPABILITIES.qq).toContain("game_data");
    expect(ACCOUNT_KIND_CAPABILITIES.wechat).toContain("game_data");
  });

  it("pioneer has pioneer capability", () => {
    expect(ACCOUNT_KIND_CAPABILITIES.pioneer).toEqual(["pioneer"]);
  });
});

describe("CAPABILITY_LABELS", () => {
  it("has a label for every Capability variant", () => {
    const caps: Capability[] = ["game_data", "wegame", "qqsafe", "pioneer"];
    for (const cap of caps) {
      expect(CAPABILITY_LABELS[cap]).toBeDefined();
    }
  });
});

describe("LOGIN_FLOW_MODE_MAP", () => {
  it("maps all QQ-mode flows to 'qq'", () => {
    expect(LOGIN_FLOW_MODE_MAP.qq).toBe("qq");
    expect(LOGIN_FLOW_MODE_MAP.qqsafe).toBe("qq");
    expect(LOGIN_FLOW_MODE_MAP.wegame_qq).toBe("qq");
    expect(LOGIN_FLOW_MODE_MAP.pioneer).toBe("qq");
  });

  it("maps all WeChat-mode flows to 'wechat'", () => {
    expect(LOGIN_FLOW_MODE_MAP.wechat).toBe("wechat");
    expect(LOGIN_FLOW_MODE_MAP.wegame_wechat).toBe("wechat");
  });

  it("covers all LOGIN_FLOW_KINDS", () => {
    for (const kind of LOGIN_FLOW_KINDS) {
      expect(LOGIN_FLOW_MODE_MAP[kind]).toBeDefined();
    }
  });
});

describe("LOGIN_FLOW_KINDS", () => {
  it("contains all 6 flow kinds", () => {
    expect(LOGIN_FLOW_KINDS).toHaveLength(6);
    expect(LOGIN_FLOW_KINDS).toContain("qq");
    expect(LOGIN_FLOW_KINDS).toContain("wechat");
    expect(LOGIN_FLOW_KINDS).toContain("qqsafe");
    expect(LOGIN_FLOW_KINDS).toContain("wegame_qq");
    expect(LOGIN_FLOW_KINDS).toContain("wegame_wechat");
    expect(LOGIN_FLOW_KINDS).toContain("pioneer");
  });
});

describe("QUERY_WORKBENCH_KINDS", () => {
  it("contains all 6 query kinds", () => {
    expect(QUERY_WORKBENCH_KINDS).toHaveLength(6);
  });

  it("every kind has a label", () => {
    for (const kind of QUERY_WORKBENCH_KINDS) {
      expect(QUERY_WORKBENCH_LABELS[kind]).toBeDefined();
      expect(QUERY_WORKBENCH_LABELS[kind].length).toBeGreaterThan(0);
    }
  });
});

describe("AccountKind camelCase consistency", () => {
  it("frontend AccountKind values match Rust serde camelCase output", () => {
    // Rust #[serde(rename_all = "camelCase")] produces:
    // Qq → "qq", Wechat → "wechat", QqSafe → "qqSafe",
    // WegameQq → "wegameQq", WegameWechat → "wegameWechat", Pioneer → "pioneer"
    const expected: AccountKind[] = ["qq", "wechat", "qqSafe", "wegameQq", "wegameWechat", "pioneer"];
    const actual = Object.keys(ACCOUNT_KIND_LABELS) as AccountKind[];
    expect(actual.sort()).toEqual(expected.sort());
  });
});
