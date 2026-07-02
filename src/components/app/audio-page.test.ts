import {describe, expect, it} from "vitest";
import audioPageSource from "./audio-page.tsx?raw";

/**
 * 验证 audio-page 事件监听补全（VAL-DF-007, VAL-DF-008）。
 *
 * audio-page.tsx 必须订阅 AUDIO_EVENTS.hotkeyTriggered 和 AUDIO_EVENTS.regionMatched 事件，
 * 事件到达时提供视觉反馈，unmount 时正确 unlisten。
 */
describe("audio-page 事件监听", () => {
    it("订阅 hotkeyTriggered 事件", () => {
        expect(audioPageSource).toMatch(/AUDIO_EVENTS\.hotkeyTriggered/);
    });

    it("订阅 regionMatched 事件", () => {
        expect(audioPageSource).toMatch(/AUDIO_EVENTS\.regionMatched/);
    });

    it("hotkeyTriggered 回调中有视觉反馈（toast）", () => {
        // 事件回调中应触发 toast 通知
        const hotkeyBlock = audioPageSource.match(
            /listenEvent\(AUDIO_EVENTS\.hotkeyTriggered[^}]*\}[^}]*\}/s
        );
        expect(hotkeyBlock).not.toBeNull();
        expect(hotkeyBlock![0]).toMatch(/toast\./);
    });

    it("regionMatched 回调中有视觉反馈（toast）", () => {
        const regionBlock = audioPageSource.match(
            /listenEvent\(AUDIO_EVENTS\.regionMatched[^}]*\}[^}]*\}/s
        );
        expect(regionBlock).not.toBeNull();
        expect(regionBlock![0]).toMatch(/toast\./);
    });

    it("unmount 时 unlisten hotkeyTriggered", () => {
        expect(audioPageSource).toMatch(/unlistenHotkeyTriggered/);
    });

    it("unmount 时 unlisten regionMatched", () => {
        expect(audioPageSource).toMatch(/unlistenRegionMatched/);
    });
});
