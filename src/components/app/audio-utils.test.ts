import {describe, expect, it} from "vitest";
import {
    createEmptyAudioCard,
    generateCardId,
    mergeAudioWatchRegionsIntoForm,
    parseSettingsForm,
    settingsToForm,
} from "@/components/app/audio-utils";
import {DEFAULT_AUDIO_CARD} from "@/components/app/audio-types";

describe("audio-utils", () => {
    describe("settingsToForm", () => {
        it("converts empty settings to empty form", () => {
            const settings = {
                audioEnabled: true,
                cards: [],
            };
            const form = settingsToForm(settings);
            expect(form.audioEnabled).toBe(true);
            expect(form.cards).toHaveLength(0);
        });

        it("converts card numbers to strings", () => {
            const settings = {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "测试",
                        volume: 0.5,
                        cooldownMs: 2000,
                        watchMatchThreshold: 0.85,
                        watchPollIntervalMs: 1000,
                        hotkey: "Ctrl+F1",
                    },
                ],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].volume).toBe("0.5");
            expect(form.cards[0].cooldownMs).toBe("2000");
            expect(form.cards[0].watchMatchThreshold).toBe("0.85");
            expect(form.cards[0].watchPollIntervalMs).toBe("1000");
            expect(form.cards[0].hotkey).toBe("Ctrl+F1");
        });
    });

    describe("parseSettingsForm", () => {
        it("parses valid form back to settings", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "测试",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFilePath: "test.mp3",
                        volume: "0.8",
                        cooldownMs: "1000",
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].name).toBe("测试");
            expect(settings.cards[0].volume).toBe(0.8);
            expect(settings.cards[0].cooldownMs).toBe(1000);
            expect(settings.cards[0].hotkey).toBe("Ctrl+F1");
        });

        it("throws for empty name", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFilePath: "test.mp3",
                        volume: "0.8",
                        cooldownMs: "1000",
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("卡片名称不能为空");
        });

        it("throws for invalid volume", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "测试",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFilePath: "test.mp3",
                        volume: "1.5",
                        cooldownMs: "1000",
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("音量必须在 0 到 1 之间");
        });
    });

    describe("generateCardId", () => {
        it("generates unique ids", () => {
            const id1 = generateCardId();
            const id2 = generateCardId();
            expect(id1).not.toBe(id2);
            expect(id1).toMatch(/^audio-/);
        });
    });

    describe("createEmptyAudioCard", () => {
        it("creates card with defaults and new id", () => {
            const card = createEmptyAudioCard();
            expect(card.name).toBe("");
            expect(card.volume).toBe(0.8);
            expect(card.id).toMatch(/^audio-/);
        });
    });

    describe("mergeAudioWatchRegionsIntoForm", () => {
        it("merges backend watchRegion without overwriting local edits", () => {
            const current = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "本地未保存名称",
                        enabled: true,
                        triggerMode: "regionWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "local.png",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFilePath: "local.mp3",
                        volume: "0.8",
                        cooldownMs: "1000",
                    },
                ],
            };
            const merged = mergeAudioWatchRegionsIntoForm(current, {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "后端名称",
                        triggerMode: "regionWatch",
                        watchRegion: {x: 10, y: 20, width: 30, height: 40},
                        audioFilePath: "remote.mp3",
                    },
                ],
            });

            expect(merged.cards[0].name).toBe("本地未保存名称");
            expect(merged.cards[0].audioFilePath).toBe("local.mp3");
            expect(merged.cards[0].watchRegion).toEqual({x: 10, y: 20, width: 30, height: 40});
        });
    });
});
