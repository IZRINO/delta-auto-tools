import type {
  AudioCard,
  AudioCardForm,
  AudioSettings,
  AudioSettingsForm,
} from "@/components/app/audio-types";
import { DEFAULT_AUDIO_CARD } from "@/components/app/audio-types";

export function settingsToForm(settings: AudioSettings): AudioSettingsForm {
  return {
    audioEnabled: settings.audioEnabled,
    cards: settings.cards.map((card) => cardToForm(card)),
  };
}

function cardToForm(card: AudioCard): AudioCardForm {
  return {
    id: card.id,
    name: card.name,
    enabled: card.enabled,
    triggerMode: card.triggerMode,
    hotkey: card.hotkey ?? "",
    watchRegion: card.watchRegion,
    watchReferenceImagePath: card.watchReferenceImagePath ?? "",
    watchMatchThreshold: String(card.watchMatchThreshold),
    watchPollIntervalMs: String(card.watchPollIntervalMs),
    audioFilePath: card.audioFilePath,
    volume: String(card.volume),
    cooldownMs: String(card.cooldownMs),
  };
}

export function parseSettingsForm(form: AudioSettingsForm): AudioSettings {
  const cards = form.cards.map((card) => parseCardForm(card));
  return {
    audioEnabled: form.audioEnabled,
    cards,
  };
}

function parseCardForm(form: AudioCardForm): AudioCard {
  const name = form.name.trim();
  if (!name) {
    throw new Error("卡片名称不能为空。");
  }

  const volume = parseFloat(form.volume);
  if (Number.isNaN(volume) || volume < 0 || volume > 1) {
    throw new Error("音量必须在 0 到 1 之间。");
  }

  const cooldownMs = parseInt(form.cooldownMs, 10);
  if (Number.isNaN(cooldownMs) || cooldownMs < 0 || cooldownMs > 60000) {
    throw new Error("冷却时间必须在 0 到 60000 毫秒之间。");
  }

  const watchMatchThreshold = parseFloat(form.watchMatchThreshold);
  if (Number.isNaN(watchMatchThreshold) || watchMatchThreshold < 0 || watchMatchThreshold > 1) {
    throw new Error("匹配阈值必须在 0 到 1 之间。");
  }

  const watchPollIntervalMs = parseInt(form.watchPollIntervalMs, 10);
  if (Number.isNaN(watchPollIntervalMs) || watchPollIntervalMs < 100 || watchPollIntervalMs > 10000) {
    throw new Error("轮询间隔必须在 100 到 10000 毫秒之间。");
  }

  const hotkey = form.triggerMode === "hotkey" ? form.hotkey.trim() || null : null;
  if (form.triggerMode === "hotkey" && !hotkey) {
    throw new Error("快捷键模式下必须设置热键。");
  }

  return {
    id: form.id || generateCardId(),
    name,
    enabled: form.enabled,
    triggerMode: form.triggerMode,
    hotkey,
    watchRegion: form.triggerMode === "regionWatch" ? form.watchRegion : null,
    watchReferenceImagePath: form.triggerMode === "regionWatch" ? form.watchReferenceImagePath.trim() || null : null,
    watchMatchThreshold,
    watchPollIntervalMs,
    audioFilePath: form.audioFilePath.trim(),
    volume,
    cooldownMs,
  };
}

export function generateCardId(): string {
  return `audio-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
}

export function createEmptyAudioCard(): AudioCard {
  return {
    ...DEFAULT_AUDIO_CARD,
    id: generateCardId(),
  };
}

export function mergeAudioWatchRegionsIntoForm(
  current: AudioSettingsForm | null,
  settings: AudioSettings,
): AudioSettingsForm {
  const nextForm = settingsToForm(settings);
  if (!current) {
    return nextForm;
  }
  const byId = new Map(nextForm.cards.map((card) => [card.id, card]));
  return {
    ...current,
    cards: current.cards.map((card) => {
      const remote = byId.get(card.id);
      return remote ? { ...card, watchRegion: remote.watchRegion } : card;
    }),
  };
}

export function getAudioCardFormErrors(form: AudioCardForm): Record<string, string | null> {
  const errors: Record<string, string | null> = {};

  if (!form.name.trim()) {
    errors.name = "卡片名称不能为空";
  }

  const volume = parseFloat(form.volume);
  if (Number.isNaN(volume) || volume < 0 || volume > 1) {
    errors.volume = "音量必须在 0 到 1 之间";
  }

  const cooldownMs = parseInt(form.cooldownMs, 10);
  if (Number.isNaN(cooldownMs) || cooldownMs < 0 || cooldownMs > 60000) {
    errors.cooldownMs = "冷却时间必须在 0 到 60000 毫秒之间";
  }

  if (form.triggerMode === "hotkey" && !form.hotkey.trim()) {
    errors.hotkey = "必须设置快捷键";
  }

  if (form.triggerMode === "regionWatch" && !form.watchRegion) {
    errors.watchRegion = "必须设置监听区域";
  }

  return errors;
}
