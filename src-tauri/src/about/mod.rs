pub mod events;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// 前端依赖项
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub name: String,
    pub kind: String,
    pub license: String,
    pub url: String,
}

/// 更新检查结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
}

/// 更新进度
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "phase")]
pub enum UpdateProgress {
    Checking,
    NotAvailable,
    Available {
        version: String,
        notes: Option<String>,
    },
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Downloaded,
    Installing,
    Installed,
    Error {
        message: String,
    },
}

/// 关于面板引导数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutBootstrap {
    pub name: String,
    pub version: String,
    pub identifier: String,
    pub target: String,
    pub tauri_version: String,
    pub license: String,
    pub license_url: String,
    pub repository_url: String,
    pub dependencies: Vec<Dependency>,
}

const REPOSITORY_URL: &str = "https://github.com/IZRINO/delta-auto-tools";
const LICENSE_URL: &str = "https://github.com/IZRINO/delta-auto-tools/blob/master/LICENSE";

const LICENSE_TEXT: &str = include_str!("../../../LICENSE");

/// 前端 + Rust 主要依赖列表（编译期硬编码）
fn built_in_dependencies() -> Vec<Dependency> {
    let mut deps = Vec::new();

    // ── 前端 ──
    let frontend: Vec<(&str, &str, &str)> = vec![
        ("React 19", "MIT", "https://react.dev"),
        ("Vite 7", "MIT", "https://vite.dev"),
        ("@remixicon/react", "Apache-2.0", "https://remixicon.com"),
        ("@fontsource-variable/jetbrains-mono", "OFL-1.1", "https://fontsource.org/fonts/jetbrains-mono"),
        ("@tauri-apps/api", "MIT/Apache-2.0", "https://tauri.app"),
        ("@tauri-apps/plugin-opener", "MIT/Apache-2.0", "https://tauri.app"),
        ("@tauri-apps/plugin-updater", "MIT/Apache-2.0", "https://tauri.app"),
        ("@tauri-apps/plugin-process", "MIT/Apache-2.0", "https://tauri.app"),
        ("radix-ui", "MIT", "https://www.radix-ui.com"),
        ("shadcn/ui", "MIT", "https://ui.shadcn.com"),
        ("tailwindcss 4", "MIT", "https://tailwindcss.com"),
        ("sonner", "MIT", "https://sonner.emilkowal.dev"),
        ("date-fns", "MIT", "https://date-fns.org"),
    ];
    for (name, license, url) in frontend {
        deps.push(Dependency {
            name: name.to_string(),
            kind: "frontend".to_string(),
            license: license.to_string(),
            url: url.to_string(),
        });
    }

    // ── Rust 运行时 ──
    let runtime: Vec<(&str, &str, &str)> = vec![
        ("tauri", "MIT/Apache-2.0", "https://tauri.app"),
        ("tauri-plugin-updater", "MIT/Apache-2.0", "https://tauri.app"),
        ("tauri-plugin-opener", "MIT/Apache-2.0", "https://tauri.app"),
        ("tauri-plugin-window-state", "MIT/Apache-2.0", "https://tauri.app"),
        ("tauri-plugin-process", "MIT/Apache-2.0", "https://tauri.app"),
        ("reqwest", "MIT/Apache-2.0", "https://github.com/seanmonstar/reqwest"),
        ("enigo", "MIT", "https://github.com/enigo-rs/enigo"),
        ("willhook", "MIT", "https://github.com/2hndr/willhook"),
        ("xcap", "MIT", "https://github.com/nicedoc/xcap"),
        ("image", "MIT/Apache-2.0", "https://github.com/image-rs/image"),
        ("rodio", "MIT/Apache-2.0", "https://github.com/RustAudio/rodio"),
        ("tokio", "MIT", "https://tokio.rs"),
        ("serde", "MIT/Apache-2.0", "https://serde.rs"),
        ("serde_json", "MIT/Apache-2.0", "https://github.com/serde-rs/json"),
        ("regex", "MIT/Apache-2.0", "https://github.com/rust-lang/regex"),
        ("thiserror", "MIT/Apache-2.0", "https://github.com/dtolnay/thiserror"),
        ("crossbeam-channel", "MIT/Apache-2.0", "https://github.com/crossbeam-rs/crossbeam"),
        ("url", "MIT/Apache-2.0", "https://github.com/servo/rust-url"),
        ("windows-sys", "MIT/Apache-2.0", "https://github.com/microsoft/windows-rs"),
    ];
    for (name, license, url) in runtime {
        deps.push(Dependency {
            name: name.to_string(),
            kind: "runtime".to_string(),
            license: license.to_string(),
            url: url.to_string(),
        });
    }

    deps
}

#[tauri::command]
pub fn about_get_bootstrap(app: AppHandle) -> AboutBootstrap {
    let info = app.package_info();
    AboutBootstrap {
        name: info.name.clone(),
        version: info.version.to_string(),
        identifier: app.config().identifier.clone(),
        target: std::env::consts::OS.to_string(),
        tauri_version: tauri::VERSION.to_string(),
        license: LICENSE_TEXT.to_string(),
        license_url: LICENSE_URL.to_string(),
        repository_url: REPOSITORY_URL.to_string(),
        dependencies: built_in_dependencies(),
    }
}

/// 提取版本号的数值部分元组 (major, minor, patch)，忽略 pre-release 后缀
/// "0.17.0-beta.5" → (0, 17, 0)
/// "0.17.1"        → (0, 17, 1)
fn numeric_version_tuple(v: &str) -> (u64, u64, u64) {
    let base = v.split_once('-').map_or(v, |(s, _)| s);
    let parts: Vec<u64> = base.split('.').filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [a, b, c] => (*a, *b, *c),
        _ => (0, 0, 0),
    }
}

/// 返回版本的可比较秩：(major, minor, patch, 是否正式版, pre-release 原始字符串)。
/// 正式版（无 pre-release）秩高于同数值的 pre-release 版本，符合 SemVer
/// （`0.17.0-beta.5 < 0.17.0`）。pre-release 之间仅按原始字符串字典序比较，
/// 用于区分同一版本下不同 beta，不追求严格 SemVer 标识符排序。
fn version_rank(v: &str) -> (u64, u64, u64, bool, &str) {
    match v.split_once('-') {
        Some((base, pre)) => {
            let (a, b, c) = numeric_version_tuple(base);
            (a, b, c, false, pre)
        }
        None => {
            let (a, b, c) = numeric_version_tuple(v);
            (a, b, c, true, "")
        }
    }
}

/// 判断是否应提供更新：remote 严格高于 current 时为 true。
/// 遵循 SemVer：beta 会更新到同数值正式版（0.17.0-beta.5 → 0.17.0 提供更新），
/// 也会更新到更高数值的正式版（0.17.0-beta.5 → 0.17.1 提供更新）；
/// 正式版不会降级到同数值 beta。
fn should_offer_update(current: &str, remote: &str) -> bool {
    version_rank(remote) > version_rank(current)
}

#[tauri::command]
pub async fn about_check_for_update(app: AppHandle) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;

    let current_version = app.package_info().version.to_string();

    let update = app
        .updater()
        .map_err(|e| classify_updater_error(e))?
        .check()
        .await
        .map_err(|e| classify_check_error(&e))?;

    match update {
        Some(update) if should_offer_update(&current_version, &update.version) => Ok(UpdateInfo {
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            pub_date: update.date.map(|d| d.to_string()),
        }),
        _ => Ok(UpdateInfo {
            available: false,
            version: None,
            notes: None,
            pub_date: None,
        }),
    }
}

#[tauri::command]
pub async fn about_download_and_install(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;

    let app_clone = app.clone();
    let _ = app_clone.emit_to("main", events::UPDATE_PROGRESS, UpdateProgress::Checking);

    let current_version = app.package_info().version.to_string();

    let update = app
        .updater()
        .map_err(|e| {
            let msg = classify_updater_error(e);
            let app_err = app.clone();
            let _ = app_err.emit_to(
                "main",
                events::UPDATE_PROGRESS,
                UpdateProgress::Error { message: msg.clone() },
            );
            msg
        })?
        .check()
        .await
        .map_err(|e| {
            let msg = classify_check_error(&e);
            let app_err = app.clone();
            let _ = app_err.emit_to(
                "main",
                events::UPDATE_PROGRESS,
                UpdateProgress::Error { message: msg.clone() },
            );
            msg
        })?;

    let update = match update {
        Some(u) if should_offer_update(&current_version, &u.version) => u,
        _ => {
            let app_na = app.clone();
            let _ = app_na.emit_to("main", events::UPDATE_PROGRESS, UpdateProgress::NotAvailable);
            return Ok(());
        }
    };

    let version = update.version.clone();
    let notes = update.body.clone();

    let app_avail = app.clone();
    let _ = app_avail.emit_to(
        "main",
        events::UPDATE_PROGRESS,
        UpdateProgress::Available {
            version: version.clone(),
            notes: notes.clone(),
        },
    );

    let mut downloaded: u64 = 0;
    let app_dl = app.clone();

    update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                let _ = app_dl.emit_to(
                    "main",
                    events::UPDATE_PROGRESS,
                    UpdateProgress::Downloading {
                        downloaded,
                        total: content_length.map(|l| l as u64),
                    },
                );
            },
            {
                let app_done = app.clone();
                move || {
                    let _ = app_done.emit_to("main", events::UPDATE_PROGRESS, UpdateProgress::Downloaded);
                    let _ = app_done.emit_to("main", events::UPDATE_PROGRESS, UpdateProgress::Installing);
                }
            },
        )
        .await
        .map_err(|e| {
            let msg = format!("下载安装失败: {e}");
            let app_err = app.clone();
            let _ = app_err.emit_to(
                "main",
                events::UPDATE_PROGRESS,
                UpdateProgress::Error { message: msg.clone() },
            );
            msg
        })?;

    let app_installed = app.clone();
    let _ = app_installed.emit_to("main", events::UPDATE_PROGRESS, UpdateProgress::Installed);

    Ok(())
}

/// 将 updater 初始化错误转为用户友好的中文信息
fn classify_updater_error(e: tauri_plugin_updater::Error) -> String {
    let msg = e.to_string();
    if msg.contains("pubkey") || msg.contains("public key") || msg.contains("signature") {
        "自动更新未配置签名密钥，请前往 GitHub Release 页面手动下载更新".to_string()
    } else {
        format!("更新器初始化失败: {msg}")
    }
}

/// 将 check 错误转为用户友好的中文信息
fn classify_check_error(e: &tauri_plugin_updater::Error) -> String {
    let msg = e.to_string();
    if msg.contains("Could not fetch") || msg.contains("release JSON") || msg.contains("404") {
        "暂无可用更新文件，请前往 GitHub Release 页面手动检查".to_string()
    } else if msg.contains("network") || msg.contains("timeout") || msg.contains("connect") || msg.contains("dns") {
        format!("网络连接失败: {msg}")
    } else {
        format!("检查更新失败: {msg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_version_tuple_parses_standard() {
        assert_eq!(numeric_version_tuple("0.17.1"), (0, 17, 1));
        assert_eq!(numeric_version_tuple("1.2.3"), (1, 2, 3));
    }

    #[test]
    fn numeric_version_tuple_strips_prerelease() {
        assert_eq!(numeric_version_tuple("0.17.0-beta.1"), (0, 17, 0));
        assert_eq!(numeric_version_tuple("0.17.0-beta.5"), (0, 17, 0));
        assert_eq!(numeric_version_tuple("1.0.0-alpha.99"), (1, 0, 0));
    }

    #[test]
    fn numeric_version_tuple_handles_invalid() {
        assert_eq!(numeric_version_tuple(""), (0, 0, 0));
        assert_eq!(numeric_version_tuple("x.y.z"), (0, 0, 0));
        assert_eq!(numeric_version_tuple("1"), (0, 0, 0));
    }

    #[test]
    fn should_offer_update_beta_to_same_numeric_stable_is_true() {
        // 0.17.0-beta.5 → 0.17.0：beta 升级到同数值正式版（SemVer：pre-release 优先级低于正式版），提供更新
        assert!(should_offer_update("0.17.0-beta.5", "0.17.0"));
        assert!(should_offer_update("0.17.0-beta.1", "0.17.0"));
        assert!(should_offer_update("0.17.0-beta.3", "0.17.0"));
    }

    #[test]
    fn should_offer_update_stable_to_same_numeric_beta_is_false() {
        // 0.17.0 → 0.17.0-beta.5：正式版不降级到同数值 beta，不更新
        assert!(!should_offer_update("0.17.0", "0.17.0-beta.5"));
        assert!(!should_offer_update("0.17.0", "0.17.0-beta.1"));
    }

    #[test]
    fn should_offer_update_beta_to_higher_numeric_stable_is_true() {
        // 0.17.0-beta.5 → 0.17.1：数值部分更高，提供更新
        assert!(should_offer_update("0.17.0-beta.5", "0.17.1"));
        // 0.16.0-beta.1 → 0.17.0：数值部分更高，提供更新
        assert!(should_offer_update("0.16.0-beta.1", "0.17.0"));
    }

    #[test]
    fn should_offer_update_stable_to_stable_is_standard() {
        // 正式版之间的标准比较
        assert!(should_offer_update("0.16.3", "0.17.0"));
        assert!(should_offer_update("0.17.0", "0.17.1"));
        assert!(!should_offer_update("0.17.0", "0.17.0"));
        assert!(!should_offer_update("0.17.1", "0.17.0"));
    }
}
