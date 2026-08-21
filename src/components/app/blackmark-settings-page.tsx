import {AboutPanel} from "@/components/app/about-page";
import {BlackmarkPage} from "@/components/app/blackmark-page";
import {useTheme} from "@/hooks/use-theme";

export function BlackmarkSettingsPage() {
    const {uiWorld, setUiWorld, uiScheme, setUiScheme} = useTheme();

    return (
        <BlackmarkPage
            copy="界面世界与配色正交。黑标用夜航或日间；战地三套配色只在战地里改。"
            title="设置"
        >
            <section className="px-8 pb-16">
                <h2 className="text-2xl font-bold tracking-tight uppercase">界面世界</h2>
                <p className="bm-muted mt-2 max-w-[60ch] text-sm font-light">
                    切换立即换壳。overlay 窗不跟随。
                </p>
                <div className="mt-8 grid gap-px md:grid-cols-2">
                    <WorldCard
                        active={uiWorld === "blackmark"}
                        copy="默认。顶栏加底 dock。直角、发丝线、碳纤细扫光。"
                        onSelect={() => setUiWorld("blackmark")}
                        title="夜航黑标"
                    />
                    <WorldCard
                        active={uiWorld === "console"}
                        copy="左侧轨、铜缝凹槽、三套配色。"
                        onSelect={() => setUiWorld("console")}
                        title="战地控制台"
                    />
                </div>

                {uiWorld === "blackmark" ? (
                    <>
                        <h2 className="mt-16 text-2xl font-bold tracking-tight uppercase">色相</h2>
                        <p className="bm-muted mt-2 max-w-[60ch] text-sm font-light">
                            结构不变，只反转底与字。
                        </p>
                        <div className="mt-8 grid gap-px md:grid-cols-2">
                            <WorldCard
                                active={uiScheme === "night"}
                                copy="纯黑碳纤。夜间值机、游戏同屏时用这一面。"
                                onSelect={() => setUiScheme("night")}
                                title="夜航"
                            />
                            <WorldCard
                                active={uiScheme === "day"}
                                copy="同结构浅色。发丝线、直角、三色条位置不变。"
                                onSelect={() => setUiScheme("day")}
                                title="日间"
                            />
                        </div>
                    </>
                ) : null}

                <h2 className="mt-16 text-2xl font-bold tracking-tight uppercase">关于</h2>
                <div className="mt-8 border border-[var(--bm-hair)] bg-[var(--bm-surface)] px-6 py-6">
                    <AboutPanel active/>
                </div>
            </section>
        </BlackmarkPage>
    );
}

function WorldCard({
    active,
    copy,
    onSelect,
    title,
}: {
    active: boolean;
    copy: string;
    onSelect: () => void;
    title: string;
}) {
    return (
        <button
            className="bm-scheme-card"
            data-active={active}
            onClick={onSelect}
            type="button"
        >
            <div className="text-2xl font-bold tracking-tight uppercase">{title}</div>
            <p className="bm-copy mt-3 max-w-[36ch] text-sm font-light">{copy}</p>
        </button>
    );
}
