fn main() {
    let attributes = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attributes).expect("运行 Tauri 构建脚本失败");
    configure_windows_manifests();
    expose_windows_resource_for_tests();
}

fn configure_windows_manifests() {
    let Ok(target) = std::env::var("TARGET") else {
        return;
    };
    if !target.contains("windows") {
        return;
    }

    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("缺少 CARGO_MANIFEST_DIR"),
    )
    .join("windows");
    let app_manifest = manifest_dir.join("app.manifest");
    let common_controls_manifest = manifest_dir.join("common-controls.manifest");

    println!("cargo:rerun-if-changed={}", app_manifest.display());
    println!(
        "cargo:rerun-if-changed={}",
        common_controls_manifest.display()
    );
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg=/MANIFESTINPUT:{}",
        common_controls_manifest.display()
    );
    println!("cargo:rustc-link-arg-bin=delta-auto-tools=/MANIFESTUAC:NO");
    println!("cargo:rustc-link-arg-bin=delta-auto-tools=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bin=delta-auto-tools=/MANIFESTINPUT:{}",
        app_manifest.display()
    );
}

fn expose_windows_resource_for_tests() {
    let Ok(target) = std::env::var("TARGET") else {
        return;
    };
    if !target.contains("windows") {
        return;
    }

    let Ok(out_dir) = std::env::var("OUT_DIR") else {
        return;
    };
    let resource_lib = std::path::Path::new(&out_dir).join("resource.lib");
    if resource_lib.exists() {
        println!("cargo:rustc-link-search=native={out_dir}");
    }
}
