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

#[tauri::command]
pub async fn about_check_for_update(app: AppHandle) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;

    let update = app
        .updater()
        .map_err(|e| classify_updater_error(e))?
        .check()
        .await
        .map_err(|e| classify_check_error(&e))?;

    match update {
        Some(update) => Ok(UpdateInfo {
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            pub_date: update.date.map(|d| d.to_string()),
        }),
        None => Ok(UpdateInfo {
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
        Some(u) => u,
        None => {
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
