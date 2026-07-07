import {describe, expect, it} from "vitest";
import {
    createEmptyRecognitionCard,
    generateCardId,
    getRecognitionCardFormErrors,
    mergeRecognitionWatchRegionsIntoForm,
    parseSettingsForm,
    settingsToForm,
} from "@/components/app/recognition-utils";
import {DEFAULT_RECOGNITION_CARD} from "@/components/app/recognition-types";

describe("recognition-utils", () => {
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
                        ...DEFAULT_RECOGNITION_CARD,
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

        it("migrates legacy audio fields into audio effect form", () => {
            const form = settingsToForm({
                recognitionEnabled: true,
                cards: [{
                    ...DEFAULT_RECOGNITION_CARD,
                    id: "c1",
                    name: "旧音频卡",
                    audioFiles: ["a.mp3"],
                    playMode: "single",
                    volume: 0.4,
                }],
            });

            expect(form.cards[0].audioEffectEnabled).toBe(true);
            expect(form.cards[0].audioFiles).toEqual(["a.mp3"]);
            expect(form.cards[0].volume).toBe("0.4");
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
            expect(settings.cards[0].effects?.audio?.volume).toBe(0.8);
            expect(settings.cards[0].cooldownMs).toBe(1000);
            expect(settings.cards[0].hotkey).toBe("Ctrl+F1");
            expect(settings.cards[0].effects?.audio?.allowSimultaneous).toBe(false);
        });

        it("saves hotkey effect without audio files", () => {
            const settings = parseSettingsForm({
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "按键效果",
                    enabled: true,
                    triggerMode: "hotkey",
                    hotkey: "Ctrl+F1",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    audioEffectEnabled: false,
                    hotkeyEffectEnabled: true,
                    effectHotkey: "F2",
                    clickEffectEnabled: false,
                    audioFiles: [],
                    playMode: "single",
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "1000",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all",
                    colorMatchMethod: "average",
                }],
            });

            expect(settings.cards[0].effects?.audio).toBeUndefined();
            expect(settings.cards[0].effects?.hotkey?.hotkey).toBe("F2");
        });

        it("parses hotkey effect steps with per-step delay", () => {
            const settings = parseSettingsForm({
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "按键序列",
                    enabled: true,
                    triggerMode: "hotkey",
                    hotkey: "Ctrl+F1",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    audioEffectEnabled: false,
                    hotkeyEffectEnabled: true,
                    effectHotkey: "F2",
                    hotkeyEffectSteps: [
                        {hotkey: "F2", delayMs: "0"},
                        {hotkey: "Ctrl+=", delayMs: "250"},
                    ],
                    clickEffectEnabled: false,
                    audioFiles: [],
                    playMode: "single",
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "1000",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all",
                    colorMatchMethod: "average",
                } as any],
            });

            expect(settings.cards[0].effects?.hotkey?.steps).toEqual([
                {hotkey: "F2", delayMs: 0},
                {hotkey: "Ctrl+=", delayMs: 250},
            ]);
        });

        it("writes timed activation for region watch cards", () => {
            const settings = parseSettingsForm({
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "限时识别",
                    enabled: true,
                    triggerMode: "regionWatch",
                    hotkey: "",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    activationMode: "timedHotkey",
                    activationHotkey: "Alt+F1",
                    activationDurationMs: "3000",
                    activationTriggerCount: "10",
                    audioEffectEnabled: false,
                    hotkeyEffectEnabled: true,
                    effectHotkey: "F2",
                    clickEffectEnabled: false,
                    audioFiles: [],
                    playMode: "single",
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "1000",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all",
                    colorMatchMethod: "average",
                }],
            });

            expect(settings.cards[0].activation).toEqual({
                mode: "timedHotkey",
                hotkey: "Alt+F1",
                durationMs: 3000,
                triggerCount: 10,
            });
        });

        it("hotkey 来源忽略 activation 配置", () => {
            const settings = parseSettingsForm({
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "快捷键直接触发",
                    enabled: true,
                    triggerMode: "hotkey",
                    hotkey: "Ctrl+F1",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    activationMode: "timedHotkey",
                    activationHotkey: "Alt+F1",
                    activationDurationMs: "3000",
                    activationTriggerCount: "3",
                    audioEffectEnabled: false,
                    hotkeyEffectEnabled: true,
                    effectHotkey: "F2",
                    clickEffectEnabled: false,
                    audioFiles: [],
                    playMode: "single",
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "1000",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all",
                    colorMatchMethod: "average",
                }],
            });

            expect(settings.cards[0].activation).toEqual({
                mode: "always",
                hotkey: null,
                durationMs: 10000,
                triggerCount: 1,
            });
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
            expect(id1).toMatch(/^recognition-/);
        });
    });

    describe("createEmptyRecognitionCard", () => {
        it("creates card with defaults and new id", () => {
            const card = createEmptyRecognitionCard();
            expect(card.name).toBe("");
            expect(card.effects?.audio?.volume).toBe(0.8);
            expect(card.watchMatchThreshold).toBe(0.75);
            expect(card.id).toMatch(/^recognition-/);
        });
    });

    describe("mergeRecognitionWatchRegionsIntoForm", () => {
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
            const merged = mergeRecognitionWatchRegionsIntoForm(current, {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_RECOGNITION_CARD,
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
                                targets: [{ color: "#c86432", tolerance: "40" }],
                                probeMatchMode: "any" as const,
                            },
                        ],
                        colorMatchMode: "all" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            // 后端：探针 region 已被框选提交（10,20,5x5）
            const merged = mergeRecognitionWatchRegionsIntoForm(current, {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_RECOGNITION_CARD,
                        id: "c1",
                        name: "识色",
                        triggerMode: "colorWatch",
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targets: [{ color: [200, 100, 50] as [number, number, number], tolerance: 40 }],
                                probeMatchMode: "any" as const,
                            },
                        ],
                        colorMatchMode: "all",
                    },
                ],
            });

            // region 被后端回写
            expect(merged.cards[0].colorProbes[0].region).toEqual({x: 10, y: 20, width: 5, height: 5});
            // 本地草稿（颜色/容差）保留，不被后端覆盖
            expect(merged.cards[0].colorProbes[0].targets[0].color).toBe("#c86432");
            expect(merged.cards[0].colorProbes[0].targets[0].tolerance).toBe("40");
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
            const merged = mergeRecognitionWatchRegionsIntoForm(current, {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_RECOGNITION_CARD,
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
                        ...DEFAULT_RECOGNITION_CARD,
                        id: "c1",
                        name: "识色",
                        triggerMode: "colorWatch" as const,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targets: [{ color: [200, 100, 50] as [number, number, number], tolerance: 40 }],
                                probeMatchMode: "any" as const,
                            },
                        ],
                        colorMatchMode: "any" as const,
                        colorMatchMethod: "average" as const,
                    },
                ],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].colorProbes).toHaveLength(1);
            expect(form.cards[0].colorProbes[0].targets[0].color).toBe("#c86432");
            expect(form.cards[0].colorProbes[0].targets[0].tolerance).toBe("40");
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
                                targets: [{ color: "#c86432", tolerance: "40" }],
                                probeMatchMode: "any" as const,
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
            expect(settings.cards[0].colorProbes[0].targets[0].color).toEqual([200, 100, 50]);
            expect(settings.cards[0].colorProbes[0].targets[0].tolerance).toBe(40);
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
                                targets: [{ color: "#c86432", tolerance: "300" }],
                                probeMatchMode: "any" as const,
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
                                targets: [{ color: "#c86432", tolerance: "40" }],
                                probeMatchMode: "any" as const,
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
            expect(settings.cards[0].colorProbes[0].targets[0].color).toEqual([200, 100, 50]);
            expect(settings.cards[0].colorProbes[0].targets[0].tolerance).toBe(40);
        });
    });

    describe("playMode / audioFiles", () => {
        it("settingsToForm 直传 audioFiles 数组与 playMode/comboWindowMs", () => {
            const settings = {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_RECOGNITION_CARD,
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
            expect(settings.cards[0].effects?.audio?.audioFiles).toEqual(["a.mp3"]);
            expect(settings.cards[0].effects?.audio?.playMode).toBe("single");
            expect(settings.cards[0].effects?.audio?.comboWindowMs).toBe(60000);
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

        it("parseSettingsForm 空音频文件拒绝音频效果", () => {
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
            expect(() => parseSettingsForm(form)).toThrow("请至少添加一个音频文件");
        });

        it("parseSettingsForm 允许自定义点击效果尚未框选区域", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "点击",
                        enabled: true,
                        triggerMode: "hotkey" as const,
                        hotkey: "Ctrl+F1",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.9",
                        watchPollIntervalMs: "500",
                        audioEffectEnabled: false,
                        hotkeyEffectEnabled: false,
                        clickEffectEnabled: true,
                        clickMode: "customRegion" as const,
                        clickCustomRegion: null,
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
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].effects?.click).toEqual({
                mode: "customRegion",
                customRegion: null,
                colorProbeIndex: null,
            });
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
            expect(settings.cards[0].effects?.audio?.comboWindows).toEqual([500, 60000, 1000]);
            expect(settings.cards[0].effects?.audio?.comboWindowMs).toBe(60000);
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

        it("getRecognitionCardFormErrors combo 文件不足标记 audioFiles 错误", () => {
            const errors = getRecognitionCardFormErrors({
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
                    ...DEFAULT_RECOGNITION_CARD,
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
                    ...DEFAULT_RECOGNITION_CARD,
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
                    ...DEFAULT_RECOGNITION_CARD,
                    id: "c1",
                    name: "识色卡",
                    triggerMode: "colorWatch" as const,
                    colorProbes: [{
                        region: {x: 0, y: 0, width: 2, height: 2},
                        targets: [{ color: "#ff0000", tolerance: "10" }],
                        probeMatchMode: "any" as const,
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

    // ---- T-9: cardToForm 边界测试 ----
    describe("cardToForm 边界测试", () => {
        it("空 cards 数组返回空 form", () => {
            const settings = {audioEnabled: false, cards: []};
            const form = settingsToForm(settings);
            expect(form.cards).toHaveLength(0);
        });

        it("卡片的 undefined/null 字段回退到默认值", () => {
            const settings = {
                audioEnabled: true,
                cards: [{
                    ...DEFAULT_RECOGNITION_CARD,
                    id: "c1",
                    name: "边界卡",
                    // hotkey 和 watchReferenceImagePath 在 cardToForm 中用 ?? "" 回退
                    // colorProbes 和 colorMatchMode/colorMatchMethod 也用 ?? 回退
                    hotkey: undefined as unknown as string,
                    watchReferenceImagePath: undefined as unknown as string,
                    colorProbes: undefined as unknown as [],
                    colorMatchMode: undefined as unknown as "all" | "any",
                    colorMatchMethod: undefined as unknown as "average" | "anyPixel",
                    playMode: undefined as unknown as "single" | "combo" | "random",
                    comboWindowMs: undefined as unknown as number,
                    comboWindows: undefined as unknown as number[],
                    allowSimultaneous: undefined as unknown as boolean,
                    audioFiles: undefined as unknown as string[],
                }],
            };
            const form = settingsToForm(settings as any);
            expect(form.cards[0].hotkey).toBe("");
            expect(form.cards[0].watchReferenceImagePath).toBe("");
            expect(form.cards[0].colorProbes).toHaveLength(0);
            expect(form.cards[0].colorMatchMode).toBe("all");
            expect(form.cards[0].colorMatchMethod).toBe("average");
            expect(form.cards[0].playMode).toBe("single");
            expect(form.cards[0].allowSimultaneous).toBe(false);
            expect(form.cards[0].audioFiles).toEqual([]);
        });

        it("极端数值：volume=0、cooldownMs=0、threshold=1", () => {
            const settings = {
                audioEnabled: true,
                cards: [{
                    ...DEFAULT_RECOGNITION_CARD,
                    id: "c1",
                    name: "极端值",
                    volume: 0,
                    cooldownMs: 0,
                    watchMatchThreshold: 1,
                    watchPollIntervalMs: 100,
                }],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].volume).toBe("0");
            expect(form.cards[0].cooldownMs).toBe("0");
            expect(form.cards[0].watchMatchThreshold).toBe("1");
            expect(form.cards[0].watchPollIntervalMs).toBe("100");
        });

        it("非法数值：volume=NaN、负数 cooldownMs", () => {
            const settings = {
                audioEnabled: true,
                cards: [{
                    ...DEFAULT_RECOGNITION_CARD,
                    id: "c1",
                    name: "非法值",
                    volume: NaN,
                    cooldownMs: -1,
                    watchMatchThreshold: -0.5,
                }],
            };
            const form = settingsToForm(settings);
            // settingsToForm 只做 string 转换，不做校验
            expect(form.cards[0].volume).toBe("NaN");
            expect(form.cards[0].cooldownMs).toBe("-1");
            expect(form.cards[0].watchMatchThreshold).toBe("-0.5");
        });

        it("parseSettingsForm 空 name 报错", () => {
            const form = {
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "",
                    enabled: true,
                    triggerMode: "hotkey" as const,
                    hotkey: "Ctrl+F1",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    audioFiles: ["a.mp3"],
                    playMode: "single" as const,
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "1000",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all" as const,
                    colorMatchMethod: "average" as const,
                }],
            };
            expect(() => parseSettingsForm(form)).toThrow("卡片名称不能为空");
        });

        it("parseSettingsForm 负 volume 报错", () => {
            const form = {
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "测试",
                    enabled: true,
                    triggerMode: "hotkey" as const,
                    hotkey: "Ctrl+F1",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    audioFiles: ["a.mp3"],
                    playMode: "single" as const,
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "-0.5",
                    cooldownMs: "1000",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all" as const,
                    colorMatchMethod: "average" as const,
                }],
            };
            expect(() => parseSettingsForm(form)).toThrow("音量必须在 0 到 1 之间");
        });

        it("parseSettingsForm NaN cooldownMs 报错", () => {
            const form = {
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "测试",
                    enabled: true,
                    triggerMode: "hotkey" as const,
                    hotkey: "",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    audioFiles: ["a.mp3"],
                    playMode: "single" as const,
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "abc",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all" as const,
                    colorMatchMethod: "average" as const,
                }],
            };
            expect(() => parseSettingsForm(form)).toThrow("冷却时间必须在 0 到 60000 毫秒之间");
        });

        it("parseSettingsForm 负 cooldownMs 报错", () => {
            const form = {
                audioEnabled: true,
                cards: [{
                    id: "c1",
                    name: "测试",
                    enabled: true,
                    triggerMode: "hotkey" as const,
                    hotkey: "",
                    watchRegion: null,
                    watchReferenceImagePath: "",
                    watchMatchThreshold: "0.75",
                    watchPollIntervalMs: "500",
                    audioFiles: ["a.mp3"],
                    playMode: "single" as const,
                    comboWindowMs: "60000",
                    comboWindows: [],
                    volume: "0.8",
                    cooldownMs: "-100",
                    allowSimultaneous: false,
                    colorProbes: [],
                    colorMatchMode: "all" as const,
                    colorMatchMethod: "average" as const,
                }],
            };
            expect(() => parseSettingsForm(form)).toThrow("冷却时间必须在 0 到 60000 毫秒之间");
        });
    });
});
