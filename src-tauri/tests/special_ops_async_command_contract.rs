#[test]
fn set_paused_command_runs_on_async_runtime() {
    let source = include_str!("../src/special_ops/mod.rs");

    assert!(
        source.contains("pub async fn special_ops_set_paused("),
        "special_ops_set_paused 必须保持 async，避免唤醒 scheduler 时占用 Tauri 主事件循环"
    );
}

#[test]
fn set_station_walkthrough_command_is_async_and_registered() {
    let source = include_str!("../src/special_ops/mod.rs");
    let lib = include_str!("../src/lib.rs");
    assert!(
        source.contains("pub async fn special_ops_set_station_walkthrough("),
        "special_ops_set_station_walkthrough 必须保持 async"
    );
    assert!(
        lib.contains("special_ops_set_station_walkthrough"),
        "缺少 Tauri command 注册：special_ops_set_station_walkthrough"
    );
}

#[test]
fn set_station_walkthrough_registers_hotkeys_before_persisting() {
    let source = include_str!("../src/special_ops/mod.rs");
    let start = source
        .find("pub async fn special_ops_set_station_walkthrough")
        .expect("缺少 special_ops_set_station_walkthrough");
    let end = source[start..]
        .find("\npub async fn special_ops_start_login_trial")
        .map(|offset| start + offset)
        .expect("set_station_walkthrough 后应紧跟 start_login_trial");
    let body = &source[start..end];
    let enable_start = body.find("if enabled {").expect("缺少开启分支");
    let enable_end = body[enable_start..]
        .find("} else {")
        .map(|offset| enable_start + offset)
        .expect("缺少关闭分支");
    let enable = &body[enable_start..enable_end];
    let register = enable
        .find("register_walkthrough_hotkeys(")
        .expect("开启走查必须先登记热键");
    let persist = enable
        .find("with_expected_revision_change")
        .expect("开启走查仍需落盘");
    assert!(
        register < persist,
        "走查热键必须在 revision 落盘之前注册，否则 P 与识别冲突会留下陈旧 revision"
    );
    assert!(
        !enable.contains("restore_walkthrough_hotkeys_if_enabled(&app)?;"),
        "开启走查不得在落盘后再用 ? 注册热键"
    );
}

#[test]
fn start_runtime_does_not_abort_setup_when_walkthrough_hotkey_conflicts() {
    let source = include_str!("../src/special_ops/mod.rs");
    let start = source
        .find("pub fn start_runtime")
        .expect("缺少 special_ops::start_runtime");
    let end = source[start..]
        .find("\npub fn shutdown")
        .map(|offset| start + offset)
        .expect("start_runtime 后应紧跟 shutdown");
    let body = &source[start..end];
    assert!(
        !body.contains("restore_walkthrough_hotkeys_if_enabled(app)?;"),
        "启动期恢复走查热键失败不得用 ? 打断 Tauri setup，否则识别热键冲突会让应用闪退"
    );
    assert!(
        body.contains("if let Err(error) = restore_walkthrough_hotkeys_if_enabled(app)"),
        "走查热键恢复失败必须吞掉并继续启动"
    );
    assert!(
        body.contains("disable_walkthrough_session(app, true)"),
        "恢复失败必须关掉走查并落盘，避免下次启动再次闪退"
    );
}

#[test]
fn hiding_other_windows_avoids_sync_visibility_query() {
    let source = include_str!("../src/special_ops/mod.rs");
    let hide_start = source
        .find("fn hide_other_windows_for_special_ops")
        .expect("缺少特勤处窗口隐藏流程");
    let hide_end = source[hide_start..]
        .find("fn restore_other_windows_after_special_ops")
        .map(|offset| hide_start + offset)
        .expect("缺少特勤处窗口恢复流程");
    let hide_source = &source[hide_start..hide_end];
    assert!(
        hide_source.contains("if !should_hide_for_special_ops(&label)"),
        "窗口隐藏流程必须排除主窗口与操作提示窗口"
    );
    assert!(
        !hide_source.contains(".is_visible()"),
        "scheduler 启动路径不得同步读取窗口可见性，否则“继续”命令可能与 UI 线程死锁"
    );
}

#[test]
fn operation_window_is_shown_before_waiting_for_page_load() {
    let source = include_str!("../src/special_ops/mod.rs");
    let create_start = source
        .find("fn create_operation_window")
        .expect("缺少特勤处操作提示窗口创建流程");
    let create_end = source[create_start..]
        .find("fn register_emergency_hotkey")
        .map(|offset| create_start + offset)
        .expect("缺少紧急停止热键注册流程");
    let create_source = &source[create_start..create_end];
    let show = create_source.find(".show()").expect("操作提示窗口必须显示");
    let ready_wait = create_source
        .find("wait_for_operation_window_ready")
        .expect("操作提示窗口必须等待页面就绪");

    assert!(
        show < ready_wait,
        "操作提示窗口必须先显示，再等待 WebView 页面加载；隐藏窗口不会触发 PageLoad"
    );
}

#[test]
fn limited_market_commands_are_registered_without_legacy_color_sampler() {
    let source = include_str!("../src/lib.rs");
    for command in [
        "special_ops_start_limited_supply_trial",
        "special_ops_start_market_trial",
        "special_ops_acknowledge_limited_supply",
        "special_ops_test_limited_supply_colors",
    ] {
        assert!(
            source.contains(command),
            "缺少 Tauri command 注册：{command}"
        );
    }
    assert!(!source.contains("special_ops_sample_limited_supply_color"));
}
