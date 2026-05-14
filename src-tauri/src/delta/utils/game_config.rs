use std::collections::HashMap;

use super::game::AmmoItem;

const AMMO_CONFIG: &[(&str, &[(&str, i32)])] = &[
    ("ammo5.56x45", &[("5.56x45mm M995", 5), ("5.56x45mm M855A1", 4), ("5.56x45mm M855", 3), ("5.56x45mm FMJ", 2), ("5.56x45mm RRLP", 1)]),
    ("ammo7.62x39", &[("7.62x39mm AP", 5), ("7.62x39mm BP", 4), ("7.62x39mm PS", 3), ("7.62x39mm T45M", 2), ("7.62x39mm LP", 1)]),
    ("ammo5.8x42", &[("5.8x42mm DVC12", 5), ("5.8x42mm DBP10", 4), ("5.8x42mm DVP88", 3), ("5.8x42mm DBP87", 2)]),
    ("ammo5.45x39", &[("5.45x39mm BS", 5), ("5.45x39mm BT", 4), ("5.45x39mm PS", 3), ("5.45x39mm T", 2), ("5.45x39mm PRS", 1)]),
    ("ammo12.7x55", &[("12.7x55mm PS12B", 5), ("12.7x55mm PS12", 4), ("12.7x55mm PD12双头弹", 4), ("12.7x55mm PS12A", 3)]),
    ("ammo6.8x51", &[("6.8x51mm AP", 6), ("6.8x51mm Hybrid", 5), ("6.8x51mm FMJ", 4)]),
    ("ammo7.62x51", &[("7.62x51mm M61", 6), ("7.62x51mm M62", 5), ("7.62x51mm M80", 4), ("7.62x51mm BPZ", 3), ("7.62x51mm UN", 2)]),
    ("ammo9x39", &[("9x39mm BP", 5), ("9x39mm SP6", 4), ("9x39mm SP5", 3)]),
    ("ammo7.62x35", &[(".300BLK", 5), (".300BLK", 4), (".300BLK", 3)]),
    ("ammo9x19", &[("9x19mm PBP", 4), ("9x19mm RIP", 3), ("9x19mm AP6.3", 3), ("9x19mm Pst", 2), ("9x19mm PSO", 1)]),
    ("ammo5.7x28", &[("5.7x28mm SS190", 5), ("5.7x28mm SS193", 4), ("5.7x28mm R37.F", 3), ("5.7x28mm L191", 3), ("5.7x28mm SS197SR", 2), ("5.7x28mm SS198LF", 1)]),
    ("ammo.45", &[(".45 ACP AP", 4), (".45 ACP RIP", 3), (".45 ACP FMJ", 3), (".45 ACP JHP", 2), (".45 ACP HS", 1)]),
    ("ammo4.6x30", &[("4.6x30mm AP SX", 5), ("4.6x30mm FMJ SX", 4), ("4.6x30mm Subsonic SX", 3)]),
    ("ammo.357", &[(".357 Magnum FMJ", 4), (".357 Magnum 9号霰射\"鼠弹\"", 3), (".357 Magnum JHP", 3), (".357 Magnum HP", 2)]),
    ("ammo.50", &[(".50 AE FMJ", 4), (".50 AE JHP", 3), (".50 AE HP", 2)]),
    ("ammo7.62x54", &[("7.62x54R SNB", 6), ("7.62x54R BT", 5), ("7.62x54R LPS", 4), ("7.62x54R T46M", 3)]),
    ("ammo.338", &[(".338 Lap Mag AP", 7)]),
    ("ammo12", &[("12 Gauge独头 AP-20", 4), ("12 Gauge 龙息弹", 4), ("12 Gauge 8.5毫米鹿弹", 3), ("12 Gauge 箭形弹", 3), ("12 Gauge独头 FTX", 3), ("12 Gauge独头 RIP", 2), ("12 Gauge独头 GT", 2), ("12 Gauge 7毫米鹿弹", 1)]),
];

const ACCESSORY_CONFIG: &[(&str, &str)] = &[
    ("1", "后握把"),
    ("2", "枪管"),
    ("3", "枪托"),
    ("4", "护木"),
    ("5", "弹匣"),
    ("6", "枪口"),
    ("7", ""),
    ("8", "前握把"),
    ("9", ""),
    ("10", "脚架"),
    ("11", "瞄准镜"),
    ("12", ""),
    ("13", ""),
    ("14", "战术设备"),
    ("15", ""),
    ("16", ""),
    ("17", "上导轨"),
    ("18", "下导轨"),
    ("19", "左导轨"),
    ("20", "右导轨"),
    ("21", ""),
    ("22", "独特组件"),
    ("23", ""),
    ("24", ""),
    ("25", ""),
    ("26", ""),
    ("27", ""),
    ("28", "枪托垫"),
    ("29", "托腮板"),
    ("30", ""),
    ("31", "侧副瞄具"),
    ("32", "上贴片"),
    ("33", ""),
    ("34", "左贴片"),
    ("35", "右贴片"),
    ("36", ""),
    ("37", "护木套件"),
    ("38", "枪托套件"),
    ("39", ""),
    ("40", "上副瞄具"),
    ("41", "增高座瞄具"),
    ("42", ""),
    ("43", ""),
    ("44", "弹匣座"),
    ("45", "遮光罩"),
    ("46", ""),
    ("47", "导气"),
    ("48", "击锤"),
    ("49", "扳机"),
    ("50", "增高座瞄具"),
    ("51", ""),
    ("52", ""),
    ("53", ""),
    ("54", ""),
];

pub fn built_in_ammo_config() -> HashMap<String, Vec<AmmoItem>> {
    AMMO_CONFIG
        .iter()
        .map(|(caliber, items)| {
            (
                (*caliber).to_string(),
                items
                    .iter()
                    .map(|(name, grade)| AmmoItem {
                        name: (*name).to_string(),
                        grade: *grade,
                    })
                    .collect(),
            )
        })
        .collect()
}

pub fn built_in_accessory_config() -> HashMap<String, String> {
    ACCESSORY_CONFIG
        .iter()
        .map(|(slot, name)| ((*slot).to_string(), (*name).to_string()))
        .collect()
}
