import {describe, expect, it} from "vitest";
import {
    createEmptyAudioCard,
    generateCardId,
    getAudioCardFormErrors,
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
                        allowSimultaneous: false,
                    },
                ],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].volume).toBe("0.5");
            expect(form.cards[0].cooldownMs).toBe("2000");
            expect(form.cards[0].watchMatchThreshold).toBe("0.85");
            expect(form.cards[0].watchPollIntervalMs).toBe("1000");
            expect(form.cards[0].hotkey).toBe("Ctrl+F1");
            expect(form.cards[0].allowSimultaneous).toBe(false);
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
                        audioFiles: ["test.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].name).toBe("测试");
            expect(settings.cards[0].volume).toBe(0.8);
            expect(settings.cards[0].cooldownMs).toBe(1000);
            expect(settings.cards[0].hotkey).toBe("Ctrl+F1");
            expect(settings.cards[0].allowSimultaneous).toBe(false);
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
                        audioFiles: ["test.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
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
                        audioFiles: ["test.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "1.5",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
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
            expect(card.watchMatchThreshold).toBe(0.75);
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
                        audioFiles: ["local.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
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
                        audioFiles: ["remote.mp3"],
                    },
                ],
            });

            expect(merged.cards[0].name).toBe("本地未保存名称");
            expect(merged.cards[0].audioFiles).toEqual(["local.mp3"]);
            expect(merged.cards[0].watchRegion).toEqual({x: 10, y: 20, width: 30, height: 40});
        });

        it("merges backend color probe region without overwriting local color/tolerance", () => {
            // 本地 form：探针已设颜色/容差但 region 尚未框选（null）
            const current = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [
                            {
                                region: null,
                                targetColor: "#c86432",
                                tolerance: "40",
                            },
                        ],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            // 后端：探针 region 已被框选提交（10,20,5x5）
            const merged = mergeAudioWatchRegionsIntoForm(current, {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "识色",
                        triggerMode: "colorWatch",
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: [200, 100, 50] as [number, number, number],
                                tolerance: 40,
                            },
                        ],
                        colorMatchMode: "all",
                    },
                ],
            });

            // region 被后端回写
            expect(merged.cards[0].colorProbes[0].region).toEqual({x: 10, y: 20, width: 5, height: 5});
            // 本地草稿（颜色/容差）保留，不被后端覆盖
            expect(merged.cards[0].colorProbes[0].targetColor).toBe("#c86432");
            expect(merged.cards[0].colorProbes[0].tolerance).toBe("40");
        });

        it("不覆盖本地 comboWindows 草稿（Issue #62）", () => {
            // 本地 combo 模式卡，用户已为每段设了窗口草稿
            const current = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "连杀",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3", "b.mp3"],
                        playMode: "combo" as const,
                        comboWindowMs: "60000",
                        comboWindows: ["500", ""],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            // 后端推送的 settings 也含 comboWindows，但应被本地草稿优先（合并只覆盖 watchRegion/probe.region）
            const merged = mergeAudioWatchRegionsIntoForm(current, {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "连杀",
                        triggerMode: "hotkey",
                        hotkey: "Ctrl+F1",
                        audioFiles: ["a.mp3", "b.mp3"],
                        playMode: "combo",
                        comboWindowMs: 60000,
                        comboWindows: [9999, 9999],
                    },
                ],
            });
            // 本地 comboWindows 草稿保留，不被后端的 9999 覆盖
            expect(merged.cards[0].comboWindows).toEqual(["500", ""]);
        });
    });

    describe("colorWatch settingsToForm", () => {
        it("converts colorProbes to form with hex color string", () => {
            const settings = {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "识色",
                        triggerMode: "colorWatch" as const,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: [200, 100, 50] as [number, number, number],
                                tolerance: 40,
                            },
                        ],
                        colorMatchMode: "any" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].colorProbes).toHaveLength(1);
            expect(form.cards[0].colorProbes[0].targetColor).toBe("#c86432");
            expect(form.cards[0].colorProbes[0].tolerance).toBe("40");
            expect(form.cards[0].colorMatchMode).toBe("any");
        });
    });

    describe("colorWatch parseSettingsForm", () => {
        it("parses valid colorWatch form back to settings", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: "#c86432",
                                tolerance: "40",
                            },
                        ],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].triggerMode).toBe("colorWatch");
            expect(settings.cards[0].colorProbes).toHaveLength(1);
            expect(settings.cards[0].colorProbes[0].targetColor).toEqual([200, 100, 50]);
            expect(settings.cards[0].colorProbes[0].tolerance).toBe(40);
            expect(settings.cards[0].colorMatchMode).toBe("all");
        });

        it("throws when colorWatch has no probes", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("识色模式下至少需要配置一个探针");
        });

        it("throws for invalid tolerance", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: "#c86432",
                                tolerance: "300",
                            },
                        ],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("颜色容差必须在 0 到 255 之间");
        });

        // Issue #61: 新建探针 region 必为 null（用户尚未框选）。
        // flushSettings / autosave 走 parseSettingsForm，若对 region=null 抛错则框选流程被拦在第一步、
        // 且所有 autosave 失败导致「禁用卡片/关闭总开关」等变更无法落盘（Issue #60 下游）。
        // 此处断言未框选探针可作为中间态保存，region 保留 null，颜色/容差仍校验。
        it("允许未框选探针（region=null）保存为中间态", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3"],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [
                            {
                                region: null,
                                targetColor: "#c86432",
                                tolerance: "40",
                            },
                        ],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].colorProbes).toHaveLength(1);
            expect(settings.cards[0].colorProbes[0].region).toBeNull();
            expect(settings.cards[0].colorProbes[0].targetColor).toEqual([200, 100, 50]);
            expect(settings.cards[0].colorProbes[0].tolerance).toBe(40);
        });
    });

    describe("playMode / audioFiles", () => {
        it("settingsToForm 直传 audioFiles 数组与 playMode/comboWindowMs", () => {
            const settings = {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "连杀卡",
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        audioFiles: ["a.mp3", "b.mp3", "c.mp3"],
                        playMode: "combo" as const,
                        comboWindowMs: 30000,
                    },
                ],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].audioFiles).toEqual(["a.mp3", "b.mp3", "c.mp3"]);
            expect(form.cards[0].playMode).toBe("combo");
            expect(form.cards[0].comboWindowMs).toBe("30000");
        });

        it("parseSettingsForm 去除空字符串并回写 audioFiles 数组", () => {
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
                        audioFiles: ["a.mp3", "  ", ""],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        comboWindows: [],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].audioFiles).toEqual(["a.mp3"]);
            expect(settings.cards[0].playMode).toBe("single");
            expect(settings.cards[0].comboWindowMs).toBe(60000);
        });

        it("parseSettingsForm combo 不足 2 个音频报错", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "连杀",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFiles: ["only.mp3"],
                        playMode: "combo" as const,
                        comboWindowMs: "60000",
                        comboWindows: [],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("连杀或随机播放至少需要添加 2 个音频文件");
        });

        it("parseSettingsForm random 不足 2 个音频报错", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "随机",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFiles: ["only.mp3"],
                        playMode: "random" as const,
                        comboWindowMs: "60000",
                        comboWindows: [],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("连杀或随机播放至少需要添加 2 个音频文件");
        });

        it("parseSettingsForm 空音频文件报错", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "空文件",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFiles: [],
                        playMode: "single" as const,
                        comboWindowMs: "60000",
                        comboWindows: [],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("音频文件不能为空");
        });

        it("parseSettingsForm combo 窗口越界报错", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "连杀",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3", "b.mp3"],
                        playMode: "combo" as const,
                        comboWindowMs: "10",
                        comboWindows: ["", ""],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("连杀窗口时间必须在 100 到 600000 毫秒之间");
        });

        it("parseSettingsForm combo per-segment 窗口解析为等长数字数组（空段填默认）", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "连杀",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3", "b.mp3", "c.mp3"],
                        playMode: "combo" as const,
                        comboWindowMs: "60000",
                        comboWindows: ["500", "", "1000"],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            // 空段填卡片级默认 60000；数组与 audioFiles 等长
            expect(settings.cards[0].comboWindows).toEqual([500, 60000, 1000]);
            expect(settings.cards[0].comboWindowMs).toBe(60000);
        });

        it("parseSettingsForm combo per-segment 窗口越界报错", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "连杀",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioFiles: ["a.mp3", "b.mp3"],
                        playMode: "combo" as const,
                        comboWindowMs: "60000",
                        comboWindows: ["10", ""],
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("连杀窗口时间必须在 100 到 600000 毫秒之间");
        });

        it("getAudioCardFormErrors combo 文件不足标记 audioFiles 错误", () => {
            const errors = getAudioCardFormErrors({
                id: "c1",
                name: "连杀",
                enabled: true,
                triggerMode: "hotkey",
                hotkey: "Ctrl+F1",
                watchRegion: null,
                watchReferenceImagePath: "",
                watchMatchThreshold: "0.9",
                watchPollIntervalMs: "500",
                audioFiles: ["only.mp3"],
                playMode: "combo",
                comboWindowMs: "60000",
                comboWindows: [],
                volume: "0.8",
                cooldownMs: "1000",
                allowSimultaneous: false,
                colorProbes: [],
                colorMatchMode: "all",
                colorMatchMethod: "average",
            });
            expect(errors.audioFiles).toBe("连杀或随机播放至少需要 2 个音频文件");
        });
    });

    describe("colorMatchMethod", () => {
        it("cardToForm 透传 colorMatchMethod", () => {
            const settings = {
                audioEnabled: true,
                cards: [{
                    ...DEFAULT_AUDIO_CARD,
                    id: "c1",
                    name: "识色卡",
                    triggerMode: "colorWatch" as const,
                    colorMatchMethod: "anyPixel" as const,
                    colorProbes: [],
                    audioFiles: ["a.mp3"],
                }],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].colorMatchMethod).toBe("anyPixel");
        });

        it("cardToForm 缺省 colorMatchMethod 回退 average", () => {
            const settings = {
                audioEnabled: true,
                cards: [{
                    ...DEFAULT_AUDIO_CARD,
                    id: "c1",
                    name: "识色卡",
                    triggerMode: "colorWatch" as const,
                    audioFiles: ["a.mp3"],
                }],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].colorMatchMethod).toBe("average");
        });

        it("parseSettingsForm roundtrip 透传 colorMatchMethod", () => {
            const form = {
                audioEnabled: true,
                cards: [{
                    ...DEFAULT_AUDIO_CARD,
                    id: "c1",
                    name: "识色卡",
                    triggerMode: "colorWatch" as const,
                    colorProbes: [{
                        region: {x: 0, y: 0, width: 2, height: 2},
                        targetColor: "#ff0000",
                        tolerance: "10",
                    }],
                    colorMatchMode: "all" as const,
                    colorMatchMethod: "anyPixel" as const,
                    audioFiles: ["a.mp3"],
                    volume: "0.8",
                    cooldownMs: "1000",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    comboWindowMs: "60000",
                    comboWindows: [],
                    hotkey: "",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    playMode: "single" as const,
                    allowSimultaneous: false,
                    enabled: true,
                }],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].colorMatchMethod).toBe("anyPixel");
        });
    });
});
