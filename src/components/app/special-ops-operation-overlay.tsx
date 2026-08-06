import {useEffect, useState} from "react";

import {STATION_LABELS, type LoginRunKind, type LoginRunSnapshot, type SpecialOpsBootstrap} from "@/components/app/special-ops-types";
import {invokeLogged as invoke} from "@/lib/logging";
import {SPECIAL_OPS_EVENTS} from "@/lib/tauri-events";
import {subscribeTauriEvent} from "@/lib/tauri-listener";

const DEFAULT_EMERGENCY_HOTKEY = "Ctrl+Shift+F12";

const RUN_TITLES: Record<LoginRunKind, string> = {
    login: "特勤处操作中",
    navigation: "游戏内导航试运行中",
    craft: "制作试运行中",
    ammo: "子弹兑换操作中",
    round: "多账号制作轮次中",
};

const RUN_PREPARING_DETAILS: Record<LoginRunKind, string> = {
    login: "正在准备登录试运行",
    navigation: "正在准备游戏内导航试运行",
    craft: "正在准备制作试运行",
    ammo: "正在准备子弹兑换试运行",
    round: "正在准备多账号制作轮次",
};

function roundDetail(snapshot: LoginRunSnapshot): string {
    const progress = snapshot.roundProgress;
    if (!progress) return snapshot.message;
    const account = `账号 ${progress.accountIndex}/${progress.accountTotal} · QQ ${progress.qqAccount}`;
    if (!progress.stationKind) return account;
    return `${account} · ${STATION_LABELS[progress.stationKind]} ${progress.stationIndex}/${progress.stationTotal}`;
}

export function operationOverlayText(
    snapshot: LoginRunSnapshot,
    hotkey = DEFAULT_EMERGENCY_HOTKEY,
) {
    return {
        title: snapshot.countdownSeconds === null ? RUN_TITLES[snapshot.runKind] : "即将占用键盘鼠标",
        detail: snapshot.countdownSeconds === null
            ? snapshot.runKind === "round" ? roundDetail(snapshot) : snapshot.message
            : `${snapshot.countdownSeconds} 秒后执行当前步骤`,
        hotkey,
    };
}

export async function loadOperationRunSnapshot(
    request: () => Promise<Pick<SpecialOpsBootstrap, "runSnapshot">>,
): Promise<LoginRunSnapshot | null> {
    return (await request()).runSnapshot;
}

export function SpecialOpsOperationOverlay() {
    const [snapshot, setSnapshot] = useState<LoginRunSnapshot | null>(null);

    useEffect(() => {
        let disposed = false;
        const unsubscribe = subscribeTauriEvent<LoginRunSnapshot>(
            SPECIAL_OPS_EVENTS.runChanged,
            (event) => setSnapshot(event.payload),
        );
        void loadOperationRunSnapshot(() => invoke<SpecialOpsBootstrap>("special_ops_get_bootstrap"))
            .then((initialSnapshot) => {
                if (!disposed) setSnapshot((current) => current ?? initialSnapshot);
            })
            .catch(() => undefined);
        return () => {
            disposed = true;
            unsubscribe();
        };
    }, []);

    const search = new URLSearchParams(window.location.search);
    const hotkey = search.get("emergencyHotkey")
        ?? DEFAULT_EMERGENCY_HOTKEY;
    const requestedRunKind = search.get("runKind");
    const runKind: LoginRunKind = requestedRunKind === "navigation" || requestedRunKind === "craft" || requestedRunKind === "ammo" || requestedRunKind === "round"
        ? requestedRunKind
        : "login";
    const text = snapshot
        ? operationOverlayText(snapshot, hotkey)
        : {title: RUN_TITLES[runKind], detail: RUN_PREPARING_DETAILS[runKind], hotkey};
    return (
        <main className="card card-border h-dvh w-full bg-base-200 text-base-content shadow-lg">
            <div className="card-body gap-2 p-4">
                <h1 className="card-title text-base">{text.title}</h1>
                <p className="text-sm text-base-content/70">{text.detail}</p>
                <span className="badge badge-warning badge-sm">紧急停止：{text.hotkey}</span>
            </div>
        </main>
    );
}
