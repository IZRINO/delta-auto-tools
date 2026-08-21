import {type ReactNode, StrictMode, useEffect, useState} from "react";
import {createRoot} from "react-dom/client";
import {
    RiPauseLine,
    RiPlayLine,
    RiRefreshLine,
    RiShutDownLine,
} from "@remixicon/react";

import "./blackmark-demo.css";

const tools = [
    {id: "favorites", label: "收藏"},
    {id: "timer", label: "计时器"},
    {id: "counter", label: "计数器"},
    {id: "rapidfire", label: "连发器"},
    {id: "strategy", label: "攻略"},
    {id: "recognition", label: "识别"},
    {id: "privacy", label: "息屏"},
    {id: "specialOps", label: "特勤处"},
    {id: "morse", label: "摩斯"},
] as const;

type ToolId = (typeof tools)[number]["id"];
type PaneId = ToolId | "settings";
type Scheme = "night" | "day";

const timeline = [
    {when: "现在", account: "账号 · 7891", task: "制作台 2 收取", status: "到期", state: "due"},
    {when: "2 分钟", account: "账号 · 7891", task: "子弹兑换 5.45 BT", status: "待执行", state: "next"},
    {when: "18 分钟", account: "账号 · 3340", task: "制作台 1 收取", status: "制作中", state: "run"},
    {when: "人工", account: "账号 · 1024", task: "需人工检查", status: "失败", state: "fail"},
    {when: "02:00", account: "账号 · 7891", task: "交易行购买 0/3", status: "窗口未开", state: "wait"},
] as const;

function paneLabel(pane: PaneId): string {
    if (pane === "settings") return "设置";
    return tools.find((item) => item.id === pane)?.label ?? "";
}

function paneCopy(pane: PaneId): string {
    if (pane === "specialOps") return "调度已暂停。下一件到期业务是制作台 2，然后把子弹兑换并进同一轮。";
    if (pane === "settings") return "只改这张演示页。夜航是主套，日间是对应浅色，确认后再接到真设置。";
    return "这是黑标语法套在工具页上的样子。确认后再接到设置切换，现行战地控制台不动。";
}

function BlackmarkDemo() {
    const [pane, setPane] = useState<PaneId>("specialOps");
    const [paused, setPaused] = useState(true);
    const [globalOn, setGlobalOn] = useState(true);
    const [scheme, setScheme] = useState<Scheme>("night");
    const activeLabel = paneLabel(pane);

    useEffect(() => {
        document.documentElement.dataset.scheme = scheme;
        return () => {
            delete document.documentElement.dataset.scheme;
        };
    }, [scheme]);

    return (
        <div className="bm-stage flex h-dvh flex-col" data-scheme={scheme}>
            <div className="bm-content flex min-h-0 flex-1 flex-col">
                <header className="navbar bm-header h-16 min-h-16 px-5">
                    <div className="navbar-start min-w-0 gap-3">
                        <span className="bm-stripe-v h-8 w-1 shrink-0" aria-hidden="true"/>
                        <p className="truncate text-sm font-bold uppercase tracking-[0.18em]">
                            三角洲行动工具
                        </p>
                        <span className="bm-muted hidden text-xs font-light tracking-wide lg:inline">
                            黑标演示 · 不写入设置
                        </span>
                    </div>
                    <div className="navbar-end gap-4">
                        <span className="bm-copy text-xs font-light tracking-[0.12em]">档案 A</span>
                        <label className="flex items-center gap-2 text-xs font-bold tracking-[0.12em] uppercase">
                            <RiShutDownLine className="size-3.5" aria-hidden="true"/>
                            {globalOn ? "全局开" : "全局关"}
                            <input
                                aria-label="全局总开关"
                                checked={globalOn}
                                className="toggle toggle-sm"
                                onChange={(event) => setGlobalOn(event.target.checked)}
                                type="checkbox"
                            />
                        </label>
                    </div>
                </header>
                <div className="bm-stripe h-1 w-full" aria-hidden="true"/>

                <main className="min-h-0 flex-1 overflow-y-auto pb-28">
                    <section className="relative px-8 pb-10 pt-12">
                        <span className="bm-watermark" aria-hidden="true" key={`mark-${pane}`}>
                            {activeLabel}
                        </span>
                        <div className="relative flex flex-wrap items-end justify-between gap-6">
                            <div>
                                <h1
                                    className="bm-title max-w-[12ch] text-[3.5rem] leading-none font-bold tracking-tight uppercase"
                                    key={`title-${pane}`}
                                >
                                    {activeLabel}
                                </h1>
                                <p className="bm-copy mt-4 max-w-[46ch] text-base leading-relaxed font-light">
                                    {paneCopy(pane)}
                                </p>
                            </div>
                            {pane === "settings" ? null : (
                                <div className="flex flex-wrap items-center gap-3">
                                    <button
                                        className="bm-btn inline-flex items-center gap-2"
                                        onClick={() => setPaused((value) => !value)}
                                        type="button"
                                    >
                                        {paused
                                            ? <RiPlayLine className="size-4" aria-hidden="true"/>
                                            : <RiPauseLine className="size-4" aria-hidden="true"/>}
                                        {paused ? "继续" : "暂停"}
                                    </button>
                                    <button className="bm-btn-ghost inline-flex items-center gap-2" type="button">
                                        <RiRefreshLine className="size-4" aria-hidden="true"/>
                                        刷新
                                    </button>
                                </div>
                            )}
                        </div>
                        <div className="bm-stripe bm-stripe-draw mt-8 h-1 w-28" key={`stripe-${pane}`} aria-hidden="true"/>
                    </section>

                    {pane === "specialOps" ? <SpecialOpsBody key={pane}/> : null}
                    {pane === "settings" ? <SettingsBody scheme={scheme} onScheme={setScheme}/> : null}
                    {pane !== "specialOps" && pane !== "settings" ? <OtherToolBody key={pane} label={activeLabel}/> : null}
                </main>

                <nav aria-label="工具" className="dock bm-dock">
                    {tools.map((item) => {
                        const active = item.id === pane;
                        return (
                            <button
                                aria-current={active ? "page" : undefined}
                                aria-label={item.label}
                                className={active ? "dock-active" : undefined}
                                key={item.id}
                                onClick={() => setPane(item.id)}
                                type="button"
                            >
                                <ToolGlyph id={item.id}/>
                                <span className="dock-label">{item.label}</span>
                            </button>
                        );
                    })}
                    <span className="bm-dock-rule" aria-hidden="true"/>
                    <button
                        aria-current={pane === "settings" ? "page" : undefined}
                        aria-label="设置"
                        className={pane === "settings" ? "dock-active" : undefined}
                        onClick={() => setPane("settings")}
                        type="button"
                    >
                        <ToolGlyph id="settings"/>
                        <span className="dock-label">设置</span>
                    </button>
                </nav>
            </div>
        </div>
    );
}

function glyph(children: ReactNode) {
    return (
        <svg aria-hidden="true" fill="none" height="22" viewBox="0 0 24 24" width="22">
            {children}
        </svg>
    );
}

const stroke = {
    stroke: "currentColor",
    strokeLinecap: "square" as const,
    strokeLinejoin: "miter" as const,
    strokeWidth: 1.75,
};

function ToolGlyph({id}: {id: PaneId}) {
    switch (id) {
        case "favorites":
            return glyph(
                <>
                    <path d="M12 3.5 13.8 10.2 20.5 12 13.8 13.8 12 20.5 10.2 13.8 3.5 12 10.2 10.2Z" {...stroke}/>
                    <path d="M12 8.5v7M8.5 12h7" {...stroke}/>
                </>,
            );
        case "timer":
            return glyph(
                <>
                    <path d="M8 4.5h8" {...stroke}/>
                    <path d="M12 4.5v2.2" {...stroke}/>
                    <path d="M12 8.2v4.3l3.2 2.1" {...stroke}/>
                    <path d="M19.2 12a7.2 7.2 0 1 1-2.1-5.1" {...stroke}/>
                </>,
            );
        case "counter":
            return glyph(
                <>
                    <path d="M5 18V9" {...stroke}/>
                    <path d="M10.5 18V6" {...stroke}/>
                    <path d="M16 18v-7" {...stroke}/>
                    <path d="M19.5 6.5v4M17.5 8.5h4" {...stroke}/>
                </>,
            );
        case "rapidfire":
            return glyph(
                <>
                    <path d="M4.5 16.5 9 8.5l3.2 5.2 3.3-7.2 4 10" {...stroke}/>
                    <path d="M4 19.5h16" {...stroke}/>
                </>,
            );
        case "strategy":
            return glyph(
                <>
                    <path d="M4.5 7.5h11.5v12H4.5z" {...stroke}/>
                    <path d="M16 10.5h3.5v9.5H8.5v-3" {...stroke}/>
                    <path d="M4.5 12.5h11.5" {...stroke}/>
                    <path d="M10.2 7.5 13.8 19.5" {...stroke}/>
                </>,
            );
        case "recognition":
            return glyph(
                <>
                    <path d="M12 12.5v.01" {...stroke} strokeWidth={2.4}/>
                    <path d="M8.2 16.2a5.5 5.5 0 1 1 7.6 0" {...stroke}/>
                    <path d="M5.4 18.6a9 9 0 1 1 13.2 0" {...stroke}/>
                </>,
            );
        case "privacy":
            return glyph(
                <>
                    <path d="M3.5 12h17" {...stroke}/>
                    <path d="M6 8.5 12 5.5 18 8.5" {...stroke}/>
                    <path d="M6 15.5 12 18.5 18 15.5" {...stroke}/>
                </>,
            );
        case "specialOps":
            return glyph(
                <>
                    <path d="M5 7.2 12 4.2 19 7.2v6.2c0 3.4-2.8 5.7-7 7.4-4.2-1.7-7-4-7-7.4z" {...stroke}/>
                    <path d="M12 9.4v5" {...stroke}/>
                    <path d="M9.5 11.9h5" {...stroke}/>
                </>,
            );
        case "morse":
            return glyph(
                <>
                    <rect fill="currentColor" height="3.2" width="3.2" x="3.2" y="10.4"/>
                    <rect fill="currentColor" height="3.2" width="3.2" x="8.2" y="10.4"/>
                    <rect fill="currentColor" height="3.2" width="3.2" x="13.2" y="10.4"/>
                    <rect fill="currentColor" height="3.2" width="4.4" x="18.2" y="10.4"/>
                </>,
            );
        case "settings":
            return glyph(
                <>
                    <path d="M9 4.2h6v2.4h4.2v4.2H21.5v2.4H19.2v4.2H15V19.8H9v-2.4H4.8v-4.2H2.5v-2.4H4.8V6.6H9z" {...stroke}/>
                    <path d="M9.6 9.6h4.8v4.8H9.6z" {...stroke}/>
                </>,
            );
    }
}

function SpecialOpsBody() {
    return (
        <>
            <section className="bm-spec-grid grid gap-px sm:grid-cols-2 xl:grid-cols-4">
                <article className="bm-spec">
                    <div className="value bm-readout">00:08:12</div>
                    <div className="label">制作台 2 剩余</div>
                </article>
                <article className="bm-spec">
                    <div className="value">2 分钟</div>
                    <div className="label">下一任务 子弹兑换</div>
                </article>
                <article className="bm-spec">
                    <div className="value bm-readout">3</div>
                    <div className="label">启用账号</div>
                </article>
                <article className="bm-spec">
                    <div className="value text-warning">人工</div>
                    <div className="label">账号 1024 需检查</div>
                </article>
            </section>

            <section className="px-8 py-16">
                <h2 className="text-2xl font-bold tracking-tight uppercase">24 小时时间轴</h2>
                <p className="bm-muted mt-2 max-w-[60ch] text-sm font-light">
                    到期按账号分桶，交易行排最后。失败行左侧交叉切口，不用第二套强调色铺底。
                </p>
                <div className="bm-table-shell mt-8 overflow-x-auto">
                    <table className="bm-table">
                        <thead>
                        <tr>
                            <th>时间</th>
                            <th>账号</th>
                            <th>任务</th>
                            <th>状态</th>
                        </tr>
                        </thead>
                        <tbody>
                        {timeline.map((row) => (
                            <tr data-state={row.state} key={`${row.when}-${row.task}`}>
                                <td className="strong bm-readout">{row.when}</td>
                                <td>{row.account}</td>
                                <td className="strong">{row.task}</td>
                                <td>{row.status}</td>
                            </tr>
                        ))}
                        </tbody>
                    </table>
                </div>
            </section>
        </>
    );
}

function SettingsBody({scheme, onScheme}: {scheme: Scheme; onScheme: (next: Scheme) => void}) {
    return (
        <section className="px-8 pb-16">
            <div className="grid gap-px md:grid-cols-2">
                <button
                    className="bm-scheme-card"
                    data-active={scheme === "night"}
                    onClick={() => onScheme("night")}
                    type="button"
                >
                    <div className="text-2xl font-bold tracking-tight uppercase">夜航</div>
                    <p className="bm-copy mt-3 max-w-[36ch] text-sm font-light">
                        纯黑碳纤。主套。夜间值机、游戏同屏时用这一面。
                    </p>
                </button>
                <button
                    className="bm-scheme-card"
                    data-active={scheme === "day"}
                    onClick={() => onScheme("day")}
                    type="button"
                >
                    <div className="text-2xl font-bold tracking-tight uppercase">日间</div>
                    <p className="bm-copy mt-3 max-w-[36ch] text-sm font-light">
                        同结构浅色。发丝线、直角、三色条位置不变，只反转底与字。
                    </p>
                </button>
            </div>
        </section>
    );
}

function OtherToolBody({label}: {label: string}) {
    return (
        <section className="px-8 py-6">
            <div className="bm-spec-grid grid gap-px md:grid-cols-2">
                {[1, 2, 3, 4].map((index) => (
                    <article className="bm-spec" key={index}>
                        <div className="flex items-end justify-between gap-4">
                            <div>
                                <div className="value bm-readout">{index === 1 ? "04:59.0" : "IDLE"}</div>
                                <div className="label">{label} {String(index).padStart(2, "0")}</div>
                            </div>
                            <button className="bm-btn-ghost h-10 px-5" type="button">
                                {index === 1 ? "停止" : "启动"}
                            </button>
                        </div>
                    </article>
                ))}
            </div>
        </section>
    );
}

createRoot(document.getElementById("blackmark-demo-root") as HTMLElement).render(
    <StrictMode>
        <BlackmarkDemo/>
    </StrictMode>,
);
