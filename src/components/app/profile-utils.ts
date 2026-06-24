/**
 * Profile 纯逻辑工具函数。不依赖 React / Tauri，可在 Node 测试。
 */

import type {Profile, ProfileBootstrap} from "@/components/app/profile-types";

/** 把 unix 毫秒时间戳格式化为 `YYYY-MM-DD HH:mm` 本地显示字符串。 */
export function formatProfileTimestamp(ms: number): string {
    if (!Number.isFinite(ms) || ms <= 0) return "—";
    const d = new Date(ms);
    if (Number.isNaN(d.getTime())) return "—";
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** 校验 Profile 名称：非空且去空白后长度 1-40。 */
export function validateProfileName(name: string): string | null {
    const trimmed = name.trim();
    if (trimmed.length === 0) return "配置名称不能为空";
    if (trimmed.length > 40) return "配置名称不能超过 40 个字符";
    return null;
}

/** 在列表中查找指定 id 的 Profile。 */
export function findProfile(
    profiles: readonly Profile[],
    id: string,
): Profile | undefined {
    return profiles.find((p) => p.id === id);
}

/**
 * 判断某个 Profile 是否包含指定工具的快照。
 *
 * 用于面板展示「该配置包含哪些工具」，以及切换前的提示。
 */
export function snapshotTools(
    profile: Profile,
): Array<{key: string; label: string; included: boolean}> {
    const s = profile.snapshot;
    return [
        {key: "morse", label: "摩斯", included: Boolean(s.morse)},
        {key: "timer", label: "计时器", included: Boolean(s.timer)},
        {key: "counter", label: "计数器", included: Boolean(s.counter)},
        {key: "rapidfire", label: "连发器", included: Boolean(s.rapidfire)},
        {key: "audio", label: "音频", included: Boolean(s.audio)},
    ];
}

/** 统计 Profile 快照中包含的工具数量。 */
export function countIncludedTools(profile: Profile): number {
    return snapshotTools(profile).filter((t) => t.included).length;
}

/** 判断给定 id 是否为当前激活 Profile。 */
export function isActiveProfile(boot: ProfileBootstrap | null, id: string): boolean {
    return Boolean(boot && boot.activeProfileId === id);
}

/** 获取当前激活的 Profile 对象。无 bootstrap 或未命中时返回 null。 */
export function getActiveProfile(boot: ProfileBootstrap | null): Profile | null {
    if (!boot) return null;
    return boot.profiles.find((p) => p.id === boot.activeProfileId) ?? null;
}

/** 获取顶栏显示名；无激活配置时回退为"配置1"。 */
export function getProfileDisplayName(boot: ProfileBootstrap | null): string {
    return getActiveProfile(boot)?.name ?? "配置1";
}

/**
 * 按 Switcher 下拉需求排序：当前激活配置排在首位，其余保持原顺序。
 */
export function sortProfilesForSwitcher(
    profiles: readonly Profile[],
    activeProfileId: string,
): Profile[] {
    const active = profiles.find((p) => p.id === activeProfileId);
    const rest = profiles.filter((p) => p.id !== activeProfileId);
    return active ? [active, ...rest] : [...profiles];
}
