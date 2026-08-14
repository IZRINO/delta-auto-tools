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

    it("提供四制作台与当天子弹校正并经二次确认提交，支持部分选中", () => {
        expect(pageSource).toContain("人工校正制作与子弹状态");
        expect(pageSource).toContain("立即到期");
        expect(pageSource).toContain("正在制作");
        expect(pageSource).toContain("空闲");
        // 每个制作台与子弹都有"不修改"选项
        expect(pageSource).toContain("不修改");
        expect(pageSource).toContain("special_ops_confirm_account_station_states");
        expect(pageSource).toContain("当天已成功兑换");
        expect(pageSource).toContain("当天未成功兑换");
        expect(pageSource).toContain("ammoTargets: correctionAmmoPayload");
        expect(pageSource).toContain("确认制作台与子弹状态并保存");
        expect(pageSource).toContain("人工校正提交失败");
        expect(pageSource).toContain("正在保存");
        // 新文案：选中项提交，未选中项保持不变
        expect(pageSource).toContain("选中项提交后原子恢复调度，未选中项保持不变");
        expect(pageSource).toContain("确认后将覆盖所选项的制作计时与子弹状态，并清除对应失败记录");
        // submit gate：至少一项选中即可提交（允许 correctionPayload 为空数组且 correctionAmmoPayload 非空，反之亦然）
        expect(pageSource).toContain("correctionPayload.length === 0 && correctionAmmoPayload.length === 0");
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

    it("限时商品使用原生颜色面板且不再绑定取色区域", () => {
        const html = renderToStaticMarkup(createElement(SpecialOpsPage));
        expect((html.match(/type="color"/g) ?? []).length).toBe(2);
        expect(pageSource).toContain("limitedColorToHex");
        expect(pageSource).toContain("parseLimitedColorHex");
        expect(pageSource).not.toContain("colorSampleRegions");
        expect(pageSource).not.toContain("samplingLimitedColor");
        expect(pageSource).not.toContain("sampleLimitedColor");
        expect(pageSource).not.toContain("special_ops_sample_limited_supply_color");
        expect(pageSource).not.toContain("取色区域");
        expect(pageSource).not.toContain("限时商品识色区域校准");
    });

    it("交易行业务配置位于默认与独立账号配置", () => {
        expect(pageSource).toContain("默认交易行购买");
        expect(pageSource).toContain("独立交易行配置");
        expect(pageSource).toContain("defaultMarket.purchaseCount");
        expect(pageSource).toContain("business.market?.purchaseCount");
        expect(pageSource).toContain("windowStartMinute");
        expect(pageSource).not.toContain("默认设定价格");
    });

    it("交易行默认与账号独立配置提供商品入口点击点校准", () => {
        expect(pageSource).toContain("defaultMarket.productPoint");
        expect(pageSource).toContain("business.market.productPoint");
        expect(pageSource).toContain('beginCalibration(activeEnvironment, "business.market.product"');
        expect(pageSource).toContain('beginCalibration(activeEnvironment, "business.market.product", account.id)');
    });

    it("交易行试运行使用 Rust 支持的真实单次尝试模式", () => {
        expect(pageSource).toContain('mode: "realSingleAttempt"');
        expect(pageSource).not.toContain('mode: "single"');
    });

    it("账号独立设置默认折叠", () => {
        expect(pageSource).toContain('<summary className="collapse-title">独立设置</summary>');
        expect(pageSource).toContain('<div className="collapse-content">');
    });

    it("子弹入口显示两段固定等待并按普通与赛季稳定分组", () => {
        expect(pageSource).toContain("点击军需处前等待");
        expect(pageSource).toContain("点击进入军需处前等待");
        expect(pageSource).not.toContain("研发部门等待（ms）");
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

    it("新增账号入口位于账号列表末尾", () => {
        const header = pageSource.indexOf('<h2 className="text-lg font-semibold">账号</h2>');
        const list = pageSource.indexOf("bootstrap.settings.accounts.map((account, index) => {", header);
        const trailingButton = pageSource.indexOf('<Button size="sm" onClick={addAccount}>', list);

        expect(header).toBeGreaterThanOrEqual(0);
        expect(list).toBeGreaterThan(header);
        expect(trailingButton).toBeGreaterThan(list);
    });

    it("时间轴按账号配置顺序显示两位账号序号", () => {
        expect(pageSource).toContain("const accountNumbers = new Map(");
        expect(pageSource).toContain("bootstrap.settings.accounts.map((account, index)");
        expect(pageSource).toContain('String(index + 1).padStart(2, "0")');
        expect(pageSource).toContain('accountNumbers.get(task.accountId) ?? "--"');
    });

    it("账号级失败可从账号卡片与时间轴确认已人工检查", () => {
        expect(pageSource).toContain("special_ops_confirm_account_manual_check");
        expect(pageSource).toContain("已人工检查");
        expect(pageSource).toContain("manualCheckRequired");
    });

    it("账号卡片与账号区标题提供一键恢复状态", () => {
        expect(pageSource).toContain("special_ops_restore_account_state");
        expect(pageSource).toContain("一键恢复状态");
        expect(pageSource).toContain("全部一键恢复");
        // 单账号传 id、批量传 null，后端按 Option<String> 分流。
        expect(pageSource).toContain("restoreAccountState(account.id)");
        expect(pageSource).toContain("restoreAccountState(null)");
        expect(pageSource).toContain("accountRestorable(account, currentDay)");
        expect(pageSource).toContain("anyAccountRestorable");
    });

    it("一键恢复按钮常驻显示，无可恢复项时 disabled 并说明原因", () => {
        // 之前按 accountRestorable 条件渲染，干净状态下整块消失 -> 用户「找不到一键恢复 UI」。
        expect(pageSource).not.toContain("accountRestorable(account) && <Button");
        expect(pageSource).not.toContain("accounts.some(accountRestorable) && <Button");
        expect(pageSource).toContain("disabled={!anyAccountRestorable || !isNativeShell}");
        expect(pageSource).toContain("disabled={!accountRestorable(account, currentDay) || !isNativeShell}");
        expect(pageSource).toContain("当前没有需要恢复的异常状态");
        expect(pageSource).toContain("当前账号没有需要恢复的异常状态");
    });

    it("限时商品检查完出栏，确认与重新检查入口都在账号人工校正面板", () => {
        // 检查完就出栏（任何终态），任务栏不再展示结果文案与确认按钮。
        // 结果与两个人工动作都在 CorrectionLimitedSupply（账号人工校正面板）。
        expect(pageSource).not.toContain("limitedOutcomeLabels");
        expect(pageSource).not.toContain("onAcknowledgeLimited");
        // 确认已移入面板，仍通过 special_ops_acknowledge_limited_supply 提交。
        expect(pageSource).toContain("special_ops_acknowledge_limited_supply");
        expect(pageSource).toContain("已查看高价值商品");
        // 确认只在高价值且未确认时出现；重新检查是通用入口。
        expect(pageSource).toContain("needsAcknowledge");
        expect(pageSource).toContain('outcome === "highValue" && !limited.acknowledged');
        // 重新检查在面板里直接可见，不再经任务栏中转。
        expect(pageSource).toContain("onAcknowledge={acknowledgeLimitedSupply}");
        // 任务栏不再有 onRecheckLimited / onAcknowledgeLimited prop。
        expect(pageSource).not.toContain("onRecheckLimited");
    });

    it("交易行任务栏展示购买进度与状态", () => {
        // 不渲染进度 -> 改了购买次数任务栏毫无变化，用户以为配置没生效。
        expect(pageSource).toContain("marketStatusLabels");
        expect(pageSource).toContain("已购买 {task.marketCompletedCount ?? 0}/{task.marketTargetCount ?? 0}");
    });

    it("限时商品重新检查入口在账号人工校正面板内", () => {
        // 入口挂在账号而非任务栏任务上：noHighValue / failed 周期在任务栏没有可挂的动作，
        // 而复位需要的只是 accountId + 账号自己的 cycleId。
        expect(pageSource).toContain("CorrectionLimitedSupply");
        expect(pageSource).toContain("special_ops_recheck_limited_supply");
        expect(pageSource).toContain("重新检查");
        expect(pageSource).toContain("account.limitedSupply");
        expect(pageSource).toContain("onRecheck(account.id, cycleId)");
        expect(pageSource).toContain("onRecheck={recheckLimitedSupply}");
        expect(pageSource).toContain("onAcknowledge={acknowledgeLimitedSupply}");
        expect(pageSource).toContain("correctionLimitedOutcomeLabels");
        // 未检查的周期不给复位入口；已检查且 cycleId 有效才启用。
        expect(pageSource).toContain("disabled={disabled || submitting !== null || !checked || !cycleId}");
    });

    it("账号级动作失败在按钮旁展示原因", () => {
        // 顶部横幅在账号列表里不可见 -> 点「已人工检查」报错看起来像「没反应」。
        expect(pageSource).toContain("accountActionError");
        expect(pageSource).toContain("accountActionError?.accountId === account.id");
        expect(pageSource).toContain("setAccountActionError({accountId, message: `人工检查提交失败：${String(cause)}`})");
    });

    it("自动暂停原因在页头显式展示", () => {
        expect(pageSource).toContain("bootstrap.settings.pausedReason");
        expect(pageSource).toContain("自动化已暂停：");
    });

    it("人工判定选正在制作时预填异常前剩余时间", () => {
        expect(pageSource).toContain("createStationRemainingTimeDraft");
        expect(pageSource).toContain("createCorrectionDraft(account.stations, bootstrap.nowMs)");
        expect(pageSource).toContain("留空继承异常前剩余时间");
        expect(pageSource).toContain("buildInlineStationCorrection");
        // 剩余时间不再硬编码 0/0，否则提交校验永远按 < 1 分钟拒绝。
        expect(pageSource).not.toContain('hours: "0", minutes: "0"');
    });

    it("时间轴单项判定入口不再只看定位失败", () => {
        expect(pageSource).toContain("timelineTaskAllowsInlineCorrection(task, station)");
        expect(pageSource).toContain("needsManualCorrection && !inlineCorrectable");
        expect(pageSource).not.toContain("if (!task.manualFailure) return null;");
    });
});
