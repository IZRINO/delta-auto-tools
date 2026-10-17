import {describe, expect, it} from "vitest";
import {mergePracticalToolsRegions, type PracticalToolsSettings} from "./practical-tools-types";

const saved: PracticalToolsSettings = {
    enabled: false, autoFullAutoEnabled: false, autoFullAutoToggleKey: "F6", fireKey: "MouseLeft", weaponKeys: ["1", "2", "3", "4", "5"],
    startDelayMs: 80, weaponDetectionDelayMs: 100, pressMs: 30, intervalMs: 30,
    autoRegion: {x: 10, y: 20, width: 100, height: 40}, matchThreshold: 0.75, autoIndicatorOpacity: 0.9, autoIndicatorRight: 28, autoIndicatorBottom: 28,
    bombProgressEnabled: false, installRegion: null, bombIconRegion: null, bombTargetColor: "#F5C400", bombColorTolerance: 55,
    bombDurationSeconds: 30, warningSeconds: 5, showRemainingTime: true, overlayX: 0, overlayY: 0,
    overlayWidth: 420, overlayHeight: 28, overlayOpacity: 0.85,
};

describe("实用小工具框选状态同步", () => {
    it("更新已保存区域，同时保留尚在编辑的开关和参数", () => {
        const draft = {...saved, enabled: true, autoFullAutoEnabled: true, fireKey: "A", autoRegion: null};
        expect(mergePracticalToolsRegions(draft, saved)).toEqual({...draft, autoRegion: saved.autoRegion});
        expect(draft.autoRegion).toBeNull();
    });

    it("同时同步两个爆破区域，支持首次加载", () => {
        const next = {...saved, installRegion: saved.autoRegion, bombIconRegion: saved.autoRegion};
        expect(mergePracticalToolsRegions(saved, next)).toEqual(next);
        expect(mergePracticalToolsRegions(null, next)).toEqual(next);
    });
});
