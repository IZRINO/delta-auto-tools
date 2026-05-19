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

export type TimerDirection = "countdown" | "countup";

export type TimerItem = {
  id: string;
  name: string;
  durationSeconds: number;
  hotkey: string;
  direction: TimerDirection;
  enabled: boolean;
  ignoreRunning: boolean;
  segmentCount: number | null;
};

export type CounterItem = {
  id: string;
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
  name: string;
  durationSeconds: string;
  hotkey: string;
  direction: TimerDirection;
  enabled: boolean;
  ignoreRunning: boolean;
  segmentCount: string;
};

export type CounterItemForm = {
  id: string;
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
  timers: TimerItemForm[];
  counters: CounterItemForm[];
};

export type TimerDisplayTarget = "timer" | "counter";

export type TimerSelectionOutcome = {
  kind: "selected" | "cancelled" | "closed";
  rect: TimerRect;
  target: TimerDisplayTarget;
};

export type TimerDisplayMode = "display" | "position" | "counter-display" | "counter-position";

export const TIMER_AUTOSAVE_DELAY_MS = 400;
export const TIMER_DISPLAY_WIDTH = 320;
export const TIMER_DISPLAY_MIN_HEIGHT = 96;
