import type { RegionRect } from "@/components/app/morse-types";

export type AudioTriggerMode = "hotkey" | "regionWatch";

export type AudioCard = {
  id: string;
  name: string;
  enabled: boolean;
  triggerMode: AudioTriggerMode;
  hotkey: string | null;
  watchRegion: RegionRect | null;
  watchReferenceImagePath: string | null;
  watchMatchThreshold: number;
  watchPollIntervalMs: number;
  audioFilePath: string;
  volume: number;
  cooldownMs: number;
};

export type AudioSettings = {
  audioEnabled: boolean;
  cards: AudioCard[];
};

export type AudioBootstrap = {
  settings: AudioSettings;
  hotkeyError: string | null;
};

export type AudioSettingsForm = {
  audioEnabled: boolean;
  cards: AudioCardForm[];
};

export type AudioCardForm = {
  id: string;
  name: string;
  enabled: boolean;
  triggerMode: AudioTriggerMode;
  hotkey: string;
  watchRegion: RegionRect | null;
  watchReferenceImagePath: string;
  watchMatchThreshold: string;
  watchPollIntervalMs: string;
  audioFilePath: string;
  volume: string;
  cooldownMs: string;
};

export const DEFAULT_AUDIO_CARD: AudioCard = {
  id: "",
  name: "",
  enabled: true,
  triggerMode: "hotkey",
  hotkey: null,
  watchRegion: null,
  watchReferenceImagePath: null,
  watchMatchThreshold: 0.9,
  watchPollIntervalMs: 500,
  audioFilePath: "",
  volume: 0.8,
  cooldownMs: 1000,
};

export const DEFAULT_AUDIO_SETTINGS: AudioSettings = {
  audioEnabled: true,
  cards: [],
};

export const AUDIO_AUTOSAVE_DELAY_MS = 400;
