use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::error::DeltaError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmmoItem {
    pub name: String,
    pub grade: i32,
}

pub fn normalize_caliber_code(raw: &str) -> String {
    if raw.starts_with("ammo") {
        raw.to_string()
    } else {
        format!("ammo{raw}")
    }
}

pub fn parse_bind_role_js(raw: &str) -> Result<HashMap<String, String>, DeltaError> {
    let block_re =
        Regex::new(r"\{[^{}]*\}").map_err(|error| DeltaError::Parse(error.to_string()))?;
    let block = block_re
        .find(raw)
        .ok_or_else(|| DeltaError::Parse("no role block".to_string()))?
        .as_str();
    let kv_re = Regex::new(r#"[\"']?(\w+)[\"']?\s*:\s*[\"']([^\"']*)[\"']"#)
        .map_err(|error| DeltaError::Parse(error.to_string()))?;

    let mut out = HashMap::new();
    for captures in kv_re.captures_iter(block) {
        out.insert(captures[1].to_string(), captures[2].to_string());
    }
    Ok(out)
}

pub fn enrich_gun_detail(
    gun: &mut Value,
    ammo_config: &HashMap<String, Vec<AmmoItem>>,
    accessory_config: &HashMap<String, String>,
) {
    let Some(detail) = gun.get_mut("gunDetail") else {
        return;
    };

    let caliber = normalize_caliber_code(detail["caliber"].as_str().unwrap_or_default());
    detail["caliber"] = Value::String(caliber.clone());

    if let Some(ammo_list) = detail["ammo"].as_array_mut() {
        for (index, ammo) in ammo_list.iter_mut().enumerate() {
            let object_id = ammo["objectID"].clone();
            let mapped = ammo_config.get(&caliber).and_then(|items| items.get(index));
            *ammo = json!({
                "objectID": object_id,
                "name": mapped.map(|item| item.name.clone()).unwrap_or_default(),
                "grade": mapped.map(|item| item.grade).unwrap_or_default(),
            });
        }
    }

    for field in ["accessory", "allAccessory"] {
        if let Some(accessories) = detail[field].as_array_mut() {
            for accessory in accessories.iter_mut() {
                let slot_id = accessory["slotID"]
                    .as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| accessory["slotID"].as_i64().map(|value| value.to_string()))
                    .unwrap_or_default();
                *accessory = json!({
                    "slotID": slot_id,
                    "name": accessory_config.get(&slot_id).cloned().unwrap_or_default(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{enrich_gun_detail, normalize_caliber_code, parse_bind_role_js, AmmoItem};
    use crate::delta::utils::game_config::{built_in_accessory_config, built_in_ammo_config};

    #[test]
    fn normalizes_plain_caliber_codes() {
        assert_eq!(normalize_caliber_code("7.62x51"), "ammo7.62x51");
        assert_eq!(normalize_caliber_code("ammo5.56x45"), "ammo5.56x45");
    }

    #[test]
    fn parses_bind_role_fragment() {
        let parsed = parse_bind_role_js(
            "callback({\"msg\":\"ok\",\"checkparam\":\"a|b|role-42\",\"md5str\":\"sig\"})",
        )
        .unwrap();

        assert_eq!(parsed.get("checkparam").unwrap(), "a|b|role-42");
        assert_eq!(parsed.get("md5str").unwrap(), "sig");
    }

    #[test]
    fn built_in_configs_enrich_gun_detail() {
        let ammo = built_in_ammo_config();
        let accessory = built_in_accessory_config();

        let mut gun = json!({
            "gunDetail": {
                "caliber": "7.62x51",
                "ammo": [
                    { "objectID": 1 },
                    { "objectID": 2 }
                ],
                "accessory": [
                    { "slotID": 8 }
                ],
                "allAccessory": [
                    { "slotID": "11" }
                ]
            }
        });

        enrich_gun_detail(&mut gun, &ammo, &accessory);

        assert_eq!(gun["gunDetail"]["caliber"], "ammo7.62x51");
        assert_eq!(gun["gunDetail"]["ammo"][0]["name"], "7.62x51mm M61");
        assert_eq!(gun["gunDetail"]["ammo"][1]["grade"], 5);
        assert_eq!(gun["gunDetail"]["accessory"][0]["name"], "前握把");
        assert_eq!(gun["gunDetail"]["allAccessory"][0]["name"], "瞄准镜");
    }

    #[test]
    fn built_in_configs_expose_expected_entries() {
        let ammo = built_in_ammo_config();
        let accessory = built_in_accessory_config();

        assert_eq!(ammo["ammo5.56x45"][0].name, "5.56x45mm M995");
        assert_eq!(ammo["ammo5.56x45"][0].grade, 5);
        assert_eq!(ammo["ammo.338"][0].grade, 7);
        assert_eq!(accessory["8"], "前握把");
        assert_eq!(accessory["45"], "遮光罩");
    }

    #[test]
    fn leaves_missing_config_entries_empty() {
        let mut gun = json!({
            "gunDetail": {
                "caliber": "9x19",
                "ammo": [{ "objectID": 9 }],
                "accessory": [{ "slotID": 99 }],
                "allAccessory": []
            }
        });

        enrich_gun_detail(
            &mut gun,
            &HashMap::<String, Vec<AmmoItem>>::new(),
            &HashMap::new(),
        );

        assert_eq!(gun["gunDetail"]["caliber"], "ammo9x19");
        assert_eq!(gun["gunDetail"]["ammo"][0]["name"], "");
        assert_eq!(gun["gunDetail"]["accessory"][0]["name"], "");
    }
}
