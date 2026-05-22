fn main() {
    tauri_build::build();
    expose_windows_resource_for_tests();
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
