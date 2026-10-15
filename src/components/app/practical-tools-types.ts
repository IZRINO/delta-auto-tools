import type {RegionRect} from "@/components/app/morse-types";

export type PracticalToolsSettings = {
    enabled: boolean;
    autoFullAutoEnabled: boolean;
    autoFullAutoToggleKey: string;
    fireKey: string;
    weaponKeys: [string, string, string, string, string];
    startDelayMs: number;
    weaponDetectionDelayMs: number;
    pressMs: number;
    intervalMs: number;
    autoRegion: RegionRect | null;
    matchThreshold: number;
    autoIndicatorOpacity: number;
    autoIndicatorRight: number;
    autoIndicatorBottom: number;
    bombProgressEnabled: boolean;
    installRegion: RegionRect | null;
    bombIconRegion: RegionRect | null;
    bombTargetColor: string;
    bombColorTolerance: number;
    bombDurationSeconds: number;
    warningSeconds: number;
    overlayX: number;
    overlayY: number;
    overlayWidth: number;
    overlayHeight: number;
    overlayOpacity: number;
};

export type PracticalToolsBootstrap = {settings: PracticalToolsSettings};

export function mergePracticalToolsRegions(
    current: PracticalToolsSettings | null,
    saved: PracticalToolsSettings,
): PracticalToolsSettings {
    return current ? {
        ...current,
        autoRegion: saved.autoRegion,
        installRegion: saved.installRegion,
        bombIconRegion: saved.bombIconRegion,
    } : saved;
}
