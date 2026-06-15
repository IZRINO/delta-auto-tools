import {type Event, listen} from "@tauri-apps/api/event";
import type {CounterBootstrap, TimerBootstrap} from "@/components/app/timer-types";
import type {MorseRunResult, RegionSelectionProgress} from "@/components/app/morse-types";
import type {RapidfireBootstrap} from "@/components/app/rapidfire-types";
import type {AudioBootstrap} from "@/components/app/audio-types";
import type {UpdateProgress} from "@/components/app/about-types";

export const MORSE_EVENTS = {
    runFinished: {name: "morse://run-finished" as const, payload: null as unknown as MorseRunResult},
    selectionProgress: {
        name: "morse://selection-progress" as const,
        payload: null as unknown as RegionSelectionProgress
    },
    hotkeyError: {name: "morse://hotkey-error" as const, payload: null as unknown as string},
} as const;

export const TIMER_EVENTS = {
    stateChanged: {name: "timer://state-changed" as const, payload: null as unknown as TimerBootstrap},
    hotkeyError: {name: "timer://hotkey-error" as const, payload: null as unknown as string},
    hotkeyTriggered: {name: "timer://hotkey-triggered" as const, payload: null as unknown as string[]},
} as const;

export const COUNTER_EVENTS = {
    stateChanged: {name: "counter://state-changed" as const, payload: null as unknown as CounterBootstrap},
    hotkeyError: {name: "counter://hotkey-error" as const, payload: null as unknown as string},
    hotkeyTriggered: {name: "counter://hotkey-triggered" as const, payload: null as unknown as string[]},
} as const;

export const RAPIDFIRE_EVENTS = {
    stateChanged: {name: "rapidfire://state-changed" as const, payload: null as unknown as RapidfireBootstrap},
    hotkeyError: {name: "rapidfire://hotkey-error" as const, payload: null as unknown as string},
} as const;

export const AUDIO_EVENTS = {
    stateChanged: {name: "audio://state-changed" as const, payload: null as unknown as AudioBootstrap},
    hotkeyTriggered: {name: "audio://hotkey-triggered" as const, payload: null as unknown as string},
    regionMatched: {name: "audio://region-matched" as const, payload: null as unknown as string},
    hotkeyError: {name: "audio://hotkey-error" as const, payload: null as unknown as string},
} as const;

export const GLOBAL_EVENTS = {
    enabledChanged: {name: "global://enabled-changed" as const, payload: null as unknown as boolean},
} as const;

export const ABOUT_EVENTS = {
    updateProgress: {name: "about://update-progress" as const, payload: null as unknown as UpdateProgress},
} as const;

export async function listenEvent<T extends { name: string; payload: unknown }>(
    event: T,
    handler: (event: Event<T["payload"]>) => void | Promise<void>
) {
    return listen<T["payload"]>(event.name, handler);
}
