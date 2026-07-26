fn main() {
    tauri_build::build();
    require_admin_for_main_binary();
    expose_windows_resource_for_tests();
}

fn require_admin_for_main_binary() {
    let Ok(target) = std::env::var("TARGET") else {
        return;
    };
    if !target.contains("windows") {
        return;
    }

    println!(
        "cargo:rustc-link-arg-bin=delta-auto-tools=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'"
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
