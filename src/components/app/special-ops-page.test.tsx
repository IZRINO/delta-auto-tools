import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {describe, expect, it} from "vitest";

import pageSource from "./special-ops-page.tsx?raw";
import calibrationOverlaySource from "./special-ops-calibration-overlay.tsx?raw";
import utilsSource from "./special-ops-utils.ts?raw";
import {SpecialOpsPage} from "@/components/app/special-ops-page";

describe("SpecialOpsPage 登录试运行配置", () => {
    it("制作台只保留时长配置", () => {
        expect(pageSource).not.toContain('placeholder="制作物品"');
        expect(pageSource).toContain("station.durationMinutes / 60");
        expect(pageSource).toContain("station.durationMinutes % 60");
    });

    it("提供独立游戏内导航试运行", () => {
        expect(pageSource).toContain("special_ops_start_navigation_trial");
        expect(pageSource).toContain("special_ops_start_craft_trial");
        expect(pageSource).toContain("special_ops_start_craft_batch_trial");
        expect(pageSource).toContain("special_ops_start_ammo_trial");
        expect(pageSource).toContain("当前账号结束后暂停");
        expect(pageSource).toContain("制作试运行目标");
        expect(pageSource).not.toContain('select className="select select-sm join-item"');
        expect(pageSource).toContain("游戏内导航试运行");
        expect(pageSource).toContain("当前账号四制作台批处理试运行");
        expect(pageSource).toContain("子弹兑换试运行");
    });

    it("不提供手动到期轮次入口", () => {
        expect(pageSource).not.toContain("special_ops_start_due_round");
        expect(pageSource).not.toContain("开始当前到期轮次");
        expect(pageSource).not.toContain("开始多账号子弹轮次");
    });

    it("在导航校准步骤旁配置四段固定等待时间", () => {
        expect(pageSource).toContain("点击烽火地带前等待");
        expect(pageSource).toContain("Space 前等待");
        expect(pageSource).toContain("Tab 前等待");
        expect(pageSource).toContain("点击前等待");
        expect(pageSource).toContain("navigationSpaceDelayMs");
        expect(pageSource).toContain("navigationTabDelayMs");
        expect(pageSource).toContain("navigationSpecialOpsDelayMs");
        expect(pageSource).toContain("navigationBeaconDelayMs");
        expect(pageSource).toContain("parseNavigationDelayMs");
    });

    it("显示可执行文件、紧急热键与单账号试运行边界", () => {
        const html = renderToStaticMarkup(createElement(SpecialOpsPage));

        expect(html).toContain("WeGame 可执行文件");
        expect(html).toContain("游戏可执行文件");
        expect(html).toContain("录制紧急停止热键");
        expect(html).toContain("可单独测试登录或从当前游戏进入四制作台页面");
        expect(html).toContain("先点击“继续”解除暂停");
        expect(html).not.toContain("开始当前到期轮次");
        expect(html).toContain("所有必需模板必须先测试通过");
        expect(html).toContain('class="card card-border');
        expect(html).toContain('class="select');
        expect(html).toContain('role="alert"');
    });

    it("active run 清理完成前锁定设置、校准与新试运行", () => {
        expect(pageSource).toContain("hasActiveSpecialOpsRun(runSnapshot)");
        expect(pageSource).toContain("if (hasActiveSpecialOpsRun(bootstrapRef.current.runSnapshot)) return;");
        expect(pageSource).toContain("if (!isNativeShell || controlsLocked) return;");
        expect(pageSource).toContain('disabled={!hasActiveRun || runSnapshot?.status === "stopping"}');
        expect(pageSource).toContain('<fieldset disabled={controlsLocked} className="contents">');
    });

    it("继续请求未完成时锁定新试运行", () => {
        expect(pageSource).toContain("const [pauseTransition, setPauseTransition] = useState(false);");
        expect(pageSource).toContain("const controlsLocked = hasActiveRun || pauseTransition;");
        expect(pageSource).toContain("if (!isNativeShell || controlsLocked) return;");
        expect(pageSource).toContain('disabled={!isNativeShell || !selectedAccountId || controlsLocked}');
        expect(pageSource).toContain('pauseTransition ? "正在继续"');
    });

    it("校准测试失败也显示在校准区域", () => {
        expect(pageSource).toContain("setCalibrationTestResult(`${target.label}：测试失败：${String(cause)}`)");
    });

    it("制作固定探测只显示仍参与流程的校准与三段等待", () => {
        expect(pageSource).not.toContain("制作中共享参考图");
        expect(pageSource).not.toContain("craftInProgressReferenceImagePath");
        expect(pageSource).not.toContain("craftInProgressMatchThreshold");
        expect(pageSource).toContain("craftSpaceDelayMs");
        expect(pageSource).toContain("craftReopenDelayMs");
        expect(pageSource).toContain("craftConfirmPinnedDelayMs");
        expect(pageSource).toContain("收取点击后按 Space 等待");
        expect(pageSource).toContain("Space 后再次点击制作台等待");
        expect(pageSource).toContain("再次点击后确认置顶等待");
    });

    it("没有未保存修改时 flush 不重复保存旧草稿", () => {
        expect(pageSource).toContain("if (!settingsDirtyRef.current) return bootstrapRef.current;");
    });

    it("提供四制作台与当天子弹原子校正并经二次确认提交", () => {
        expect(pageSource).toContain("人工校正制作与子弹状态");
        expect(pageSource).toContain("立即到期");
        expect(pageSource).toContain("正在制作");
        expect(pageSource).toContain("空闲");
        expect(pageSource).toContain("special_ops_confirm_account_station_states");
        expect(pageSource).toContain("当天已成功兑换");
        expect(pageSource).toContain("当天未成功兑换");
        expect(pageSource).toContain("ammoTargets: correctionAmmoPayload");
        expect(pageSource).toContain("确认制作台与子弹状态并保存");
        expect(pageSource).toContain("人工校正提交失败");
        expect(pageSource).toContain("正在保存");
    });

    it("独立设置提供四制作台账号级制作物品选择点击点", () => {
        expect(pageSource).toContain("账号级制作物品选择点击点");
        expect(pageSource).toContain("business.recipePoints");
        expect(pageSource).toContain("`craft.recipe.${station.kind}`, account.id");
        expect(calibrationOverlaySource).toContain('params.get("account_id")');
        expect(calibrationOverlaySource).toContain("accountId");
    });

    it("默认配置提供子弹兑换顺序编辑器", () => {
        expect(pageSource).toContain("默认子弹兑换顺序");
        expect(pageSource).toContain("defaultBusinessConfig.ammoTargets");
        expect(pageSource).toContain("AmmoTargetEditor");
        expect(pageSource).toContain('<details className="collapse collapse-arrow">');
        expect(pageSource).toContain('<summary className="collapse-title">默认子弹兑换顺序</summary>');
    });

    it("账号独立设置默认折叠", () => {
        expect(pageSource).toContain('<summary className="collapse-title">独立设置</summary>');
        expect(pageSource).toContain('<div className="collapse-content">');
    });

    it("子弹入口显示两段固定等待并按普通与赛季稳定分组", () => {
        expect(pageSource).toContain("点击军需处前等待");
        expect(pageSource).toContain("点击战术部门前等待");
        expect(pageSource).toContain("ammoSupplyDelayMs");
        expect(pageSource).toContain("ammoTacticalDelayMs");
        expect(pageSource).toContain("changeAmmoTargetSeasonal");
        expect(pageSource).toContain("moveAmmoTargetWithinGroup");
        expect(pageSource).toContain("insertNormalAmmoTarget");
    });

    it("默认与独立制作台提供制作物品备注", () => {
        expect(pageSource).toContain("制作物品备注");
        expect(pageSource).toContain("recipeNote");
        expect(pageSource).toContain("updateDefaultStation(station, {recipeNote");
        expect(pageSource).toContain("updateIndependentStation(account, station, {recipeNote");
    });

    it("子弹目标使用备注、指定点击点和顶部绝对滚轮位置", () => {
        expect(pageSource).toContain("子弹备注");
        expect(pageSource).toContain("指定点击点");
        expect(pageSource).toContain("A/D 重置后向下滚动次数（0 表示不滚动）");
        expect(pageSource).toContain("游戏内模板测试将在 3 秒后切换到游戏窗口");
        expect(pageSource).not.toContain("兑换后滚轮方向");
        expect(pageSource).not.toContain("兑换后滚轮次数");
        expect(pageSource).toContain("business.ammo.");
    });

    it("展示滚动未来二十四小时时间轴且不提供拖动改期", () => {
        expect(pageSource).toContain("未来 24 小时任务");
        expect(pageSource).toContain("groupTimelineTasks");
        expect(pageSource).toContain("buildTimelineHourSlots");
        expect(pageSource).toContain("0 分钟后");
        expect(pageSource).toContain("setTimelineNowMs(Date.now())");
        expect(pageSource).toContain("60_000");
        expect(pageSource).not.toContain("draggable=");
    });

    it("时间轴按失败任务提供单项人工判定", () => {
        expect(pageSource).toContain("task.manualFailure");
        expect(pageSource).toContain("请在账号页处理");
        expect(pageSource).toContain("submitting");
        expect(pageSource).toContain("error");
        expect(pageSource).toContain("立即到期");
        expect(pageSource).toContain("正在制作");
        expect(pageSource).toContain("空闲中");
        expect(pageSource).toContain("已兑换");
        expect(pageSource).toContain("未兑换");
        expect(pageSource).toContain("special_ops_confirm_station_state");
        expect(pageSource).toContain("special_ops_confirm_ammo_state");
        expect(pageSource).not.toContain("onOpenCorrection(task.accountId)");
        expect(utilsSource).toContain("Math.ceil");
    });

    it("账号页完整人工校正入口保持不变", () => {
        expect(pageSource).toContain("人工校正制作与子弹状态");
        expect(pageSource).toContain("special_ops_confirm_account_station_states");
    });
});
