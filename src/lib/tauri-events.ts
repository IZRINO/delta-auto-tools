/**
 * 前端事件名常量 —— 与后端各模块 events.rs 一一对齐。
 *
 * 调用方使用显式泛型：
 *   subscribeTauriEvent<MorseRunResult>(MORSE_EVENTS.runFinished, (event) => { ... })
 */

// ——— morse ——— morse/events.rs
export const MORSE_EVENTS = {
    runFinished: "morse://run-finished",
    selectionProgress: "morse://selection-progress",
    hotkeyError: "morse://hotkey-error",
} as const;

// ——— timer ——— timer/events.rs
export const TIMER_EVENTS = {
    stateChanged: "timer://state-changed",
    runsChanged: "timer://runs-changed",
    hotkeyError: "timer://hotkey-error",
    hotkeyTriggered: "timer://hotkey-triggered",
} as const;

// ——— counter ——— counter/events.rs
export const COUNTER_EVENTS = {
    stateChanged: "counter://state-changed",
    runsChanged: "counter://runs-changed",
    hotkeyError: "counter://hotkey-error",
    hotkeyTriggered: "counter://hotkey-triggered",
} as const;

// ——— rapidfire ——— rapidfire/events.rs
export const RAPIDFIRE_EVENTS = {
    stateChanged: "rapidfire://state-changed",
    runsChanged: "rapidfire://runs-changed",
    hotkeyError: "rapidfire://hotkey-error",
} as const;

// ——— recognition ——— recognition/events.rs
export const RECOGNITION_EVENTS = {
    stateChanged: "recognition://state-changed",
    hotkeyTriggered: "recognition://hotkey-triggered",
    regionMatched: "recognition://region-matched",
    hotkeyError: "recognition://hotkey-error",
} as const;

// ——— global ——— global_state.rs
export const GLOBAL_EVENTS = {
    enabledChanged: "global://enabled-changed",
} as const;

// ——— about ——— about/events.rs
export const ABOUT_EVENTS = {
    updateProgress: "about://update-progress",
} as const;

// ——— theme ——— theme/events.rs
export const THEME_EVENTS = {
    changed: "theme://changed",
} as const;

// ——— profile ——— profile/events.rs
export const PROFILE_EVENTS = {
    changed: "profile://changed",
} as const;
