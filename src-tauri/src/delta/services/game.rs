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
    constants::{ACI_MAIN, DF_REFERER, IDE_GATEWAY},
    error::DeltaError,
    response::ApiResponse,
    utils::{
        encoding::decode_gbk,
        game::{enrich_gun_detail, AmmoItem},
        game_config::{built_in_accessory_config, built_in_ammo_config},
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
        if self.acctype == "wx" {
            "wx"
        } else {
            "qc"
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameService {
    client: Client,
    #[allow(dead_code)]
    jar: Arc<Jar>,
    ammo_config: HashMap<String, Vec<AmmoItem>>,
    accessory_config: HashMap<String, String>,
    ide_gateway: String,
}

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
        let ammo_config = built_in_ammo_config();
        let accessory_config = built_in_accessory_config();
        Ok(Self {
            client,
            jar,
            ammo_config,
            accessory_config,
            ide_gateway: IDE_GATEWAY.to_string(),
        })
    }

    async fn ide(&self, call: IdeCall<'_>) -> Result<Value, DeltaError> {
        call.execute_with_url(&self.client, &self.ide_gateway).await
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
        let param = json!({
            "type": "place",
            "place": place,
            "hasPriceData": true,
        });
        let body = self.ide(IdeCall::new(352143, "YWRywA", param)).await?;
        Ok(body["data"][place]["list"].clone())
    }

    pub async fn get_price(&self, args: Vec<i64>, with_recent: bool) -> Result<Value, DeltaError> {
        let mut latest = self
            .ide(
                IdeCall::new(352143, "YWRywA", json!({ "ids": args }))
                    .with_method("dfm/object.price.latest"),
            )
            .await?;

        if with_recent {
            let Some(map) = latest.as_object_mut() else {
                return Err(DeltaError::Parse("物价最新数据格式异常".to_string()));
            };
            for key in map.keys().cloned().collect::<Vec<_>>() {
                let recent = self
                    .ide(
                        IdeCall::new(352143, "YWRywA", json!({ "objectID": key }))
                            .with_method("dfm/object.price.recent"),
                    )
                    .await?;
                let recent_list = recent["objectPriceRecent"]["list"]
                    .as_array()
                    .ok_or_else(|| DeltaError::Parse("物价近期数据格式异常".to_string()))?
                    .clone();
                if let Some(entry) = map.get_mut(&key) {
                    entry["recent"] = Value::Array(recent_list);
                }
            }
        }

        Ok(latest)
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

        for (key, object_id) in [
            ("coin", 17888808888_i64),
            ("tickets", 17888808889_i64),
            ("money", 17020000010_i64),
        ] {
            let param = json!({
                "type": 3,
                "page": 1,
                "itemId": object_id,
                "openid": auth.openid,
                "access_token": auth.access_token,
                "acctype": auth.acctype_api(),
            });
            let body = self.ide(IdeCall::new(319386, "zMemOt", param)).await?;
            player[key] = body["data"][0]["totalMoney"].clone();
        }
        Ok(player)
    }

    pub async fn get_assets(&self, auth: &GameAuth) -> Result<ApiResponse<Value>, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        let raw = self.ide(IdeCall::new(318948, "Plaqzy", param)).await?;

        let ret = raw.get("ret").and_then(Value::as_i64);
        if ret == Some(-4000) {
            return Ok(ApiResponse::of(
                -1,
                "您的账号由于腾讯内部错误无法使用这个功能",
                json!([]),
            ));
        }
        if matches!(ret, Some(value) if value != 0) {
            let msg = raw
                .get("msg")
                .or_else(|| raw.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("资产查询失败");
            return Ok(ApiResponse::of(-1, msg, json!([])));
        }

        let data = raw
            .get("jData")
            .cloned()
            .ok_or_else(|| DeltaError::Parse("资产数据格式异常".to_string()))?;
        Ok(ApiResponse::ok("获取成功", data))
    }

    pub async fn get_logs(
        &self,
        auth: &GameAuth,
        log_type: i64,
        page: i64,
    ) -> Result<Value, DeltaError> {
        let param = json!({
            "type": log_type,
            "page": page,
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        let body = self.ide(IdeCall::new(319386, "zMemOt", param)).await?;
        if log_type == 3 {
            return Ok(json!([{ "totalMoney": body["data"][0]["totalMoney"].clone() }]));
        }
        Ok(body)
    }

    pub async fn get_recent(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "resourceType": "sol",
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(316969, "NoOapI", param).with_method("dfm/center.recent.detail"))
            .await
    }

    pub async fn get_achievement(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "resourceType": "sol",
            "seasonid": [1, 2, 3, 4, 5],
            "isAllSeason": true,
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(316969, "NoOapI", param).with_method("dfm/center.person.resource"))
            .await
    }

    pub async fn get_password(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        let data = self
            .ide(IdeCall::new(352143, "YWRywA", param).with_method("dfm/center.day.secret"))
            .await?;

        let mut out = serde_json::Map::new();
        let items = data
            .as_array()
            .or_else(|| data.get("data").and_then(Value::as_array));
        if let Some(items) = items {
            for item in items {
                if let (Some(name), Some(secret)) =
                    (item["mapName"].as_str(), item["secret"].as_str())
                {
                    out.insert(name.to_string(), Value::String(secret.to_string()));
                }
            }
        }
        Ok(Value::Object(out))
    }

    pub async fn get_manufacture(&self, auth: &GameAuth) -> Result<Value, DeltaError> {
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });
        self.ide(IdeCall::new(365589, "bQaMCQ", param).with_source("5"))
            .await
    }

    pub async fn get_guns(&self, gun_id: &str) -> Result<Value, DeltaError> {
        let param = json!({
            "primary": "gun",
            "second": "gunRifle",
            "objectID": gun_id,
        });
        let mut body = self.ide(IdeCall::new(352143, "YWRywA", param)).await?;
        if let Some(list) = body.as_array_mut() {
            for gun in list.iter_mut() {
                enrich_gun_detail(gun, &self.ammo_config, &self.accessory_config);
            }
        } else if let Some(list) = body["data"]["list"].as_array_mut() {
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
        let checkparam = parsed
            .get("checkparam")
            .filter(|value| !value.is_empty())
            .cloned()
            .ok_or_else(|| DeltaError::Parse("角色绑定缺少 checkparam".to_string()))?;
        let md5str = parsed
            .get("md5str")
            .filter(|value| !value.is_empty())
            .cloned()
            .ok_or_else(|| DeltaError::Parse("角色绑定缺少 md5str".to_string()))?;
        let role_id = checkparam
            .split('|')
            .nth(2)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| DeltaError::Parse("角色绑定缺少 role_id".to_string()))?
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use mockito::Server;
    use reqwest::{cookie::Jar, Client};
    use serde_json::{json, Value};
    use url::form_urlencoded::Serializer;

    use super::{GameAuth, GameService};
    use crate::delta::{constants::DF_REFERER, utils::game::AmmoItem};

    fn make_service(
        ide_gateway: String,
        ammo_config: HashMap<String, Vec<AmmoItem>>,
        accessory_config: HashMap<String, String>,
    ) -> GameService {
        GameService {
            client: Client::new(),
            jar: Arc::new(Jar::default()),
            ammo_config,
            accessory_config,
            ide_gateway,
        }
    }

    fn sample_auth() -> GameAuth {
        GameAuth {
            openid: "openid-1".to_string(),
            access_token: "access-token-1".to_string(),
            acctype: "qq".to_string(),
        }
    }

    fn ide_form(
        chart_id: u64,
        ide_token: &str,
        method: Option<&str>,
        source: Option<&str>,
        param: Value,
    ) -> Vec<u8> {
        let mut serializer = Serializer::new(String::new());
        serializer.append_pair("iChartId", &chart_id.to_string());
        serializer.append_pair("sIdeToken", ide_token);
        if let Some(method) = method {
            serializer.append_pair("method", method);
        }
        if let Some(source) = source {
            serializer.append_pair("source", source);
        }
        serializer.append_pair("param", &serde_json::to_string(&param).unwrap());
        serializer.finish().into_bytes()
    }

    #[tokio::test]
    async fn get_price_fetches_latest_and_recent_prices() {
        let mut server = Server::new_async().await;
        let latest_param = json!({ "ids": [37100500001_i64, 37100500002_i64] });
        let recent_first_param = json!({ "objectID": "37100500001" });
        let recent_second_param = json!({ "objectID": "37100500002" });

        let latest_mock = server
            .mock("POST", "/ide/")
            .match_header("referer", DF_REFERER)
            .match_body(ide_form(
                352143,
                "YWRywA",
                Some("dfm/object.price.latest"),
                None,
                latest_param,
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "37100500001": { "avgPrice": 12345 },
                    "37100500002": { "avgPrice": 67890 }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let recent_first_mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                Some("dfm/object.price.recent"),
                None,
                recent_first_param,
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "objectPriceRecent": { "list": [{ "price": 111 }] } }).to_string())
            .create_async()
            .await;

        let recent_second_mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                Some("dfm/object.price.recent"),
                None,
                recent_second_param,
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "objectPriceRecent": { "list": [{ "price": 222 }] } }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service
            .get_price(vec![37100500001, 37100500002], true)
            .await
            .unwrap();

        assert_eq!(data["37100500001"]["avgPrice"], 12345);
        assert_eq!(data["37100500001"]["recent"][0]["price"], 111);
        assert_eq!(data["37100500002"]["recent"][0]["price"], 222);
        latest_mock.assert_async().await;
        recent_first_mock.assert_async().await;
        recent_second_mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_price_rejects_missing_recent_list() {
        let mut server = Server::new_async().await;

        server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                Some("dfm/object.price.latest"),
                None,
                json!({ "ids": [37100500001_i64] }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "37100500001": { "avgPrice": 12345 } }).to_string())
            .create_async()
            .await;

        server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                Some("dfm/object.price.recent"),
                None,
                json!({ "objectID": "37100500001" }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "objectPriceRecent": {} }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let error = service
            .get_price(vec![37100500001], true)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("物价近期数据格式异常"));
    }

    #[tokio::test]
    async fn get_assets_maps_special_restriction_error() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });

        let mock = server
            .mock("POST", "/ide/")
            .match_header("referer", DF_REFERER)
            .match_body(ide_form(318948, "Plaqzy", None, None, param))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "ret": -4000 }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let response = service.get_assets(&auth).await.unwrap();

        assert_eq!(response.code, -1);
        assert_eq!(response.msg, "您的账号由于腾讯内部错误无法使用这个功能");
        assert_eq!(response.data, json!([]));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_assets_maps_application_error() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });

        server
            .mock("POST", "/ide/")
            .match_body(ide_form(318948, "Plaqzy", None, None, param))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "ret": -1, "msg": "登录态失效" }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let response = service.get_assets(&auth).await.unwrap();

        assert_eq!(response.code, -1);
        assert_eq!(response.msg, "登录态失效");
        assert_eq!(response.data, json!([]));
    }

    #[tokio::test]
    async fn get_assets_requires_jdata_on_success() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();
        let param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });

        server
            .mock("POST", "/ide/")
            .match_body(ide_form(318948, "Plaqzy", None, None, param))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "ret": 0 }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let error = service.get_assets(&auth).await.unwrap_err();
        assert!(error.to_string().contains("资产数据格式异常"));
    }

    #[tokio::test]
    async fn get_player_uses_wallet_endpoint_and_decodes_name() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();
        let base_param = json!({
            "openid": auth.openid,
            "access_token": auth.access_token,
            "acctype": auth.acctype_api(),
        });

        let base_mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(317814, "QIRBwm", None, None, base_param))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "data": { "charac_name": "Alice%20Bob" } }).to_string())
            .create_async()
            .await;

        let wallet_expectations = [
            ("coin", 17888808888_i64, 100_i64),
            ("tickets", 17888808889_i64, 200_i64),
            ("money", 17020000010_i64, 300_i64),
        ];

        for (_, object_id, total_money) in wallet_expectations {
            server
                .mock("POST", "/ide/")
                .match_body(ide_form(
                    319386,
                    "zMemOt",
                    None,
                    None,
                    json!({
                        "type": 3,
                        "page": 1,
                        "itemId": object_id,
                        "openid": auth.openid,
                        "access_token": auth.access_token,
                        "acctype": auth.acctype_api(),
                    }),
                ))
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(json!({ "data": [{ "totalMoney": total_money }] }).to_string())
                .create_async()
                .await;
        }

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_player(&auth).await.unwrap();

        assert_eq!(data["data"]["charac_name"], "Alice Bob");
        assert_eq!(data["coin"], 100);
        assert_eq!(data["tickets"], 200);
        assert_eq!(data["money"], 300);
        base_mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_logs_type_three_returns_total_money_only() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                319386,
                "zMemOt",
                None,
                None,
                json!({
                    "type": 3,
                    "page": 1,
                    "openid": auth.openid,
                    "access_token": auth.access_token,
                    "acctype": auth.acctype_api(),
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "data": [{ "totalMoney": 88 }, { "totalMoney": 99 }] }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_logs(&auth, 3, 1).await.unwrap();

        assert_eq!(data, json!([{ "totalMoney": 88 }]));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_recent_uses_documented_chart_and_method() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                316969,
                "NoOapI",
                Some("dfm/center.recent.detail"),
                None,
                json!({
                    "resourceType": "sol",
                    "openid": auth.openid,
                    "access_token": auth.access_token,
                    "acctype": auth.acctype_api(),
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "data": { "solDetail": [] } }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_recent(&auth).await.unwrap();

        assert_eq!(data["data"]["solDetail"], json!([]));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_achievement_uses_documented_payload() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                316969,
                "NoOapI",
                Some("dfm/center.person.resource"),
                None,
                json!({
                    "resourceType": "sol",
                    "seasonid": [1, 2, 3, 4, 5],
                    "isAllSeason": true,
                    "openid": auth.openid,
                    "access_token": auth.access_token,
                    "acctype": auth.acctype_api(),
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "data": { "points": 7 } }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_achievement(&auth).await.unwrap();

        assert_eq!(data["data"]["points"], 7);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_password_folds_secret_list_into_map() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                Some("dfm/center.day.secret"),
                None,
                json!({
                    "openid": auth.openid,
                    "access_token": auth.access_token,
                    "acctype": auth.acctype_api(),
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!([
                    { "mapName": "零号大坝", "secret": "1234" },
                    { "mapName": "长弓溪谷", "secret": "5678" }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_password(&auth).await.unwrap();

        assert_eq!(data, json!({ "零号大坝": "1234", "长弓溪谷": "5678" }));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_manufacture_uses_documented_chart_and_source() {
        let mut server = Server::new_async().await;
        let auth = sample_auth();

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                365589,
                "bQaMCQ",
                None,
                Some("5"),
                json!({
                    "openid": auth.openid,
                    "access_token": auth.access_token,
                    "acctype": auth.acctype_api(),
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "data": [{ "status": "busy" }] }).to_string())
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_manufacture(&auth).await.unwrap();

        assert_eq!(data["data"][0]["status"], "busy");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_guns_is_no_auth_and_enriches_details() {
        let mut server = Server::new_async().await;
        let mut ammo_config = HashMap::new();
        ammo_config.insert(
            "ammo7.62x51".to_string(),
            vec![AmmoItem {
                name: "7.62x51mm M61".to_string(),
                grade: 6,
            }],
        );
        let mut accessory_config = HashMap::new();
        accessory_config.insert("8".to_string(), "前握把".to_string());

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                None,
                None,
                json!({
                    "primary": "gun",
                    "second": "gunRifle",
                    "objectID": "gun-akm",
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!([
                    {
                        "gunDetail": {
                            "caliber": "7.62x51",
                            "ammo": [{ "objectID": 1 }],
                            "accessory": [{ "slotID": 8 }],
                            "allAccessory": []
                        }
                    }
                ])
                .to_string(),
            )
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            ammo_config,
            accessory_config,
        );
        let data = service.get_guns("gun-akm").await.unwrap();

        assert_eq!(data[0]["gunDetail"]["caliber"], "ammo7.62x51");
        assert_eq!(data[0]["gunDetail"]["ammo"][0]["name"], "7.62x51mm M61");
        assert_eq!(data[0]["gunDetail"]["accessory"][0]["name"], "前握把");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_recommendation_uses_documented_place_payload() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/ide/")
            .match_body(ide_form(
                352143,
                "YWRywA",
                None,
                None,
                json!({
                    "type": "place",
                    "place": "tech",
                    "hasPriceData": true,
                }),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({ "data": { "tech": { "list": [{ "name": "合金板" }] } } }).to_string(),
            )
            .create_async()
            .await;

        let service = make_service(
            format!("{}/ide/", server.url()),
            HashMap::new(),
            HashMap::new(),
        );
        let data = service.get_recommendation("tech").await.unwrap();

        assert_eq!(data, json!([{ "name": "合金板" }]));
        mock.assert_async().await;
    }
}
