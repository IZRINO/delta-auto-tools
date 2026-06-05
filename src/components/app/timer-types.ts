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

export type TimerSettings = {
  enabled?: boolean;
  timerEnabled?: boolean;
  counterEnabled?: boolean;
  display: TimerDisplaySettings;
  counterDisplay: TimerDisplaySettings;
  timerGroups?: TimerGroup[];
  counterGroups?: CounterGroup[];
  timers: TimerItem[];
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

export type TimerBootstrap = {
  settings: TimerSettings;
  runs: TimerRunState[];
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
  counterEnabled: boolean;
  display: {
    rect: TimerRect;
    fontOpacity: string;
  };
  counterDisplay: {
    rect: TimerRect;
    fontOpacity: string;
  };
  timerGroups: TimerGroupForm[];
  counterGroups: TimerGroupForm[];
  timers: TimerItemForm[];
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

export type TimerDisplayTarget = "timer" | "counter";

export type TimerSelectionOutcome = {
  kind: "selected" | "cancelled" | "closed";
  rect: TimerRect;
  target: TimerDisplayTarget;
  groupId?: string | null;
};

export type TimerDisplayMode = "display" | "position" | "counter-display" | "counter-position";

export const TIMER_AUTOSAVE_DELAY_MS = 400;
export const TIMER_DISPLAY_WIDTH = 320;
export const TIMER_DISPLAY_MIN_HEIGHT = 96;
export const DEFAULT_TIMER_GROUP_ID = "default-timer-group";
export const DEFAULT_COUNTER_GROUP_ID = "default-counter-group";
