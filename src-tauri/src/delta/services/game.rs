use std::collections::HashMap;
use std::sync::Arc;

use reqwest::{cookie::Jar, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::delta::{
    client::{
        http::{build_client, HttpOptions},
        ide::IdeCall,
    },
    constants::{ACI_MAIN, DF_REFERER},
    error::DeltaError,
    utils::{
        encoding::decode_gbk,
        game::{enrich_gun_detail, parse_accessory_config, parse_ammo_config, AmmoItem},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAuth {
    pub openid: String,
    pub access_token: String,
    #[serde(default = "default_acctype")]
    pub acctype: String,
}

fn default_acctype() -> String {
    "qc".to_string()
}

impl GameAuth {
    pub fn acctype_api(&self) -> &str {
        if self.acctype == "wx" { "wx" } else { "qc" }
    }
}

#[derive(Debug, Clone)]
pub struct GameService {
    client: Client,
    #[allow(dead_code)]
    jar: Arc<Jar>,
    ammo_config: HashMap<String, Vec<AmmoItem>>,
    accessory_config: HashMap<String, String>,
}

const AMMO_PHP: &str = include_str!("../../../../ammo.php");
const ACCESSORY_PHP: &str = include_str!("../../../../accessory.php");

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h as u8) * 16 + l as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl GameService {
    pub fn new(options: HttpOptions) -> Result<Self, DeltaError> {
        let (client, jar) = build_client(options)?;
        let ammo_config = parse_ammo_config(AMMO_PHP).unwrap_or_default();
        let accessory_config = parse_accessory_config(ACCESSORY_PHP).unwrap_or_default();
        Ok(Self { client, jar, ammo_config, accessory_config })
    }

    async fn ide(&self, call: IdeCall<'_>) -> Result<Value, DeltaError> {
        call.execute(&self.client).await
    }

    // --- No-auth endpoints ---

    pub async fn get_items(
        &self,
        type_id: i64,
        sub_type: i64,
        item_id: Option<String>,
    ) -> Result<Value, DeltaError> {
        let param = json!({
            "primary": type_id,
            "second": sub_type,
            "objectID": item_id.unwrap_or_default(),
        });
        self.ide(IdeCall::new(352143, "YWRywA", param)).await
    }

    pub async fn get_config(&self) -> Result<Value, DeltaError> {
        let param = json!({ "configType": "all" });
        self.ide(
            IdeCall::new(352143, "YWRywA", param)
                .with_method("dfm/config.list")
                .with_source("5"),
        )
        .await
    }

    pub async fn get_firearm_mod_list(
        &self,
        page: i64,
        page_size: i64,
    ) -> Result<Value, DeltaError> {
        let param = json!({
            "page": page,
            "limit": page_size,
            "solutionType": "gun",
        });
        self.ide(IdeCall::new(352143, "YWRywA", param)).await
    }

    pub async fn get_recommendation(&self, place: &str) -> Result<Value, DeltaError> {
        let param = json!({ "place": place });
        let body = self
            .ide(IdeCall::new(352143, "YWRywA", param))
            .await?;
        Ok(body["data"][place]["list"].clone())
    }

    pub async fn get_price(&self, args: Vec<i64>, with_recent: bool) -> Result<Value, DeltaError> {
        let param = json!({ "objectIDs": args, "withRecent": with_recent });
        self.ide(IdeCall::new(352143, "YWRywA", param)).await
    }

    // --- Auth endpoints ---

    pub async fn get_record(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let mut gun = Vec::new();
        let mut operator = Vec::new();
        for kind in [4, 5] {
            for page in 1..=5 {
                let param = json!({
                    "type": kind,
                    "page": page,
                    "openid": auth.openid,
                    "access_token": auth.access_token,
                    "acctype": auth.acctype_api(),
                });
                let body = self.ide(IdeCall::new(319386, "zMemOt", param)).await?;
                if kind == 4 {
                    gun.push(body);
                } else {
                    operator.push(body);
                }
            }
        }
        Ok(json!({ "gun": gun, "operator": operator }))
    }

    pub async fn get_player(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let base_param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        let mut player = self
            .ide(IdeCall::new(317814, "QIRBwm", base_param.clone()))
            .await?;

        if let Some(name) = player["data"]["charac_name"].as_str() {
            player["data"]["charac_name"] = Value::String(percent_decode(name));
        }

        let mut assets = Vec::new();
        for object_id in ["17888808888", "17888808889", "17020000010"] {
            let param = json!({
                "objectID": object_id,
                "openid": auth.openid,
                "access_token": auth.access_token,
                "acctype": auth.acctype_api(),
            });
            let body = self.ide(IdeCall::new(317814, "QIRBwm", param)).await?;
            assets.push(body);
        }
        Ok(json!({ "player": player, "assets": assets }))
    }

    pub async fn get_assets(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let mut out = Vec::new();
        for object_id in ["17888808888", "17888808889", "17020000010"] {
            let param = json!({
                "objectID": object_id,
                "openid": auth.openid,
                "access_token": auth.access_token,
                "acctype": auth.acctype_api(),
            });
            let body = self.ide(IdeCall::new(317814, "QIRBwm", param)).await?;
            out.push(body);
        }
        Ok(json!(out))
    }

    pub async fn get_logs(
        &self,
        auth: &GameAuth,
        log_type: i64,
        page: i64,
    ) -> Result<Value, DeltaError> {
        let param = json!({
            "logType": log_type,
            "page": page,
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(319386, "zMemOt", param)).await
    }

    pub async fn get_recent(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(319386, "zMemOt", param)).await
    }

    pub async fn get_achievement(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(319386, "zMemOt", param)).await
    }

    pub async fn get_password(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(319386, "zMemOt", param)).await
    }

    pub async fn get_manufacture(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(319386, "zMemOt", param)).await
    }

    pub async fn get_guns(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        let mut body = self.ide(IdeCall::new(319386, "zMemOt", param)).await?;
        if let Some(list) = body["data"]["list"].as_array_mut() {
            for gun in list.iter_mut() {
                enrich_gun_detail(gun, &self.ammo_config, &self.accessory_config);
            }
        }
        Ok(body)
    }

    pub async fn get_bind(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        let bind = self.ide(IdeCall::new(316964, "95ookO", param)).await?;
        let bindarea = &bind["data"]["bindarea"];
        if !bindarea.is_null() && !bindarea.as_str().unwrap_or("").is_empty() {
            return Ok(bind);
        }

        let resp = self
            .client
            .get(ACI_MAIN)
            .header("Referer", DF_REFERER)
            .query(&[
                ("needGopenid", "1"),
                ("sAMSAcctype", auth.acctype_api()),
                ("sAMSAccessToken", auth.access_token.as_str()),
                ("sAMSAppOpenId", auth.openid.as_str()),
                ("sAMSSourceAppId", "101491592"),
                ("game", "dfm"),
                ("sCloudApiName", "ams.gameattr.role"),
                ("area", "36"),
                ("platid", "1"),
                ("partition", "36"),
            ])
            .send()
            .await?
            .bytes()
            .await?;
        let body = decode_gbk(&resp);
        let parsed = crate::delta::utils::game::parse_bind_role_js(&body)?;
        let checkparam = parsed.get("checkparam").cloned().unwrap_or_default();
        let md5str = parsed.get("md5str").cloned().unwrap_or_default();
        let role_id = checkparam
            .split('|')
            .nth(2)
            .unwrap_or_default()
            .to_string();

        let bind_param = json!({
            "sArea": 36,
            "sPlatId": 1,
            "sPartition": 36,
            "sCheckparam": checkparam,
            "sRoleId": role_id,
            "md5str": md5str,
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(316965, "sTzZS2", bind_param)).await
    }
}
