//! 扫描 `人名/1.png` 目录并灌进图库。启动时给内置资源用。

use std::fs;
use std::path::{Path, PathBuf};

use super::types::FingerprintPerson;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub name: String,
    pub sources: [Option<PathBuf>; 8],
}

pub fn scan_library_dir(root: &Path) -> Result<Vec<ImportPlan>, String> {
    if !root.is_dir() {
        return Err(format!("不是目录: {}", root.display()));
    }
    let mut plans = Vec::new();
    let entries = fs::read_dir(root).map_err(|error| format!("无法读取图库目录: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("无法读取图库项: {error}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .to_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("目录名无效: {}", path.display()))?
            .to_string();
        let mut sources: [Option<PathBuf>; 8] = Default::default();
        let files = fs::read_dir(&path).map_err(|error| format!("无法读取 {name}: {error}"))?;
        for file in files {
            let file = file.map_err(|error| format!("无法读取 {name} 内文件: {error}"))?;
            let file_path = file.path();
            if !file_path.is_file() {
                continue;
            }
            let Some(slot) = slot_from_filename(&file_path) else {
                continue;
            };
            sources[slot] = Some(file_path);
        }
        if sources.iter().any(Option::is_some) {
            plans.push(ImportPlan { name, sources });
        }
    }
    plans.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(plans)
}

fn slot_from_filename(path: &Path) -> Option<usize> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp") {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let number: usize = stem.parse().ok()?;
    (1..=8).contains(&number).then_some(number - 1)
}

pub fn apply_import(
    people: &mut Vec<FingerprintPerson>,
    plans: &[ImportPlan],
    dest_root: &Path,
    overwrite: bool,
) -> Result<usize, String> {
    let mut imported = 0_usize;
    for plan in plans {
        let person_id = people
            .iter()
            .find(|person| person.name == plan.name)
            .map(|person| person.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let dest_dir = dest_root.join(&person_id);
        fs::create_dir_all(&dest_dir)
            .map_err(|error| format!("无法创建 {} 目录: {error}", plan.name))?;
        let mut fingerprint_paths = people
            .iter()
            .find(|person| person.id == person_id)
            .map(|person| person.fingerprint_paths.clone())
            .unwrap_or_default();
        for (slot, source) in plan.sources.iter().enumerate() {
            let Some(source) = source else {
                continue;
            };
            if !overwrite && fingerprint_paths[slot].is_some() {
                continue;
            }
            let dest = dest_dir.join(format!("{}.png", slot + 1));
            fs::copy(source, &dest)
                .map_err(|error| format!("复制 {} 第 {} 枚失败: {error}", plan.name, slot + 1))?;
            fingerprint_paths[slot] = Some(dest.to_string_lossy().into_owned());
            imported += 1;
        }
        if let Some(person) = people.iter_mut().find(|person| person.id == person_id) {
            person.fingerprint_paths = fingerprint_paths;
        } else {
            people.push(FingerprintPerson {
                id: person_id,
                name: plan.name.clone(),
                name_image_path: String::new(),
                fingerprint_paths,
            });
        }
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join(name), b"png").unwrap();
    }

    #[test]
    fn scan_reads_numbered_pngs_and_skips_noise() {
        let root = tempfile::tempdir().unwrap();
        write_file(&root.path().join("克莱尔"), "1.png");
        write_file(&root.path().join("克莱尔"), "6.png");
        write_file(&root.path().join("克莱尔"), "readme.txt");
        write_file(&root.path().join("埃德温"), "1.png");
        write_file(&root.path().join("埃德温"), "4.png");
        fs::write(root.path().join("ignore.png"), b"x").unwrap();

        let plans = scan_library_dir(root.path()).unwrap();
        assert_eq!(plans.len(), 2);
        let claire = plans.iter().find(|plan| plan.name == "克莱尔").unwrap();
        let edwin = plans.iter().find(|plan| plan.name == "埃德温").unwrap();
        assert!(edwin.sources[0].is_some());
        assert!(edwin.sources[3].is_some());
        assert!(edwin.sources[4].is_none());
        assert!(claire.sources[5].is_some());
    }

    #[test]
    fn apply_import_upserts_by_name_keeps_name_strip() {
        let dest = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        write_file(&src.path().join("克莱尔"), "1.png");
        write_file(&src.path().join("克莱尔"), "2.png");
        let plans = scan_library_dir(src.path()).unwrap();
        let mut people = vec![FingerprintPerson {
            id: "keep".into(),
            name: "克莱尔".into(),
            name_image_path: "already-name.png".into(),
            fingerprint_paths: Default::default(),
        }];
        let count = apply_import(&mut people, &plans, dest.path(), true).unwrap();
        assert_eq!(count, 2);
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].id, "keep");
        assert_eq!(people[0].name_image_path, "already-name.png");
        assert!(people[0].fingerprint_paths[0]
            .as_ref()
            .unwrap()
            .ends_with("1.png"));
        assert!(people[0].fingerprint_paths[1].is_some());
    }

    #[test]
    fn fill_only_skips_existing_slots() {
        let dest = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        write_file(&src.path().join("克莱尔"), "1.png");
        write_file(&src.path().join("克莱尔"), "2.png");
        let plans = scan_library_dir(src.path()).unwrap();
        let mut people = vec![FingerprintPerson {
            id: "keep".into(),
            name: "克莱尔".into(),
            name_image_path: String::new(),
            fingerprint_paths: [
                Some("old-1.png".into()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        }];
        let count = apply_import(&mut people, &plans, dest.path(), false).unwrap();
        assert_eq!(count, 1);
        assert_eq!(people[0].fingerprint_paths[0].as_deref(), Some("old-1.png"));
        assert!(people[0].fingerprint_paths[1].is_some());
    }

    #[test]
    fn bundled_repo_images_cover_nine_people() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("fingerprint");
        let plans = scan_library_dir(&root).unwrap();
        assert_eq!(plans.len(), 9);
        let geheros = plans.iter().find(|plan| plan.name == "格赫罗斯").unwrap();
        assert!(geheros.sources[7].is_some());
        let edwin = plans.iter().find(|plan| plan.name == "埃德温").unwrap();
        assert!(edwin.sources[3].is_some());
        assert!(edwin.sources[4].is_none());
    }
}
