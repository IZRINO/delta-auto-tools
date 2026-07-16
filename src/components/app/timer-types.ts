export type TimerRect = {
    x: number;
    y: number;
    width: number;
    height: number;
};

export type TimerDisplaySettings = {
    rect: TimerRect;
    fontOpacity: number;
};

export type TimerGroup = {
    id: string;
    name: string;
    enabled: boolean;
    display: TimerDisplaySettings;
};

export type CounterGroup = {
    id: string;
    name: string;
    enabled: boolean;
    display: TimerDisplaySettings;
};

export type TimerDirection = "countdown" | "countup";

export type TimerTriggerMode = "press" | "release";

export type TimerItem = {
    id: string;
    groupId?: string;
    name: string;
    durationSeconds: number;
    hotkey: string;
    direction: TimerDirection;
    triggerMode: TimerTriggerMode;
    enabled: boolean;
    ignoreRunning: boolean;
    segmentCount: number | null;
};

export type CounterItem = {
    id: string;
    groupId?: string;
    name: string;
    startValue: number;
    hotkey: string;
    enabled: boolean;
};

/** 计时器后端 Settings（仅 timer 字段） */
export type TimerSettings = {
    enabled?: boolean;
    timerEnabled?: boolean;
    display: TimerDisplaySettings;
    timerGroups?: TimerGroup[];
    timers: TimerItem[];
};

/** 计数器后端 Settings（仅 counter 字段） */
export type CounterSettings = {
    enabled?: boolean;
    counterEnabled?: boolean;
    display: TimerDisplaySettings;
    counterGroups?: CounterGroup[];
    counters: CounterItem[];
};

export type TimerRunStatus = "running" | "finished";

export type TimerRunState = {
    id: string;
    currentSeconds: number;
    remainingSeconds: number;
    durationSeconds: number;
    direction: TimerDirection;
    status: TimerRunStatus;
    segmentCount: number | null;
    segmentDuration: number;
    recovering: boolean;
    recoveringCount: number;
    activeSegmentIndex: number;
    startedAtMs: number;
    recoveryStartPool: number;
};

export type CounterRunState = {
    id: string;
    value: number;
};

export type TimerRunsChanged = {
    runs: TimerRunState[];
};

export type CounterRunsChanged = {
    counterRuns: CounterRunState[];
};

/** 计时器 Bootstrap */
export type TimerBootstrap = {
    settings: TimerSettings;
    runs: TimerRunState[];
    hotkeyError: string | null;
};

/** 计数器 Bootstrap */
export type CounterBootstrap = {
    settings: CounterSettings;
    counterRuns: CounterRunState[];
    hotkeyError: string | null;
};

export type TimerItemForm = {
    id: string;
    groupId: string;
    name: string;
    durationSeconds: string;
    hotkey: string;
    direction: TimerDirection;
    triggerMode: TimerTriggerMode;
    enabled: boolean;
    ignoreRunning: boolean;
    segmentCount: string;
};

export type CounterItemForm = {
    id: string;
    groupId: string;
    name: string;
    startValue: string;
    hotkey: string;
    enabled: boolean;
};

export type TimerSettingsForm = {
    timerEnabled: boolean;
    display: {
        rect: TimerRect;
        fontOpacity: string;
    };
    timerGroups: TimerGroupForm[];
    timers: TimerItemForm[];
};

export type CounterSettingsForm = {
    counterEnabled: boolean;
    display: {
        rect: TimerRect;
        fontOpacity: string;
    };
    counterGroups: TimerGroupForm[];
    counters: CounterItemForm[];
};

export type TimerGroupForm = {
    id: string;
    name: string;
    enabled: boolean;
    display: {
        rect: TimerRect;
        fontOpacity: string;
    };
};

export type TimerSelectionOutcome = {
    kind: "selected" | "cancelled" | "closed";
    rect: TimerRect;
    groupId?: string | null;
};

export type CounterSelectionOutcome = TimerSelectionOutcome;

export const TIMER_AUTOSAVE_DELAY_MS = 400;
export const TIMER_DISPLAY_WIDTH = 320;
export const TIMER_DISPLAY_MIN_HEIGHT = 96;
export const DEFAULT_TIMER_GROUP_ID = "default-timer-group";
export const DEFAULT_COUNTER_GROUP_ID = "default-counter-group";
