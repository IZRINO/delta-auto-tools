/**
 * 多配置 Profile 前端类型定义。
 *
 * 与 Rust 端 `src-tauri/src/profile/types.rs` 保持结构一致，
 * 所有字段名使用 camelCase（Rust 端 `#[serde(rename_all = "camelCase")]`）。
 */

import type {MorseSettings} from "@/components/app/morse-types";
import type {CounterSettings, TimerSettings} from "@/components/app/timer-types";
import type {RapidfireSettings} from "@/components/app/rapidfire-types";
import type {AudioSettings} from "@/components/app/audio-types";

/** 单个工具的配置快照（5 选 1，其余为 None）。 */
export interface ToolSettingsSnapshot {
    morse?: MorseSettings | null;
    timer?: TimerSettings | null;
    counter?: CounterSettings | null;
    rapidfire?: RapidfireSettings | null;
    audio?: AudioSettings | null;
}

/** 一个完整 Profile。 */
export interface Profile {
    id: string;
    name: string;
    /** 创建时间 unix 毫秒。 */
    createdAt: number;
    /** 最后更新时间 unix 毫秒。 */
    updatedAt: number;
    snapshot: ToolSettingsSnapshot;
}

/** Profile 持久化设置。 */
export interface ProfileSettings {
    profiles: Profile[];
    /** 当前激活 Profile id。空串表示「默认」（未保存的现场）。 */
    activeProfileId: string;
}

/** Profile bootstrap：一次性返回前端所需的全部信息。 */
export interface ProfileBootstrap {
    profiles: Profile[];
    activeProfileId: string;
}
