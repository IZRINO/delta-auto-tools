use std::{fs, path::PathBuf};

fn read_manifest(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("windows")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", path.display()))
}

#[test]
fn 主程序与测试使用不同权限清单() {
    let require_admin = ["require", "Administrator"].concat();
    let app = read_manifest("app.manifest");
    assert!(app.contains("name=\"DeltaAutoTools\""));
    assert!(app.contains("Microsoft.Windows.Common-Controls"));
    assert!(app.contains(&format!("level=\"{require_admin}\"")));

    let common = read_manifest("common-controls.manifest");
    assert!(common.contains("name=\"DeltaAutoTools\""));
    assert!(common.contains("Microsoft.Windows.Common-Controls"));
    assert!(!common.contains("requestedExecutionLevel"));

    let current_exe =
        fs::read(std::env::current_exe().expect("读取测试程序路径失败")).expect("读取测试程序失败");
    let embedded = String::from_utf8_lossy(&current_exe);
    let xml_start = ["<?xml", " version"].concat();
    let assembly_end = ["</", "assembly>"].concat();
    let start = embedded.rfind(&xml_start).expect("测试程序未嵌入 manifest");
    let end = embedded[start..]
        .find(&assembly_end)
        .map(|offset| start + offset + assembly_end.len())
        .expect("测试程序 manifest 不完整");
    let embedded = &embedded[start..end];
    let as_invoker = ["level=\"as", "Invoker\""].concat();
    let common_controls = ["Microsoft.Windows.", "Common-Controls"].concat();
    assert!(embedded.contains(&as_invoker));
    assert!(embedded.contains(&common_controls));
    assert!(!embedded.contains(&require_admin));
}
