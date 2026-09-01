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
    limitedSupply: "限时商品检查中",
    market: "交易行购买中",
    round: "多账号制作轮次中",
    stationWalkthrough: "多账号制作台更改中",
};

const RUN_PREPARING_DETAILS: Record<LoginRunKind, string> = {
    login: "正在准备登录试运行",
    navigation: "正在准备游戏内导航试运行",
    craft: "正在准备制作试运行",
    ammo: "正在准备子弹兑换试运行",
    limitedSupply: "正在准备限时商品试运行",
    market: "正在准备交易行试运行",
    round: "正在准备多账号制作轮次",
    stationWalkthrough: "正在准备多账号制作台更改",
};

function roundDetail(snapshot: LoginRunSnapshot): string {
    const progress = snapshot.roundProgress;
    if (!progress) return snapshot.message;
    const account = `账号 ${progress.accountIndex}/${progress.accountTotal} · QQ ${progress.qqAccount}`;
    if (snapshot.status === "waiting" && snapshot.message.includes("等待")) {
        return `${account} · ${snapshot.message}`;
    }
    if (!progress.stationKind) return account;
    return `${account} · ${STATION_LABELS[progress.stationKind]} ${progress.stationIndex}/${progress.stationTotal}`;
}

export function operationOverlayText(
    snapshot: LoginRunSnapshot,
    hotkey = DEFAULT_EMERGENCY_HOTKEY,
) {
    const countdownSeconds = snapshot.countdownSeconds;
    return {
        title: countdownSeconds === null ? RUN_TITLES[snapshot.runKind] : "即将占用键盘鼠标",
        detail: countdownSeconds === null
            ? snapshot.runKind === "round" ? roundDetail(snapshot) : snapshot.message
            : snapshot.message || `${countdownSeconds} 秒后执行当前步骤`,
        countdownSeconds,
        hotkey,
    };
}

export function OperationHud({
    title,
    detail,
    countdownSeconds,
    hotkey,
}: ReturnType<typeof operationOverlayText>) {
    return (
        <main className="relative flex h-dvh w-full flex-col overflow-hidden border border-white/15 bg-black/80 text-white backdrop-blur-[1px]">
            {countdownSeconds !== null ? (
                <div
                    key={countdownSeconds}
                    aria-hidden="true"
                    className="ops-fuse pointer-events-none absolute inset-x-0 top-0 h-0.5 origin-left bg-primary"
                />
            ) : null}
            <header className="px-4 pt-3">
                <h1 className="font-mono text-xs font-semibold tracking-[0.14em] text-white/70">{title}</h1>
            </header>
            <div aria-live="polite" className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-4">
                {countdownSeconds !== null ? (
                    <>
                        <p
                            key={countdownSeconds}
                            className="ops-digit font-mono text-7xl font-semibold tabular-nums leading-none"
                        >
                            {countdownSeconds}
                        </p>
                        <p className="max-w-[36ch] text-center text-xs leading-relaxed text-white/80">{detail}</p>
                    </>
                ) : (
                    <p className="max-w-[36ch] text-center text-sm leading-relaxed text-white/85">{detail}</p>
                )}
            </div>
            <footer className="border-t border-white/15 px-4 py-2">
                <p className="font-mono text-xs font-semibold text-white">紧急停止：{hotkey}</p>
            </footer>
        </main>
    );
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
    const runKind: LoginRunKind = requestedRunKind === "navigation" || requestedRunKind === "craft" || requestedRunKind === "ammo" || requestedRunKind === "limitedSupply" || requestedRunKind === "market" || requestedRunKind === "round"
        ? requestedRunKind
        : "login";
    const text = snapshot
        ? operationOverlayText(snapshot, hotkey)
        : {title: RUN_TITLES[runKind], detail: RUN_PREPARING_DETAILS[runKind], countdownSeconds: null, hotkey};
    return <OperationHud {...text}/>;
}
