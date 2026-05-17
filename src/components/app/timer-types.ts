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

export type TimerItem = {
  id: string;
  name: string;
  durationSeconds: number;
  hotkey: string;
};

export type TimerSettings = {
  enabled: boolean;
  display: TimerDisplaySettings;
  timers: TimerItem[];
};

export type TimerRunStatus = "running" | "finished";

export type TimerRunState = {
  id: string;
  remainingSeconds: number;
  status: TimerRunStatus;
};

export type TimerBootstrap = {
  settings: TimerSettings;
  runs: TimerRunState[];
  hotkeyError: string | null;
};

export type TimerItemForm = {
  id: string;
  name: string;
  durationSeconds: string;
  hotkey: string;
};

export type TimerSettingsForm = {
  enabled: boolean;
  display: {
    rect: TimerRect;
    fontOpacity: string;
  };
  timers: TimerItemForm[];
};

export type TimerSelectionOutcome = {
  kind: "selected" | "cancelled" | "closed";
  rect: TimerRect;
};

export type TimerDisplayMode = "display" | "position";

export const TIMER_AUTOSAVE_DELAY_MS = 400;
export const TIMER_DISPLAY_WIDTH = 320;
export const TIMER_DISPLAY_MIN_HEIGHT = 96;
