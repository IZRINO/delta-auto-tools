import {describe, expect, it} from "vitest";

import type {Profile, ProfileBootstrap} from "@/components/app/profile-types";
import {
    countIncludedTools,
    findProfile,
    formatProfileTimestamp,
    getActiveProfile,
    getProfileDisplayName,
    isActiveProfile,
    sortProfilesForSwitcher,
    snapshotTools,
    validateProfileName,
} from "@/components/app/profile-utils";

function emptySnapshot() {
    return {
        morse: null,
        timer: null,
        counter: null,
        rapidfire: null,
        audio: null,
    };
}

function makeProfile(id: string, name = id): Profile {
    return {
        id,
        name,
        createdAt: 1700000000000,
        updatedAt: 1700000000000,
        snapshot: emptySnapshot(),
    };
}

describe("formatProfileTimestamp", () => {
    it("格式化合法时间戳", () => {
        const ms = new Date(2024, 5, 18, 14, 30).getTime();
        const s = formatProfileTimestamp(ms);
        expect(s).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    });

    it("非法时间戳返回占位符", () => {
        expect(formatProfileTimestamp(0)).toBe("—");
        expect(formatProfileTimestamp(-1)).toBe("—");
        expect(formatProfileTimestamp(Number.NaN)).toBe("—");
    });
});

describe("validateProfileName", () => {
    it("空名称返回错误", () => {
        expect(validateProfileName("")).toBe("配置名称不能为空");
        expect(validateProfileName("   ")).toBe("配置名称不能为空");
    });

    it("超长名称返回错误", () => {
        expect(validateProfileName("a".repeat(41))).toBe("配置名称不能超过 40 个字符");
    });

    it("合法名称返回 null", () => {
        expect(validateProfileName("PVE 配置")).toBeNull();
        expect(validateProfileName("  带空格  ")).toBeNull();
    });
});

describe("findProfile", () => {
    it("返回匹配项", () => {
        const profiles = [makeProfile("a"), makeProfile("b")];
        expect(findProfile(profiles, "b")?.id).toBe("b");
    });

    it("未找到返回 undefined", () => {
        expect(findProfile([makeProfile("a")], "missing")).toBeUndefined();
    });
});

describe("snapshotTools", () => {
    it("全部为 null 时全部未包含", () => {
        const p = makeProfile("a");
        const tools = snapshotTools(p);
        expect(tools).toHaveLength(5);
        expect(tools.every((t) => !t.included)).toBe(true);
    });

    it("含 morse 时 morse 标记为已包含", () => {
        const p: Profile = {
            ...makeProfile("a"),
            snapshot: {...emptySnapshot(), morse: {hotkey: "F1"} as never},
        };
        const tools = snapshotTools(p);
        expect(tools.find((t) => t.key === "morse")?.included).toBe(true);
        expect(tools.find((t) => t.key === "timer")?.included).toBe(false);
    });
});

describe("countIncludedTools", () => {
    it("空快照返回 0", () => {
        expect(countIncludedTools(makeProfile("a"))).toBe(0);
    });

    it("统计已包含工具数", () => {
        const p: Profile = {
            ...makeProfile("a"),
            snapshot: {
                ...emptySnapshot(),
                timer: {timers: []} as never,
                audio: {cards: []} as never,
            },
        };
        expect(countIncludedTools(p)).toBe(2);
    });
});

describe("isActiveProfile", () => {
    it("激活时返回 true", () => {
        const boot: ProfileBootstrap = {profiles: [makeProfile("a")], activeProfileId: "a"};
        expect(isActiveProfile(boot, "a")).toBe(true);
    });

    it("非激活时返回 false", () => {
        const boot: ProfileBootstrap = {profiles: [makeProfile("a")], activeProfileId: "b"};
        expect(isActiveProfile(boot, "a")).toBe(false);
    });

    it("bootstrap 为 null 时返回 false", () => {
        expect(isActiveProfile(null, "a")).toBe(false);
    });
});

describe("getActiveProfile", () => {
    it("返回当前激活配置", () => {
        const boot: ProfileBootstrap = {
            profiles: [makeProfile("a", "配置1"), makeProfile("b", "配置2")],
            activeProfileId: "b",
        };
        expect(getActiveProfile(boot)?.name).toBe("配置2");
    });

    it("无 bootstrap 返回 null", () => {
        expect(getActiveProfile(null)).toBeNull();
    });

    it("未命中 activeProfileId 返回 null", () => {
        const boot: ProfileBootstrap = {
            profiles: [makeProfile("a")],
            activeProfileId: "missing",
        };
        expect(getActiveProfile(boot)).toBeNull();
    });
});

describe("getProfileDisplayName", () => {
    it("无 bootstrap 时显示配置1", () => {
        expect(getProfileDisplayName(null)).toBe("配置1");
    });

    it("空 profiles 时显示配置1", () => {
        expect(getProfileDisplayName({profiles: [], activeProfileId: ""})).toBe("配置1");
    });

    it("有激活配置时显示配置名称", () => {
        const boot: ProfileBootstrap = {
            profiles: [makeProfile("a", "配置A")],
            activeProfileId: "a",
        };
        expect(getProfileDisplayName(boot)).toBe("配置A");
    });
});

describe("sortProfilesForSwitcher", () => {
    it("当前配置排在第一位，其余保持原顺序", () => {
        const profiles = [makeProfile("a"), makeProfile("b"), makeProfile("c")];
        expect(sortProfilesForSwitcher(profiles, "b").map((p) => p.id)).toEqual(["b", "a", "c"]);
    });

    it("未命中 activeId 时保持原顺序", () => {
        const profiles = [makeProfile("a"), makeProfile("b")];
        expect(sortProfilesForSwitcher(profiles, "missing").map((p) => p.id)).toEqual(["a", "b"]);
    });
});
