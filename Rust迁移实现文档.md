# DeltaForce API Rust迁移实现文档

## 1. 概述

本文基于以下两类事实来源整理：

- 对外 API 文档：`三角洲行动API.md`
- PHP 实现：`app/controller/QQ.php`、`Wechat.php`、`QQSafe.php`、`Wegame.php`、`Game.php`、`app/common.php`

目标是将当前 PHP 版 DeltaForce API 迁移为 Rust 实现，并保留现有业务行为、登录状态流、Cookie 处理、错误语义与对外路径。

本文只覆盖以下 5 个模块：

- `QQ鉴权`
- `微信鉴权`
- `Wegame鉴权`
- `游戏数据`
- `QQ安全中心`

本文明确排除所有在 API 文档中标记为废弃的接口，不纳入 Rust 实现。

## 2. 迁移范围与排除项

### 2.1 纳入范围

#### QQ鉴权

- `GET /qq/sig`
- `POST /qq/status`
- `POST /qq/access`
- `POST /qq/update_access`

#### 微信鉴权

- `GET /wechat/login`
- `GET /wechat/status`
- `GET /wechat/access`
- `POST /wechat/update_access`

#### Wegame鉴权

- `POST /wegame/gift`
- `POST /wegame/card`
- `GET /wegame/qq/sig`
- `POST /wegame/qq/status`
- `POST /wegame/qq/access`
- `GET /wegame/wechat/login`
- `GET /wegame/wechat/status`
- `GET /wegame/wechat/access`

#### 游戏数据

- `GET /game/record`
- `GET /game/items`
- `GET /game/config`
- `GET /game/player`
- `GET /game/price`
- `GET /game/assets`
- `GET /game/logs`
- `GET /game/recent`
- `GET /game/achievement`
- `GET /game/password`
- `GET /game/manufacture`
- `GET /game/guns`
- `GET /game/bind`
- `GET /game/firearmModList`
- `GET /game/recommendation`

#### QQ安全中心

- `GET /qqsafe/sig`
- `POST /qqsafe/status`
- `POST /qqsafe/access`
- `GET /qqsafe/bannedList`

### 2.2 排除项

- `GET /game/test`
  - 文档位置：`三角洲行动API.md:56316`
  - 接口状态：`已废弃`
- `GET /qqsafe/report`
  - 文档位置：`三角洲行动API.md:64306`
  - 接口状态：`已废弃`

## 3. Rust 项目结构建议

建议按“公共能力 + 模块客户端 + HTTP 入口层”拆分：

```text
src/
  main.rs
  lib.rs
  error.rs
  response.rs
  constants.rs
  models/
    mod.rs
    auth.rs
    game.rs
    qqsafe.rs
    wegame.rs
  utils/
    mod.rs
    time.rs
    hashes.rs
    jsonp.rs
    html.rs
    cookies.rs
    encoding.rs
  client/
    mod.rs
    http_client.rs
    ide_client.rs
  services/
    mod.rs
    qq_auth.rs
    wechat_auth.rs
    wegame_auth.rs
    game.rs
    qq_safe.rs
  handlers/
    mod.rs
    qq.rs
    wechat.rs
    wegame.rs
    game.rs
    qqsafe.rs
```

如果采用 `axum`，可以让 `handlers/*` 仅负责：

- 参数解析
- 调用 service
- 统一包装 `{ code, msg, data }`

业务逻辑尽量都放在 `services/*`。

## 4. Cargo.toml 与依赖说明

```toml
[package]
name = "deltaforce-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
thiserror = "2"
axum = { version = "0.8", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "gzip", "brotli", "deflate", "cookies"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
scraper = "0.24"
base64 = "0.22"
url = "2"
cookie = "0.18"
encoding_rs = "0.8"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
```

说明：

- `reqwest + cookies` 用于维持登录态。
- `rustls-tls` 默认启用正常 TLS 校验，和 PHP 当前 `verify=false` 不同，但更安全。
- `regex` 负责解析 `ptuiCB(...)`、`coolxitech(...)`、JS 对象片段。
- `scraper` 用于微信二维码页面 HTML 提取。
- `encoding_rs` 用于 `bind()` 里的 `GBK -> UTF-8` 转码。

## 5. 公共模块设计

### 5.1 统一响应结构

PHP 当前统一返回：

```json
{ "code": 0, "msg": "获取成功", "data": {} }
```

Rust 建议保留完全一致的出参格式：

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub msg: String,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(msg: impl Into<String>, data: T) -> Self {
        Self { code: 0, msg: msg.into(), data }
    }
}
```

### 5.2 错误类型

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing parameter: {0}")]
    MissingParam(&'static str),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("regex mismatch: {0}")]
    Regex(&'static str),
    #[error("cookie not found: {0}")]
    CookieNotFound(&'static str),
    #[error("business error: {0}")]
    Business(String),
}
```

### 5.3 毫秒时间戳

对应 PHP `getMicroTime()`：

```rust
pub fn current_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
```

### 5.4 QQ 二维码 Token 算法

对应 PHP `getQrToken(qrSig)`：

```rust
pub fn get_qr_token(qr_sig: &str) -> i64 {
    let mut hash: i64 = 0;
    for ch in qr_sig.bytes() {
        hash += (hash << 5) + i64::from(ch);
    }
    hash & 0x7fff_ffff
}
```

### 5.5 GTK 算法

对应 PHP `getGTK(sKey)`：

```rust
pub fn get_gtk(s_key: &str) -> i64 {
    let mut hash: i64 = 5381;
    for ch in s_key.bytes() {
        hash += (hash << 5) + i64::from(ch);
    }
    hash & 0x7fff_ffff
}
```

### 5.6 JSONP 解析

QQ 登录与 AccessToken 兑换会返回 `ptuiCB(...)` 或 `coolxitech(...)`：

```rust
use regex::Regex;

pub fn extract_jsonp_args(body: &str, callback: &str) -> anyhow::Result<Vec<String>> {
    let pattern = format!(r"{}\((?P<body>.*)\)", regex::escape(callback));
    let re = Regex::new(&pattern)?;
    let caps = re.captures(body).ok_or_else(|| anyhow::anyhow!("callback not found"))?;
    let inner = caps.name("body").unwrap().as_str();
    let mut values = Vec::new();
    for part in inner.split(',') {
        values.push(part.trim().trim_matches('"').trim_matches('\'').to_string());
    }
    Ok(values)
}
```

### 5.7 Cookie 操作

PHP 广泛依赖跨域 CookieJar 注入，Rust 也必须保留：

```rust
use cookie::Cookie;
use reqwest::cookie::Jar;
use std::sync::Arc;
use url::Url;

pub fn insert_cookie(jar: &Arc<Jar>, domain: &str, name: &str, value: &str) {
    let url = Url::parse(domain).expect("valid url");
    let raw = Cookie::build((name.to_string(), value.to_string()))
        .path("/")
        .build()
        .to_string();
    jar.add_cookie_str(&raw, &url);
}
```

建议统一约定域名常量：

- `https://xui.ptlogin2.qq.com/`
- `https://ssl.ptlogin2.qq.com/`
- `https://graph.qq.com/`
- `https://ams.game.qq.com/`
- `https://open.weixin.qq.com/`
- `https://lp.open.weixin.qq.com/`
- `https://www.wegame.com.cn/`
- `https://gamesafe.qq.com/`

### 5.8 HTTP Client 建议

PHP 当前行为：

- 根据 cURL 能力决定 HTTP/2
- `allow_redirects=false`
- `verify=false`
- 启用 CookieJar

Rust 建议：

```rust
use reqwest::{cookie::Jar, redirect::Policy, Client};
use std::sync::Arc;
use std::time::Duration;

pub fn build_http_client(jar: Arc<Jar>) -> anyhow::Result<Client> {
    let client = Client::builder()
        .cookie_provider(jar)
        .redirect(Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 DeltaForce-Rust/0.1")
        .build()?;
    Ok(client)
}
```

说明：不建议默认关闭 TLS 校验。如果个别环境确实需要跳过证书验证，建议以显式配置开关开启，而不是默认行为。

### 5.9 Cookie 辅助函数

服务层多处依赖以下工具：

```rust
use reqwest::cookie::CookieStore;
use std::collections::HashMap;

/// 将客户端传入的 cookie JSON 串（可能含反斜杠转义）解析为 name->value 映射。
/// 对应 PHP `json_decode(stripslashes($cookie), true)`。
pub fn restore_cookie_json(raw: &str) -> anyhow::Result<HashMap<String, String>> {
    let cleaned = raw.replace('\\', "");
    let map: HashMap<String, String> = serde_json::from_str(&cleaned)?;
    Ok(map)
}

/// 从 cookie map 中取出必要字段，缺失时返回错误（对应 PHP 多处 isset 判断）
pub fn must_cookie<'a>(map: &'a HashMap<String, String>, name: &str) -> anyhow::Result<&'a str> {
    map.get(name)
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing cookie field: {name}"))
}

/// 将 Jar 中指定 URL 下的 cookie 输出为 name->value map（对应 PHP `getCookieValue`）
pub fn dump_cookies(jar: &reqwest::cookie::Jar, url: &url::Url) -> HashMap<String, String> {
    let header = jar.cookies(url).map(|h| h.to_str().unwrap_or("").to_string()).unwrap_or_default();
    let mut out = HashMap::new();
    for kv in header.split(';') {
        if let Some((k, v)) = kv.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}
```

### 5.10 URL / HTML / JSONP / JWT 解析工具

```rust
/// 从 URL query 中取参数（用于解析跳转 Location 中的 uin、code 等）
pub fn extract_query_param(url: &str, key: &str) -> Option<String> {
    url::Url::parse(url).ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

/// 微信 /connect/l/qrconnect 轮询响应里的 `wx_errcode=xxx`
pub fn extract_wx_errcode(body: &str) -> Option<i32> {
    let re = regex::Regex::new(r"wx_errcode\s*=\s*(-?\d+)").ok()?;
    re.captures(body)?.get(1)?.as_str().parse().ok()
}

/// 微信 /connect/l/qrconnect 轮询响应里的 `wx_code='xxx'`
pub fn extract_wx_code(body: &str) -> Option<String> {
    let re = regex::Regex::new(r#"wx_code\s*=\s*['"]([^'"]*)['"]"#).ok()?;
    re.captures(body).map(|c| c[1].to_string())
}

/// 微信扫码登录 HTML 页面中的二维码图片路径 `/connect/qrcode/<uuid>`
pub fn extract_wechat_qr(html: &str) -> Option<(String, String)> {
    let re = regex::Regex::new(r"/connect/qrcode/([A-Za-z0-9_-]+)").ok()?;
    let cap = re.captures(html)?;
    let uuid = cap[1].to_string();
    let url = format!("https://open.weixin.qq.com/connect/qrcode/{uuid}");
    Some((url, uuid))
}

/// QQ 安全中心 `gs_code` 的 JWT 风格 `x.<base64>.y` 中段 JSON 解析
pub fn decode_jwt_middle(token: &str) -> anyhow::Result<serde_json::Value> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 { anyhow::bail!("invalid gs_code format"); }
    let raw = URL_SAFE_NO_PAD.decode(parts[1])?;
    let v: serde_json::Value = serde_json::from_slice(&raw)?;
    Ok(v)
}
```

### 5.11 绑定角色 JS 片段解析 + GBK 转码

`Game.php#bind()` 请求 `https://comm.aci.game.qq.com/main` 返回的是类 JS 对象文本（非 JSON），其中 `msg` 为 GBK 编码。

```rust
use encoding_rs::GBK;
use std::collections::HashMap;

/// 提取形如 `ams.gameattr.role = {a:'v', b:'v'}` 里的键值；对 msg 做 GBK→UTF-8 转码
pub fn parse_bind_role_js(raw: &str) -> anyhow::Result<HashMap<String, String>> {
    let block_re = regex::Regex::new(r"\{[^{}]*\}")?;
    let block = block_re
        .find(raw)
        .ok_or_else(|| anyhow::anyhow!("no role block"))?
        .as_str();
    let kv_re = regex::Regex::new(r#"['"]?(\w+)['"]?\s*:\s*['"]([^'"]*)['"]"#)?;
    let mut map = HashMap::new();
    for cap in kv_re.captures_iter(block) {
        let key = cap[1].to_string();
        let mut value = cap[2].to_string();
        if key == "msg" {
            // PHP: iconv('GBK','UTF-8',msg)
            let bytes: Vec<u8> = value.bytes().collect();
            let (cow, _, _) = GBK.decode(&bytes);
            value = cow.into_owned();
        }
        map.insert(key, value);
    }
    Ok(map)
}
```

> 注意：当响应实际是被 HTTP 解码成 UTF-8 字符串时，上面通过 `value.bytes()` 重新拿 latin-1 字节已经丢信息。生产实现应在 reqwest 层用 `resp.bytes().await?` 拿原始 GBK 字节，再整体用 `GBK.decode()` 转 UTF-8，最后再做正则。这里给出的是示意流程，最终实现应在请求层做编码切换。

### 5.12 口径标准化 / 枪械配置类型 / 鉴权载荷

```rust
/// 对应 PHP `normalizeCaliberCode`：把形如 "7.62x51" 变成 "ammo7.62x51"
pub fn normalize_caliber_code(raw: &str) -> String {
    if raw.starts_with("ammo") { raw.to_string() } else { format!("ammo{raw}") }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AmmoItem {
    pub name: String,
    pub grade: i32,
}
pub type AmmoConfig = std::collections::HashMap<String, Vec<AmmoItem>>; // key: "ammo7.62x51"
pub type AccessoryConfig = std::collections::HashMap<String, String>;    // key: slotID -> 中文名

/// 游戏数据接口所需的鉴权上下文
#[derive(Debug, Clone)]
pub struct GameAuth {
    pub openid: String,
    pub access_token: String,
    /// header `acctype`：`qc` 或 `wx`；PHP 里非 "wx" 一律按 "qc"
    pub acctype: String,
}
impl GameAuth {
    pub fn is_qq(&self) -> bool { self.acctype != "wx" }
}
```

### 5.13 IDE 通用请求封装

Game 控制器所有走 `https://comm.ams.game.qq.com/ide/` 的接口共用以下签名与形参约定。`method`/`source` 必须作为表单一级字段，不能塞到 `param` 内。

```rust
use serde_json::Value;

pub struct IdeCall<'a> {
    pub chart_id: &'a str,    // iChartId & iSubChartId
    pub ide_token: &'a str,   // sIdeToken
    pub method: Option<&'a str>,    // e.g. "dfm/object.list"；无则不填
    pub source: Option<&'a str>,    // e.g. "2"；无则不填
    pub param: Value,         // 业务 param 对象，最终序列化成字符串
    pub extra: Vec<(&'a str, String)>, // 额外表单字段（如 type/page）
    pub auth: Option<&'a GameAuth>,    // 需要鉴权时注入 cookie
}

/// 统一调用 IDE 网关
pub async fn ide_call(client: &reqwest::Client, call: IdeCall<'_>) -> anyhow::Result<Value> {
    let mut form: Vec<(String, String)> = Vec::new();
    form.push(("iChartId".into(), call.chart_id.into()));
    form.push(("iSubChartId".into(), call.chart_id.into()));
    form.push(("sIdeToken".into(), call.ide_token.into()));
    if let Some(m) = call.method { form.push(("method".into(), m.into())); }
    if let Some(s) = call.source { form.push(("source".into(), s.into())); }
    form.push(("param".into(), call.param.to_string()));
    for (k, v) in call.extra { form.push((k.to_string(), v)); }

    // 若需要鉴权，由调用方先通过 insert_cookie() 把
    //   openid / access_token / acctype / appid=101491592
    // 写入 `.qq.com` 域，再调用此函数。
    let resp = client
        .post("https://comm.ams.game.qq.com/ide/")
        .header("Referer", "https://df.qq.com/")
        .form(&form)
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    Ok(body)
}
```

### 5.14 统一请求头（Referer / User-Agent / Origin）规约

为减少腾讯侧风控误判，所有服务层请求应按下表补齐头。缺 Referer 会导致二维码/登录/游戏数据批量 500/403。

| 目标域 | Referer | Origin | User-Agent |
| --- | --- | --- | --- |
| `df.qq.com` / `comm.ams.game.qq.com` / `apps.game.qq.com` | `https://df.qq.com/` | — | 默认浏览器 UA |
| `graph.qq.com` / `ssl.ptlogin2.qq.com` / `xui.ptlogin2.qq.com` | `https://xui.ptlogin2.qq.com/` | — | 默认浏览器 UA |
| `open.weixin.qq.com` / `lp.open.weixin.qq.com` | `https://df.qq.com/` | — | 默认浏览器 UA |
| `www.wegame.com.cn` | `https://www.wegame.com.cn/` | `https://www.wegame.com.cn` | 默认浏览器 UA |
| `gamesafe.qq.com` | `https://gamesafe.qq.com/` | — | 默认浏览器 UA |
| `wx.gamesafe.qq.com`（已废弃 report，仅供历史参考） | — | — | 必须带 `MicroMessenger` 标识 |
| `comm.aci.game.qq.com`（get_bind） | `https://df.qq.com/` | — | 默认浏览器 UA |

建议用一个中心化的 `apply_default_headers(req, target_host)` 函数统一注入，避免每个服务层重复写。

### 5.15 Redirect 策略与手动跟随 + TLS 开关

PHP Guzzle 显式 `allow_redirects=false`，登录链路依赖读取 `Location` 并在业务层手动 GET 跳转地址以让 Cookie 落到 `.qq.com` / `.wegame.com.cn` / `.ptlogin2.qq.com`。Rust 对应实现：

```rust
use reqwest::{Client, Response, StatusCode};

pub async fn follow_redirect_chain(client: &Client, start: Response) -> anyhow::Result<Response> {
    let mut current = start;
    for _ in 0..5 {
        let status = current.status();
        if !(status == StatusCode::FOUND
            || status == StatusCode::MOVED_PERMANENTLY
            || status == StatusCode::SEE_OTHER
            || status == StatusCode::TEMPORARY_REDIRECT)
        {
            break;
        }
        let loc = current
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("redirect without Location"))?
            .to_string();
        // 关键：这一步 GET 的目的不是拿响应体，而是让 Cookie 被 CookieJar 吸收
        current = client.get(&loc).send().await?;
    }
    Ok(current)
}
```

关于 TLS：PHP `verify=false` 是宽松策略，Rust 默认应保留校验。若部署环境确实必须关闭（例如内网代理），用显式开关：

```rust
pub struct HttpOptions {
    /// 明确命名为 insecure，禁止用“便捷”/“兼容”这类模糊词
    pub insecure_skip_tls_verify: bool,
}

pub fn build_http_client_with(jar: std::sync::Arc<reqwest::cookie::Jar>, opts: HttpOptions)
    -> anyhow::Result<Client>
{
    let mut b = Client::builder()
        .cookie_provider(jar)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 DeltaForce-Rust/0.1");
    if opts.insecure_skip_tls_verify {
        b = b.danger_accept_invalid_certs(true);
    }
    Ok(b.build()?)
}
```

> 运维约束：`insecure_skip_tls_verify=true` 只能由配置/环境变量显式开启，不得作为代码默认值。生产应保持证书校验。

## 6. QQ鉴权

### 6.1 获取登录二维码 / `get_login_qr`

- 所属模块：`QQ鉴权`
- HTTP 方法与路径：`GET /qq/sig`
- 请求参数：无
- 请求头：无强制要求
- 请求体示例：无

#### 响应结构

- `code`: `0` 表示成功
- `msg`: `获取成功`
- `data.qrSig`: 二维码签名
- `data.image`: base64 二维码图片
- `data.token`: `get_qr_token(qrSig)` 结果
- `data.loginSig`: 登录签名
- `data.cookie`: 首次初始化得到的 Cookie 集合

#### 成功示例

```json
{
  "code": 0,
  "msg": "获取成功",
  "data": {
    "qrSig": "...",
    "image": "base64...",
    "token": 1135289816,
    "loginSig": "...",
    "cookie": {
      "pt_login_sig": "...",
      "qrsig": "..."
    }
  }
}
```

#### 失败示例

```json
{
  "code": -1,
  "msg": "获取二维码失败",
  "data": []
}
```

#### 错误码

- `0`: 成功
- `-1`: 初始化流程失败

#### PHP 业务逻辑提炼

- 先访问 `xlogin` 初始化登录页 Cookie。
- 再访问 `ptqrshow` 获取二维码图片。
- 从 Cookie 中提取 `qrsig` 与 `pt_login_sig`。
- 使用 `get_qr_token(qrSig)` 生成轮询令牌。
- 返回二维码图片、令牌、登录签名和原始 Cookie。

#### Rust 代码片段

```rust
pub async fn get_login_qr(&self) -> anyhow::Result<QqLoginQr> {
    self.client
        .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
        .query(&[
            ("appid", "716027609"),
            ("daid", "383"),
            ("pt_3rd_aid", "101491592"),
            ("s_url", "https://graph.qq.com/oauth2.0/login_jump"),
        ])
        .send()
        .await?
        .error_for_status()?;

    let image_bytes = self.client
        .get("https://xui.ptlogin2.qq.com/ssl/ptqrshow")
        .query(&[("appid", "716027609"), ("daid", "383")])
        .send()
        .await?
        .bytes()
        .await?;

    let qr_sig = self.must_cookie("https://xui.ptlogin2.qq.com/", "qrsig")?;
    let login_sig = self.must_cookie("https://xui.ptlogin2.qq.com/", "pt_login_sig")?;

    Ok(QqLoginQr {
        qr_sig: qr_sig.clone(),
        image: base64::encode(image_bytes),
        token: get_qr_token(&qr_sig),
        login_sig,
        cookie: self.dump_cookies("https://xui.ptlogin2.qq.com/")?,
    })
}
```

### 6.2 获取登录状态 / `poll_login_status`

- 所属模块：`QQ鉴权`
- HTTP 方法与路径：`POST /qq/status`
- 请求参数：
  - `qrToken` `string` 必填
  - `qrSig` `string` 必填
  - `loginSig` `string` 必填
- 请求头：默认即可
- 请求体：
  - `cookie` `string`，JSON 字符串

#### 响应结构

- 成功时 `code=0`，返回登录后 Cookie
- 等待扫码 `code=1`
- 已扫码待确认 `code=2`
- 二维码失效 `code=-2`
- 登录被拒绝 `code=-3`
- 其他错误 `code=-4`

#### 成功示例

```json
{
  "code": 0,
  "msg": "登录成功",
  "data": {
    "cookie": {
      "p_skey": "...",
      "pt4_token": "...",
      "pt_oauth_token": "..."
    }
  }
}
```

#### 失败示例

```json
{
  "code": -2,
  "msg": "二维码失效",
  "data": []
}
```

#### 错误码

- `0`: 登录成功
- `1`: 等待扫码
- `2`: 已扫码待确认
- `-2`: 二维码失效
- `-3`: 登录被拒绝
- `-4`: 未知错误

#### PHP 业务逻辑提炼

- 从请求体拿到 Cookie JSON，必要时补入 `qrsig`。
- 注入 `.ptlogin2.qq.com` 域。
- 请求 `https://ssl.ptlogin2.qq.com/ptqrlogin`。
- 解析返回的 `ptuiCB(...)`。
- 若成功，跟随一次跳转 URL，让关键 Cookie 沉淀到 Jar。
- 从跳转 URL 里提取 `uin`，并保存完整 Cookie 到 `Access` 表。

#### Rust 代码片段

```rust
pub async fn poll_login_status(&self, req: QqStatusRequest) -> anyhow::Result<ApiResponse<serde_json::Value>> {
    self.restore_cookie_json("https://ssl.ptlogin2.qq.com/", &req.cookie)?;
    insert_cookie(&self.jar, "https://ssl.ptlogin2.qq.com/", "qrsig", &req.qr_sig);

    let body = self.client
        .get("https://ssl.ptlogin2.qq.com/ptqrlogin")
        .query(&[
            ("u1", "https://graph.qq.com/oauth2.0/login_jump"),
            ("ptqrtoken", &req.qr_token),
            ("pt_login_sig", &req.login_sig),
        ])
        .send()
        .await?
        .text()
        .await?;

    let args = extract_jsonp_args(&body, "ptuiCB")?;
    match args.first().map(String::as_str) {
        Some("0") => {
            let redirect_url = args.get(2).cloned().unwrap_or_default();
            let _ = self.client.get(&redirect_url).send().await?;
            Ok(ApiResponse::ok("登录成功", serde_json::json!({ "cookie": self.dump_cookies("https://graph.qq.com/")? })))
        }
        Some("66") => Ok(ApiResponse { code: 1, msg: "二维码未失效".into(), data: serde_json::json!([]) }),
        Some("67") => Ok(ApiResponse { code: 2, msg: "已扫码,待确认".into(), data: serde_json::json!([]) }),
        Some("65") => Ok(ApiResponse { code: -2, msg: "二维码失效".into(), data: serde_json::json!([]) }),
        Some("86") => Ok(ApiResponse { code: -3, msg: "登录被拒绝".into(), data: serde_json::json!([]) }),
        _ => Ok(ApiResponse { code: -4, msg: "未知错误信息".into(), data: serde_json::json!([]) }),
    }
}
```

### 6.3 获取访问令牌接口 / `get_access_token`

- 所属模块：`QQ鉴权`
- HTTP 方法与路径：`POST /qq/access`
- 请求参数：无
- 请求头：无特殊要求
- 请求体：
  - `cookie` `string`，与 `qq` 二选一
  - `qq` `string`，可从数据库查回已落库 Cookie

#### 响应结构

- `data.access_token`
- `data.openid`
- `data.expires_in`

#### 成功示例

```json
{
  "code": 0,
  "msg": "获取成功",
  "data": {
    "access_token": "...",
    "openid": "...",
    "expires_in": "7776000"
  }
}
```

#### 失败示例

```json
{
  "code": -1,
  "msg": "AccessToken获取失败",
  "data": []
}
```

#### 错误码

- `0`: 获取成功
- `-1`: 获取失败

#### PHP 业务逻辑提炼

- 接收 Cookie 或通过 `qq` 号查数据库回填 Cookie。
- 将 Cookie 注入 `.qq.com`。
- 从 `p_skey` 计算 `g_tk`。
- POST `https://graph.qq.com/oauth2.0/authorize` 换取 `code`。
- 从 `Location` 中提取 `code`。
- 调业务回跳地址，再访问 `ams.game.qq.com/ams/userLoginSvr` 获取 `access_token/openid`。

#### Rust 代码片段

```rust
pub async fn get_access_token(&self, cookie_json: &str) -> anyhow::Result<QqAccessToken> {
    self.restore_cookie_json("https://graph.qq.com/", cookie_json)?;
    let p_skey = self.must_cookie("https://graph.qq.com/", "p_skey")?;
    let gtk = get_gtk(&p_skey).to_string();

    let resp = self.client
        .post("https://graph.qq.com/oauth2.0/authorize")
        .form(&[
            ("response_type", "code"),
            ("client_id", "101491592"),
            ("redirect_uri", "https://milo.qq.com/comm-htdocs/login/qc_redirect.html?url=https://df.qq.com/"),
            ("g_tk", gtk.as_str()),
        ])
        .send()
        .await?;

    let location = resp.headers().get(reqwest::header::LOCATION).ok_or_else(|| anyhow::anyhow!("missing location"))?;
    let location = location.to_str()?;
    let code = extract_query_param(location, "code")?;

    let _ = self.client.get(location).send().await?;

    let body = self.client
        .get("https://ams.game.qq.com/ams/userLoginSvr")
        .query(&[
            ("a", "qcCodeToOpenId"),
            ("code", code.as_str()),
            ("callback", "coolxitech"),
        ])
        .send()
        .await?
        .text()
        .await?;

    let args = extract_jsonp_args(&body, "coolxitech")?;
    let payload: serde_json::Value = serde_json::from_str(args.first().map(String::as_str).unwrap_or("{}"))?;
    Ok(QqAccessToken {
        access_token: payload["access_token"].as_str().unwrap_or_default().to_string(),
        openid: payload["openid"].as_str().unwrap_or_default().to_string(),
        expires_in: payload["expires_in"].as_i64().unwrap_or_default(),
    })
}
```

### 6.4 更新访问令牌接口 / `update_access_token`

- 所属模块：`QQ鉴权`
- HTTP 方法与路径：`POST /qq/update_access`
- 请求参数：无
- 请求头：无特殊要求
- 请求体：
  - `cookie`
  - `openid`
  - `access_token`

#### 响应结构

- 成功：`code=0`
- 失败：`code=-1`

#### 成功示例

```json
{ "code": 0, "msg": "鉴权仍然有效", "data": [] }
```

#### 失败示例

```json
{ "code": -1, "msg": "鉴权已失效", "data": [] }
```

#### 错误码

- `0`: 仍有效
- `-1`: 已失效

#### PHP 业务逻辑提炼

- 将 Cookie 注入 `.ptlogin2.qq.com`。
- 调用 `ams.userLoginSvr` 检查 `isLogin`。
- `isLogin == 1` 判定为有效。

#### Rust 代码片段

```rust
pub async fn update_access_token(&self, req: UpdateAccessRequest) -> anyhow::Result<bool> {
    self.restore_cookie_json("https://ssl.ptlogin2.qq.com/", &req.cookie)?;
    let body = self.client
        .post("https://ams.game.qq.com/ams/userLoginSvr")
        .query(&[
            ("callback", "coolxitech"),
            ("acctype", "qc"),
            ("appid", "101491592"),
            ("access_token", req.access_token.as_str()),
            ("openid", req.openid.as_str()),
        ])
        .send()
        .await?
        .text()
        .await?;
    Ok(body.contains("\"isLogin\":1"))
}
```

## 7. 微信鉴权

### 7.1 微信扫码登录 / `get_wechat_login_qr`

- 所属模块：`微信鉴权`
- HTTP 方法与路径：`GET /wechat/login`
- 请求参数：无
- 请求头：建议携带 `referer: https://df.qq.com/`
- 请求体示例：无

#### 响应结构

- `data.qrCode`: 微信二维码链接
- `data.uuid`: 登录轮询唯一值

#### 成功示例

```json
{
  "code": 0,
  "msg": "获取成功",
  "data": {
    "qrCode": "https://open.weixin.qq.com/connect/qrcode/081Gutvi34Tfml26",
    "uuid": "081Gutvi34Tfml26"
  }
}
```

#### 失败示例

```json
{ "code": -1, "msg": "获取二维码失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 请求微信 `qrconnect` 页面。
- 从 HTML 正则提取 `/connect/qrcode/...`。
- 拼出二维码 URL，并把最后一段当成 `uuid`。

#### Rust 代码片段

```rust
pub async fn get_wechat_login_qr(&self) -> anyhow::Result<WechatQr> {
    let html = self.client
        .get("https://open.weixin.qq.com/connect/qrconnect")
        .query(&[
            ("appid", "wxfa0c35392d06b82f"),
            ("scope", "snsapi_login"),
            ("redirect_uri", "https://iu.qq.com/df_pc/df_pc.html"),
            ("state", "1"),
            ("self_redirect", "true"),
        ])
        .header(reqwest::header::REFERER, "https://df.qq.com/")
        .send()
        .await?
        .text()
        .await?;

    let re = regex::Regex::new(r#"/connect/qrcode/(?P<uuid>[A-Za-z0-9]+)"#)?;
    let caps = re.captures(&html).ok_or_else(|| anyhow::anyhow!("uuid not found"))?;
    let uuid = caps.name("uuid").unwrap().as_str().to_string();
    Ok(WechatQr {
        qr_code: format!("https://open.weixin.qq.com/connect/qrcode/{uuid}"),
        uuid,
    })
}
```

### 7.2 登录状态查询 / `poll_wechat_status`

- 所属模块：`微信鉴权`
- HTTP 方法与路径：`GET /wechat/status`
- 请求参数：`uuid` 必填
- 请求头：无特殊要求
- 请求体示例：无

#### 响应结构

- `code=3`: 扫码成功，返回 `wx_code`
- `code=2`: 已扫码未确认
- `code=1`: 等待扫码
- `code=-2`: 二维码超时
- `code=-3`: 扫码被拒绝
- `code=-4`: 其他错误

#### 成功示例

```json
{
  "code": 3,
  "msg": "扫码成功",
  "data": {
    "wx_errcode": 405,
    "wx_code": "..."
  }
}
```

#### 失败示例

```json
{ "code": -2, "msg": "二维码超时", "data": [] }
```

#### 错误码

- `3`: 扫码成功
- `2`: 已扫码
- `1`: 等待扫码
- `-2`: 超时
- `-3`: 拒绝
- `-4`: 其他错误

#### PHP 业务逻辑提炼

- 调用 `lp.open.weixin.qq.com/connect/l/qrconnect`。
- 从返回文本解析 `wx_errcode` 和 `wx_code`。
- 将微信状态码映射为项目自己的 `code/msg`。

#### Rust 代码片段

```rust
pub async fn poll_wechat_status(&self, uuid: &str) -> anyhow::Result<ApiResponse<serde_json::Value>> {
    let body = self.client
        .get("https://lp.open.weixin.qq.com/connect/l/qrconnect")
        .query(&[("uuid", uuid)])
        .send()
        .await?
        .text()
        .await?;

    let errcode = extract_wx_errcode(&body)?;
    let wx_code = extract_wx_code(&body).unwrap_or_default();

    let resp = match errcode {
        405 => ApiResponse { code: 3, msg: "扫码成功".into(), data: serde_json::json!({ "wx_errcode": 405, "wx_code": wx_code }) },
        404 => ApiResponse { code: 2, msg: "已扫码".into(), data: serde_json::json!([]) },
        408 => ApiResponse { code: 1, msg: "等待扫码".into(), data: serde_json::json!([]) },
        402 => ApiResponse { code: -2, msg: "二维码超时".into(), data: serde_json::json!([]) },
        403 => ApiResponse { code: -3, msg: "扫码被拒绝".into(), data: serde_json::json!([]) },
        _ => ApiResponse { code: -4, msg: "其他错误代码".into(), data: serde_json::json!({ "wx_errcode": errcode, "wx_code": wx_code }) },
    };

    Ok(resp)
}
```

### 7.3 获取访问令牌 / `get_wechat_access_token`

- 所属模块：`微信鉴权`
- HTTP 方法与路径：`GET /wechat/access`
- 请求参数：`code` 必填
- 请求头：`referer: https://df.qq.com/`
- 请求体示例：无

#### 响应结构

- `data.access_token`
- `data.refresh_token`
- `data.openid`
- `data.unionid`
- `data.expires_in`

#### 成功示例

```json
{
  "code": 0,
  "msg": "获取成功",
  "data": {
    "access_token": "...",
    "refresh_token": "...",
    "openid": "...",
    "unionid": "...",
    "expires_in": 5184000
  }
}
```

#### 失败示例

```json
{ "code": -1, "msg": "AccessToken获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- GET `apps.game.qq.com/ams/ame/codeToOpenId.php`。
- 参数中包含 `appid`、`wxcode`、`acctype=wx`、`originalUrl`、`wxcodedomain`、时间戳。
- 返回 JSON 里 `sMsg` 仍是一层 JSON 字符串，需要二次反序列化。

#### Rust 代码片段

```rust
pub async fn get_wechat_access_token(&self, code: &str) -> anyhow::Result<WechatAccessToken> {
    let raw: serde_json::Value = self.client
        .get("https://apps.game.qq.com/ams/ame/codeToOpenId.php")
        .query(&[
            ("appid", "wxfa0c35392d06b82f"),
            ("wxcode", code),
            ("originalUrl", "https://df.qq.com/cp/record202410ver/"),
            ("wxcodedomain", "iu.qq.com"),
            ("acctype", "wx"),
            ("_", &current_millis().to_string()),
        ])
        .header(reqwest::header::REFERER, "https://df.qq.com/")
        .send()
        .await?
        .json()
        .await?;

    let nested = raw["sMsg"].as_str().ok_or_else(|| anyhow::anyhow!("missing sMsg"))?;
    Ok(serde_json::from_str(nested)?)
}
```

### 7.4 更新访问令牌接口 / `update_wechat_access_token`

- 所属模块：`微信鉴权`
- HTTP 方法与路径：`POST /wechat/update_access`
- 请求参数：无
- 请求头：无特殊要求
- 请求体：
  - `openid`
  - `access_token`

#### 响应结构

- 成功：`code=0`
- 失败：`code=-1`

#### 成功示例

```json
{ "code": 0, "msg": "鉴权仍然有效", "data": [] }
```

#### 失败示例

```json
{ "code": -1, "msg": "鉴权已失效", "data": [] }
```

#### 错误码

- `0`: 仍有效
- `-1`: 已失效

#### PHP 业务逻辑提炼

- 与 QQ 更新逻辑一致，只是 `acctype=wx` 且 `appid` 改为微信 AppId。

#### Rust 代码片段

```rust
pub async fn update_wechat_access_token(&self, req: UpdateTokenOnlyRequest) -> anyhow::Result<bool> {
    let body = self.client
        .post("https://ams.game.qq.com/ams/userLoginSvr")
        .query(&[
            ("callback", "coolxitech"),
            ("acctype", "wx"),
            ("appid", "wxfa0c35392d06b82f"),
            ("access_token", req.access_token.as_str()),
            ("openid", req.openid.as_str()),
        ])
        .send()
        .await?
        .text()
        .await?;
    Ok(body.contains("\"isLogin\":1"))
}
```

## 8. Wegame鉴权

### 8.1 获取登录二维码 / `get_wegame_qq_login_qr`

- 所属模块：`Wegame鉴权 / QQ登录`
- HTTP 方法与路径：`GET /wegame/qq/sig`
- 请求参数：无
- 请求头：默认即可
- 请求体示例：无

#### 响应结构

- 与 `QQ鉴权 / get_login_qr` 类似，但目标站点是 Wegame 登录。

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "qrSig": "...", "token": 123, "loginSig": "..." } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取二维码失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 仍然是 QQ 扫码体系。
- 区别在于回调 URL、`appid=1600001063`、`daid=733`、`pt_3rd_aid=0`。

#### Rust 代码片段

```rust
pub async fn get_wegame_qq_login_qr(&self) -> anyhow::Result<QqLoginQr> {
    self.client
        .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
        .query(&[
            ("appid", "1600001063"),
            ("daid", "733"),
            ("pt_3rd_aid", "0"),
            ("s_url", "https://www.wegame.com.cn/login/callback.html?t=qq&c=0&a=0"),
        ])
        .send()
        .await?
        .error_for_status()?;
    self.get_login_qr().await
}
```

### 8.2 获取登录状态 / `poll_wegame_qq_status`

- 所属模块：`Wegame鉴权 / QQ登录`
- HTTP 方法与路径：`POST /wegame/qq/status`
- 请求参数：`qrToken`、`qrSig`、`loginSig`
- 请求头：默认即可
- 请求体：`cookie`

#### 响应结构

- 与 QQ 登录状态一致。

#### 成功示例

```json
{ "code": 0, "msg": "登录成功", "data": { "cookie": { "p_skey": "..." } } }
```

#### 失败示例

```json
{ "code": 1, "msg": "二维码未失效", "data": [] }
```

#### 错误码

- `0`: 成功
- `1`: 等待扫码
- `2`: 已扫码待确认
- `-2`: 二维码失效
- `-3`: 登录被拒绝
- `-4`: 未知错误

#### PHP 业务逻辑提炼

- 与 QQ 状态轮询同形。
- 特别之处是 `action` 参数为 `0-0-{millis}`，跳转 URL 指向 Wegame。

#### Rust 代码片段

```rust
// Wegame QQ 状态轮询是单次调用：直接请求 ptqrlogin，自行解析 ptuiCB(...)，
// 不再委托给主 QQ.poll_login_status（u1 / action 均与主 QQ 不同）。
pub async fn poll_wegame_qq_status(&self, req: QqStatusRequest) -> anyhow::Result<ApiResponse<serde_json::Value>> {
    self.restore_cookie_json("https://ssl.ptlogin2.qq.com/", &req.cookie)?;
    let action = format!("0-0-{}", current_millis());
    let body = self.client
        .get("https://ssl.ptlogin2.qq.com/ptqrlogin")
        .query(&[
            ("u1", "https://www.wegame.com.cn/login/callback.html?t=qq&c=0&a=0"),
            ("ptqrtoken", req.qr_token.as_str()),
            ("pt_login_sig", req.login_sig.as_str()),
            ("qrsig", req.qr_sig.as_str()),
            ("action", action.as_str()),
            ("js_ver", "10143"),
            ("js_type", "1"),
            ("login_sig", ""),
            ("pt_vcode_v1", "0"),
            ("pt_verifysession_v1", "0"),
            ("h", "1"),
            ("t", "1"),
            ("g", "1"),
            ("from_ui", "1"),
            ("ptredirect", "0"),
            ("aid", "1600001063"),
            ("daid", "733"),
            ("has_onekey", "1"),
        ])
        .header(reqwest::header::REFERER, "https://xui.ptlogin2.qq.com/")
        .send()
        .await?
        .text()
        .await?;

    // 解析 ptuiCB('code','0','url','0','msg','nickname') 风格回调
    let args = extract_jsonp_args(&body)
        .ok_or_else(|| anyhow::anyhow!("ptuiCB 解析失败"))?;
    let status = args.get(0).cloned().unwrap_or_default();
    match status.as_str() {
        "0" => {
            let redirect = args.get(2).cloned().unwrap_or_default();
            // 成功后跟随一次回跳沉淀 Wegame 侧 Cookie，等待 get_wegame_qq_access_token 消费
            if !redirect.is_empty() {
                let _ = follow_redirect_chain(&self.client, &redirect).await;
            }
            let cookies = dump_cookies(&self.jar, "https://graph.qq.com/");
            Ok(ApiResponse::ok("登录成功", serde_json::json!({ "cookie": cookies })))
        }
        "66" => Ok(ApiResponse::of(1, "二维码未失效", serde_json::json!([]))),
        "67" => Ok(ApiResponse::of(2, "已扫码待确认", serde_json::json!([]))),
        "65" => Ok(ApiResponse::of(-2, "二维码失效", serde_json::json!([]))),
        "86" => Ok(ApiResponse::of(-3, "登录被拒绝", serde_json::json!([]))),
        _ => Ok(ApiResponse::of(-4, "未知错误", serde_json::json!([]))),
    }
}
```

### 8.3 获取访问令牌 / `get_wegame_qq_access_token`

> 注：该接口在 API 文档（`三角洲行动API.md`）中并未显式记载，仅来源于 PHP 源码 `app/controller/Wegame.php::getAccessToken` 与 `route/app.php` 中的 `/wegame/qq/access` 注册；在 Rust 迁移实现中保留该入口，接口签名以 PHP 源码为准。

- 所属模块：`Wegame鉴权 / QQ登录`
- HTTP 方法与路径：`POST /wegame/qq/access`
- 请求参数：无
- 请求头：`Content-Type: application/json`
- 请求体：`cookie`

#### 响应结构

- `data.id`: `tgp_id`
- `data.ticket`: `tgp_ticket`

#### 成功示例

```json
{
  "code": 0,
  "msg": "获取成功",
  "data": {
    "id": "...",
    "ticket": "..."
  }
}
```

#### 失败示例

```json
{ "code": -1, "msg": "AccessToken获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 注入 QQ Cookie 到 `.qq.com`。
- POST `https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_qq`。
- JSON 体里需要 `clienttype=1000005`、`mappid=10001`、`uin`、`sig=p_skey`。
- 成功后从响应中取 `user_id` 和 `wt`。

#### Rust 代码片段

```rust
pub async fn get_wegame_qq_access_token(&self, cookie_json: &str) -> anyhow::Result<WegameTicket> {
    self.restore_cookie_json("https://graph.qq.com/", cookie_json)?;
    let uin = self.must_cookie("https://graph.qq.com/", "uin")?.replace('o', "");
    let p_skey = self.must_cookie("https://graph.qq.com/", "p_skey")?;

    let value: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_qq")
        .json(&serde_json::json!({
            "clienttype": 1000005,
            "mappid": 10001,
            "config_params": { "lang_type": 0 },
            "login_info": {
                "qq_info_type": 6,
                "uin": uin,
                "sig": p_skey
            }
        }))
        .send()
        .await?
        .json()
        .await?;

    Ok(WegameTicket {
        id: value["data"]["user_id"].as_str().unwrap_or_default().to_string(),
        ticket: value["data"]["wt"].as_str().unwrap_or_default().to_string(),
    })
}
```

### 8.4 微信扫码登录 / `get_wegame_wechat_login_qr`

- 所属模块：`Wegame鉴权 / 微信登录`
- HTTP 方法与路径：`GET /wegame/wechat/login`
- 请求参数：无
- 请求头：无特殊要求
- 请求体示例：无

#### 响应结构

- `data.qrCode`
- `data.uuid`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "qrCode": "...", "uuid": "..." } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取二维码失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 与主微信登录相同，但 `appid=wx911818d5d92affa8`，回调地址换成 Wegame。

#### Rust 代码片段

```rust
pub async fn get_wegame_wechat_login_qr(&self) -> anyhow::Result<WechatQr> {
    let html = self.client
        .get("https://open.weixin.qq.com/connect/qrconnect")
        .query(&[
            ("appid", "wx911818d5d92affa8"),
            ("scope", "snsapi_login"),
            ("redirect_uri", "https://www.wegame.com.cn/login/callback.html?t=wx&c=0&a=0"),
            ("state", "1"),
        ])
        .send()
        .await?
        .text()
        .await?;
    extract_wechat_qr(&html)
}
```

### 8.5 登录状态查询 / `poll_wegame_wechat_status`

- 所属模块：`Wegame鉴权 / 微信登录`
- HTTP 方法与路径：`GET /wegame/wechat/status`
- 请求参数：`uuid`
- 请求头：无特殊要求
- 请求体示例：无

#### 响应结构

- 与主微信登录状态一致。

#### 成功示例

```json
{ "code": 3, "msg": "扫码成功", "data": { "wx_code": "..." } }
```

#### 失败示例

```json
{ "code": 1, "msg": "等待扫码", "data": [] }
```

#### 错误码

- `0`: 占位（扫码成功映射为 `3`，状态码与主 `微信鉴权 / poll_wechat_status` 完全一致）
- `1`: 等待扫描（`wx_errcode=408`）
- `2`: 已扫码未确认（`wx_errcode=404`）
- `3`: 扫码成功，返回 `wx_code`（`wx_errcode=405`）
- `-2`: 二维码超时（`wx_errcode=402`）
- `-3`: 扫码被拒绝（`wx_errcode=403`）
- `-4`: 未知错误（其他 `wx_errcode`）

#### PHP 业务逻辑提炼

- 与主微信状态轮询一致，只是后续 `code` 将用于 Wegame 登录接口。

#### Rust 代码片段

```rust
pub async fn poll_wegame_wechat_status(&self, uuid: &str) -> anyhow::Result<ApiResponse<serde_json::Value>> {
    self.poll_wechat_status(uuid).await
}
```

### 8.6 获取访问令牌 / `get_wegame_wechat_access_token`

- 所属模块：`Wegame鉴权 / 微信登录`
- HTTP 方法与路径：`GET /wegame/wechat/access`
- 请求参数：`code`
- 请求头：`Content-Type: application/json`
- 请求体示例：无

#### 响应结构

- `data.id`: `tgp_id`
- `data.ticket`: `tgp_ticket`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "id": "...", "ticket": "..." } }
```

#### 失败示例

```json
{ "code": -1, "msg": "AccessToken获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- POST `login_by_wechat`。
- 成功标准是 `code == 0 && data.error_code == 0`。
- 真正的 `tgp_id` 与 `tgp_ticket` 不在 body 主字段，而在 CookieJar。

#### Rust 代码片段

```rust
pub async fn get_wegame_wechat_access_token(&self, code: &str) -> anyhow::Result<WegameTicket> {
    let value: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/middle/clientapi/auth/login_by_wechat")
        .json(&serde_json::json!({
            "clienttype": 1000005,
            "mappid": 10001,
            "login_info": {
                "wx_info_type": 1,
                "appid": "wx911818d5d92affa8",
                "code": code
            }
        }))
        .send()
        .await?
        .json()
        .await?;

    anyhow::ensure!(value["code"] == 0 && value["data"]["error_code"] == 0, "wegame wechat login failed");
    Ok(WegameTicket {
        id: self.must_cookie("https://www.wegame.com.cn/", "tgp_id")?,
        ticket: self.must_cookie("https://www.wegame.com.cn/", "tgp_ticket")?,
    })
}
```

### 8.7 领取每日保险箱礼包 / `open_treasure_gift`

- 所属模块：`Wegame鉴权`
- HTTP 方法与路径：`POST /wegame/gift`
- 请求参数：无
- 请求头：`Content-Type: application/json`
- 请求体：
  - `id`
  - `ticket`

#### 响应结构

- `data.rewards`: 奖励列表
- 如果已领取，返回已领取提示与当前奖励信息

#### 成功示例

```json
{ "code": 0, "msg": "领取成功", "data": { "rewards": [] } }
```

#### 失败示例

```json
{ "code": -1, "msg": "领取失败", "data": [] }
```

#### 错误码

- `0`: 成功或已领取
- `-1`: 失败

#### PHP 业务逻辑提炼

- 先注入 `tgp_id`、`tgp_ticket` 到 `.wegame.com.cn`。
- 先调 `OpenTreasureChest` 查看状态。
- 若 `is_obtain` 为真，直接返回“已领取”。
- 否则继续调 `ObtainTreasureChest` 完成领取。

#### Rust 代码片段

```rust
pub async fn open_treasure_gift(&self, req: WegameTicket) -> anyhow::Result<serde_json::Value> {
    insert_cookie(&self.jar, "https://www.wegame.com.cn/", "tgp_id", &req.id);
    insert_cookie(&self.jar, "https://www.wegame.com.cn/", "tgp_ticket", &req.ticket);

    let preview: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/act/delta_force/OpenTreasureChest")
        .send()
        .await?
        .json()
        .await?;

    if preview["data"]["is_obtain"].as_bool().unwrap_or(false) {
        return Ok(preview["data"].clone());
    }

    let done: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/act/delta_force/ObtainTreasureChest")
        .send()
        .await?
        .json()
        .await?;
    Ok(done["data"].clone())
}
```

### 8.8 每日抽卡 / `draw_daily_card`

- 所属模块：`Wegame鉴权`
- HTTP 方法与路径：`POST /wegame/card`
- 请求参数：无
- 请求头：`Content-Type: application/json`
- 请求体：
  - `id`
  - `ticket`

#### 响应结构

- `data.cards`: 抽卡结果或最佳组合

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "cards": [] } }
```

#### 失败示例

```json
{ "code": -1, "msg": "抽卡失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 先查 `GetUserCards`。
- 若 `has_drawn_today` 为真，直接返回现有卡牌。
- 否则依次调用 `DrawCard` 与 `GetCardsBestCombination`。

#### Rust 代码片段

```rust
pub async fn draw_daily_card(&self, req: WegameTicket) -> anyhow::Result<serde_json::Value> {
    insert_cookie(&self.jar, "https://www.wegame.com.cn/", "tgp_id", &req.id);
    insert_cookie(&self.jar, "https://www.wegame.com.cn/", "tgp_ticket", &req.ticket);

    let current: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/act/delta_force/GetUserCards")
        .send()
        .await?
        .json()
        .await?;

    if current["data"]["has_drawn_today"].as_bool().unwrap_or(false) {
        return Ok(current["data"].clone());
    }

    let _drawn: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/act/delta_force/DrawCard")
        .send()
        .await?
        .json()
        .await?;

    let combo: serde_json::Value = self.client
        .post("https://www.wegame.com.cn/api/act/delta_force/GetCardsBestCombination")
        .send()
        .await?
        .json()
        .await?;

    Ok(combo["data"].clone())
}
```

## 9. 游戏数据

### 模块公共说明

- 路径按 API 文档中的 `/game/*` 写。
- 文档级公共 Query：`openid`、`access_token`。
- 文档级可选 Header：`acctype`，缺省为 `qc`，微信区传 `wx`。
- 部分接口无需认证，PHP 里不会读取 `openid/access_token`。
- 游戏数据类接口大多通过 `https://comm.ams.game.qq.com/ide/` 的 `iChartId + sIdeToken + method + param` 组合调用实现。

### 9.1 获取战绩 / `get_record`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/record`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- `data.gun`：PHP 实现中 `type=4` 分页聚合结果，对应 API 文档里的 `touchGold` 字段族（武器维度）。
- `data.operator`：PHP 实现中 `type=5` 分页聚合结果，对应 API 文档里的 `battlefield` 字段族（干员维度）。

> 字段命名差异说明：`三角洲行动API.md` 原文用 `touchGold / battlefield`，PHP 源码 `app/controller/Game.php::record` 实际输出 `gun / operator`。本 Rust 迁移保持与 PHP 源码一致（`gun / operator`），API 文档字段名作历史对照，不再作为返回键。

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "gun": [], "operator": [] } }
```

#### 失败示例

```json
{ "code": -1, "msg": "AccessToken已失效", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 鉴权失效或请求失败

#### PHP 业务逻辑提炼

- 对 `type=4` 和 `type=5` 分别请求 1 到 5 页。
- 调用同一个 IDE 图表 `319386 / zMemOt`。
- 汇总分页结果后返回。

#### Rust 代码片段

```rust
pub async fn get_record(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    let mut gun = Vec::new();
    let mut operator = Vec::new();
    for (kind, target) in [(4_i32, &mut gun), (5_i32, &mut operator)] {
        for page in 1..=5 {
            let page_data = self.ide_call(
                319386,
                "zMemOt",
                serde_json::json!({ "type": kind, "page": page }),
                Some(&auth),
            ).await?;
            if let Some(items) = page_data["data"].as_array() {
                target.extend(items.iter().cloned());
            }
        }
    }
    Ok(serde_json::json!({ "gun": gun, "operator": operator }))
}
```

### 9.2 获取物品信息 / `get_items`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/items`
- 请求参数：
  - `type` 可选
  - `sub_type` 可选
  - `item_id` 可选
- 请求头：无
- 请求体示例：无

#### 响应结构

- 返回物品数组，元素中可能包含 `objectID`、`objectName`、`primaryClass`、`secondClass`、`avgPrice`、`protectDetail` 等。

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [{ "objectID": 11010006002, "objectName": "H70 精英头盔" }] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 请求失败

#### PHP 业务逻辑提炼

- 无需鉴权。
- 调用 IDE `352143 / YWRywA / dfm/object.list`。
- `primary` 对应 `type`，`second` 对应 `sub_type`，`objectID` 对应 `item_id`。

#### Rust 代码片段

```rust
pub async fn get_items(&self, req: GetItemsRequest) -> anyhow::Result<serde_json::Value> {
    self.ide_call(
        352143,
        "YWRywA",
        serde_json::json!({
            "primary": req.r#type,
            "second": req.sub_type,
            "objectID": req.item_id,
        }),
        None,
    ).await
}
```

### 9.3 获取配置文件 / `get_config`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/config`
- 请求参数：无
- 请求头：无
- 请求体示例：无

#### 响应结构

- `data.config`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "objectMapping": {} } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 无需鉴权。
- 调 IDE `352143 / YWRywA / source=5 / method=dfm/config.list / configType=all`。

#### Rust 代码片段

```rust
pub async fn get_config(&self) -> anyhow::Result<serde_json::Value> {
    self.ide_call(
        352143,
        "YWRywA",
        serde_json::json!({ "configType": "all" }),
        None,
    ).await
}
```

### 9.4 获取玩家信息 / `get_player`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/player`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- `data.userData`
- `data.careerData`
- `data.coin`
- `data.tickets`
- `data.money`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "userData": {}, "coin": 0, "tickets": 0, "money": 0 } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 先调 `317814 / QIRBwm` 获取 `userData/careerData`。
- 对 `charac_name` 做 `urldecode`。
- 再分别查询三种货币：`17888808888`、`17888808889`、`17020000010`。

#### Rust 代码片段

```rust
pub async fn get_player(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    let mut base = self.ide_call(317814, "QIRBwm", serde_json::json!({}), Some(&auth)).await?;
    for (key, object_id) in [
        ("coin", 17888808888_i64),
        ("tickets", 17888808889_i64),
        ("money", 17020000010_i64),
    ] {
        let wallet = self.ide_call(
            319386,
            "zMemOt",
            serde_json::json!({ "type": 3, "page": 1, "itemId": object_id }),
            Some(&auth),
        ).await?;
        base[key] = wallet["data"][0]["totalMoney"].clone();
    }
    Ok(base)
}
```

### 9.5 获取物品成交价 / `get_price`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/price`
- 请求参数：
  - `ids` 必填，逗号分隔
  - `recent` 可选，`1` 表示附带最近成交记录
- 请求头：无
- 请求体示例：无

#### 响应结构

- 返回按物品 ID 组织的价格映射；当 `recent=1` 时，元素增加 `recent` 数组。

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "37100500001": { "avgPrice": 12345, "recent": [] } } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 把 `ids` 解析为整数数组。
- 先请求 `dfm/object.price.latest`。
- 若 `recent=1`，逐个物品调用 `dfm/object.price.recent` 并挂到 `recent` 字段。

#### Rust 代码片段

```rust
pub async fn get_price(&self, ids: Vec<i64>, with_recent: bool) -> anyhow::Result<serde_json::Value> {
    let mut latest = self.ide_call(
        352143,
        "YWRywA",
        serde_json::json!({ "method": "dfm/object.price.latest", "ids": ids }),
        None,
    ).await?;

    if with_recent {
        if let Some(map) = latest.as_object_mut() {
            for key in map.keys().cloned().collect::<Vec<_>>() {
                let recent = self.ide_call(
                    352143,
                    "YWRywA",
                    serde_json::json!({ "method": "dfm/object.price.recent", "objectID": key }),
                    None,
                ).await?;
                map.get_mut(&key).unwrap()["recent"] = recent["objectPriceRecent"]["list"].clone();
            }
        }
    }
    Ok(latest)
}
```

### 9.6 获取玩家资产信息 / `get_assets`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/assets`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- `data.userData`
- `data.weponData`
- `data.dCData`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "userData": {}, "weponData": {}, "dCData": {} } }
```

#### 失败示例

```json
{ "code": -1, "msg": "您的账号由于腾讯内部错误无法使用这个功能", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败或特殊账号限制

#### PHP 业务逻辑提炼

- 调 IDE `318948 / Plaqzy`。
- 当返回 `ret == -4000` 时，需要特殊翻译业务错误消息。

#### Rust 代码片段

```rust
pub async fn get_assets(&self, auth: GameAuth) -> anyhow::Result<ApiResponse<serde_json::Value>> {
    let raw = self.ide_call(318948, "Plaqzy", serde_json::json!({}), Some(&auth)).await?;

    // PHP: if ret == -4000 返回特定错误文案。
    if raw.get("ret").and_then(|v| v.as_i64()) == Some(-4000) {
        return Ok(ApiResponse::of(
            -1,
            "您的账号由于腾讯内部错误无法使用这个功能",
            serde_json::json!([]),
        ));
    }

    let data = raw.get("jData").cloned().unwrap_or(raw);
    Ok(ApiResponse::ok("获取成功", data))
}
```

### 9.7 获取流水日志 / `get_logs`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/logs`
- 请求参数：
  - `openid`
  - `access_token`
  - `type`
  - `page`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- 常规类型返回日志列表
- 当 `type=3` 时，返回结构被压缩为只含 `totalMoney`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 调 `319386 / zMemOt`。
- 当 `type == 3` 时，只保留 `totalMoney` 字段，避免把原日志数组原样返回。

#### Rust 代码片段

```rust
pub async fn get_logs(&self, auth: GameAuth, log_type: i32, page: i32) -> anyhow::Result<serde_json::Value> {
    let mut data = self.ide_call(
        319386,
        "zMemOt",
        serde_json::json!({ "type": log_type, "page": page }),
        Some(&auth),
    ).await?;
    if log_type == 3 {
        data = serde_json::json!([{ "totalMoney": data["data"][0]["totalMoney"].clone() }]);
    }
    Ok(data)
}
```

### 9.8 获取最近收益 / `get_recent`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/recent`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- `data.solDetail`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "solDetail": [] } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 调 `316969 / NoOapI / dfm/center.recent.detail / resourceType=sol`。

#### Rust 代码片段

```rust
pub async fn get_recent(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    self.ide_call(
        316969,
        "NoOapI",
        serde_json::json!({ "resourceType": "sol" }),
        Some(&auth),
    ).await
}
```

### 9.9 获取成就信息 / `get_achievement`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/achievement`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- `data` 为成就资源结果

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": {} }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 调 `316969 / NoOapI / dfm/center.person.resource`。
- `seasonid=[1,2,3,4,5]`，`isAllSeason=true`，`resourceType=sol`。

#### Rust 代码片段

```rust
pub async fn get_achievement(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    self.ide_call(
        316969,
        "NoOapI",
        serde_json::json!({
            "resourceType": "sol",
            "seasonid": [1, 2, 3, 4, 5],
            "isAllSeason": true,
        }),
        Some(&auth),
    ).await
}
```

### 9.10 获取密码门密码 / `get_password`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/password`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- 返回 `{ mapName: secret }` 形式的字典

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "零号大坝": "1234" } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 调 `352143 / YWRywA / dfm/center.day.secret`。
- 把列表折叠成 map，键为地图名，值为密码。

#### Rust 代码片段

```rust
pub async fn get_password(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    let data = self.ide_call(352143, "YWRywA", serde_json::json!({ "method": "dfm/center.day.secret" }), Some(&auth)).await?;
    let mut out = serde_json::Map::new();
    if let Some(items) = data.as_array() {
        for item in items {
            if let (Some(name), Some(secret)) = (item["mapName"].as_str(), item["secret"].as_str()) {
                out.insert(name.to_string(), serde_json::Value::String(secret.to_string()));
            }
        }
    }
    Ok(serde_json::Value::Object(out))
}
```

### 9.11 获取特勤处制造状态 / `get_manufacture`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/manufacture`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- `data` 为制造状态数组或对象

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 调 `365589 / bQaMCQ / source=5`。
- 直接返回 `jData.data.data`。

#### Rust 代码片段

```rust
pub async fn get_manufacture(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    self.ide_call(365589, "bQaMCQ", serde_json::json!({}), Some(&auth)).await
}
```

### 9.12 枪械数据 / `get_guns`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/guns`
- 请求参数：`gunId`
- 请求头：无
- 请求体示例：无

#### 响应结构

- `data[].gunDetail.caliber`
- `data[].gunDetail.ammo[] = { objectID, name, grade }`
- `data[].gunDetail.accessory[] = { slotID, name }`
- `data[].gunDetail.allAccessory[] = { slotID, name }`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [{ "gunDetail": { "caliber": "ammo7.62x51", "ammo": [], "accessory": [] } }] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 先调 `dfm/object.list` 获取枪械数据。
- 如果口径不是 `ammo...` 形式，则用正则把 `7.62x51` 标准化成 `ammo7.62x51`。
- 使用 `config/ammo.php` 将弹药槽位映射到名称和等级。
- 使用 `config/accessory.php` 将 `slotID` 映射到中文配件名。

#### Rust 代码片段

```rust
pub async fn get_guns(&self, gun_id: &str, ammo_cfg: &AmmoConfig, accessory_cfg: &AccessoryConfig) -> anyhow::Result<serde_json::Value> {
    let mut data = self.ide_call(
        352143,
        "YWRywA",
        serde_json::json!({
            "primary": "gun",
            "second": "gunRifle",
            "objectID": gun_id,
        }),
        None,
    ).await?;

    if let Some(items) = data.as_array_mut() {
        for weapon in items {
            let caliber_raw = weapon["gunDetail"]["caliber"].as_str().unwrap_or_default();
            let caliber = normalize_caliber_code(caliber_raw);
            weapon["gunDetail"]["caliber"] = serde_json::Value::String(caliber.clone());

            if let Some(ammo_list) = weapon["gunDetail"]["ammo"].as_array_mut() {
                for (idx, ammo) in ammo_list.iter_mut().enumerate() {
                    let object_id = ammo["objectID"].clone();
                    let mapped = ammo_cfg.get(&caliber).and_then(|list| list.get(idx));
                    *ammo = serde_json::json!({
                        "objectID": object_id,
                        "name": mapped.map(|x| x.name.clone()).unwrap_or_default(),
                        "grade": mapped.map(|x| x.grade).unwrap_or_default(),
                    });
                }
            }

            // PHP: 将 accessory / allAccessory 的 slotID 翻译成中文配件名。
            for field in ["accessory", "allAccessory"] {
                if let Some(list) = weapon["gunDetail"][field].as_array_mut() {
                    for slot in list.iter_mut() {
                        let slot_id = slot["slotID"].as_str()
                            .map(|s| s.to_string())
                            .or_else(|| slot["slotID"].as_i64().map(|n| n.to_string()))
                            .unwrap_or_default();
                        let name = accessory_cfg.get(&slot_id).cloned().unwrap_or_default();
                        *slot = serde_json::json!({
                            "slotID": slot_id,
                            "name": name,
                        });
                    }
                }
            }
        }
    }
    Ok(data)
}
```

### 9.13 获取角色信息(绑定角色) / `get_bind`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/bind`
- 请求参数：`openid`、`access_token`
- 请求头：可选 `acctype`
- 请求体示例：无

#### 响应结构

- 若已绑定：直接返回 `bindarea`
- 若未绑定：返回提交绑定后的 `bindarea`

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "roleId": "..." } }
```

#### 失败示例

```json
{ "code": -1, "msg": "绑定失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 先查 `316964 / 95ookO`。
- 如果已有 `bindarea`，直接返回。
- 如果没有，则请求 `https://comm.aci.game.qq.com/main` 拉取角色信息。
- 返回不是标准 JSON，而是 JS 对象片段，需要正则拆字段。
- `msg` 用 `GBK -> UTF-8` 转码。
- 从 `checkparam` 拆 `roleId`，再 POST `316965 / sTzZS2` 完成绑定。

#### Rust 代码片段

```rust
pub async fn get_bind(&self, auth: GameAuth) -> anyhow::Result<serde_json::Value> {
    let current = self.ide_call(316964, "95ookO", serde_json::json!({}), Some(&auth)).await?;
    if !current["bindarea"].is_null() && current["bindarea"] != serde_json::json!("") {
        return Ok(current["bindarea"].clone());
    }

    let body = self.client
        .get("https://comm.aci.game.qq.com/main")
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
        .header(reqwest::header::REFERER, "https://df.qq.com/")
        .send()
        .await?
        .text()
        .await?;

    let role = parse_bind_role_js(&body)?;
    let role_id = role.checkparam.split('|').nth(2).unwrap_or_default().to_string();
    self.ide_call(
        316965,
        "sTzZS2",
        serde_json::json!({
            "sArea": 36,
            "sPlatId": 1,
            "sPartition": 36,
            "sCheckparam": role.checkparam,
            "sRoleId": role_id,
            "md5str": role.md5str,
        }),
        Some(&auth),
    ).await
}
```

### 9.14 获取改枪码列表 / `get_firearm_mod_list`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/firearmModList`
- 请求参数：
  - `page`
  - `page_size`
- 请求头：无
- 请求体示例：无

#### 响应结构

- 返回 `solutionType=gun` 的方案列表

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 无需鉴权。
- 调 `dfm/solution.arms.list`，固定 `solutionType=gun`。

#### Rust 代码片段

```rust
pub async fn get_firearm_mod_list(&self, page: i32, page_size: i32) -> anyhow::Result<serde_json::Value> {
    self.ide_call(
        352143,
        "YWRywA",
        serde_json::json!({
            "page": page,
            "limit": page_size,
            "solutionType": "gun",
        }),
        None,
    ).await
}
```

### 9.15 特勤处制造推荐 / `get_recommendation`

- 所属模块：`游戏数据`
- HTTP 方法与路径：`GET /game/recommendation`
- 请求参数：`place`，默认 `tech`
- 请求头：无
- 请求体示例：无

#### 响应结构

- 返回 `place.list` 结果列表

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败,检查鉴权是否过期", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 无需鉴权。
- 调 `dfm/place.list`，固定 `type=place`，`hasPriceData=true`。

#### Rust 代码片段

```rust
pub async fn get_recommendation(&self, place: &str) -> anyhow::Result<serde_json::Value> {
    self.ide_call(
        352143,
        "YWRywA",
        serde_json::json!({
            "type": "place",
            "place": place,
            "hasPriceData": true,
        }),
        None,
    ).await
}
```

## 10. QQ安全中心

### 10.1 获取登录二维码 / `get_qqsafe_login_qr`

- 所属模块：`QQ安全中心`
- HTTP 方法与路径：`GET /qqsafe/sig`
- 请求参数：无
- 请求头：无
- 请求体示例：无

#### 响应结构

- 与 QQ 登录二维码一致，但业务 AppId 不同。

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": { "qrSig": "...", "loginSig": "..." } }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取二维码失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 仍然是 QQ 扫码逻辑，但 `pt_3rd_aid=101944512`。

#### Rust 代码片段

```rust
pub async fn get_qqsafe_login_qr(&self) -> anyhow::Result<QqLoginQr> {
    self.client
        .get("https://xui.ptlogin2.qq.com/cgi-bin/xlogin")
        .query(&[
            ("appid", "716027609"),
            ("daid", "383"),
            ("pt_3rd_aid", "101944512"),
        ])
        .send()
        .await?
        .error_for_status()?;
    self.get_login_qr().await
}
```

### 10.2 获取登录状态 / `poll_qqsafe_status`

- 所属模块：`QQ安全中心`
- HTTP 方法与路径：`POST /qqsafe/status`
- 请求参数：`qrToken`、`qrSig`、`loginSig`
- 请求头：默认即可
- 请求体：`cookie`

#### 响应结构

- 与 QQ 登录状态一致。

#### 成功示例

```json
{ "code": 0, "msg": "登录成功", "data": { "cookie": { "p_skey": "..." } } }
```

#### 失败示例

```json
{ "code": 2, "msg": "已扫码,待确认", "data": [] }
```

#### 错误码

- `0`: 登录成功，返回 `cookie`
- `1`: 二维码未失效（等待扫码，`ptuiCB code=66`）
- `2`: 已扫码待确认（`ptuiCB code=67`）
- `-2`: 二维码失效（`ptuiCB code=65`）
- `-3`: 登录被拒绝（`ptuiCB code=86`）
- `-4`: 未知错误（其他非 0 `ptuiCB code`）

#### PHP 业务逻辑提炼

- 与 QQ 状态轮询同形，只是目标业务是 QQ 安全中心：`u1` 指向 `gamesafe.qq.com` 登录回跳 URL，`pt_3rd_aid=101944512`。

#### Rust 代码片段

```rust
pub async fn poll_qqsafe_status(&self, req: QqStatusRequest) -> anyhow::Result<ApiResponse<serde_json::Value>> {
    self.poll_login_status(req).await
}
```

### 10.3 获取访问令牌接口 / `get_qqsafe_access_token`

- 所属模块：`QQ安全中心`
- HTTP 方法与路径：`POST /qqsafe/access`
- 请求参数：无
- 请求头：无特殊要求
- 请求体：`cookie`

#### 响应结构

- `data.openid`: 来自 `gs_id`
- `data.access_token`: 来自 `gs_code` payload 中的 `token`
- `data.code`: 原始 `gs_code`

#### 成功示例

```json
{
  "code": 0,
  "msg": "获取成功",
  "data": {
    "access_token": "...",
    "openid": "...",
    "code": "gs_code..."
  }
}
```

#### 失败示例

```json
{ "code": -1, "msg": "AccessToken获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- POST `graph.qq.com/oauth2.0/authorize`，`client_id=101944512`。
- GET `https://gamesafe.qq.com/connect?...`。
- 成功后从 CookieJar 取 `gs_code` 和 `gs_id`。
- `gs_code` 不是直接 token，需要按 `.` 分段，解码中间段 base64，取 JSON 中的 `token`。

#### Rust 代码片段

```rust
pub async fn get_qqsafe_access_token(&self, cookie_json: &str) -> anyhow::Result<QqSafeAccess> {
    self.restore_cookie_json("https://graph.qq.com/", cookie_json)?;
    let p_skey = self.must_cookie("https://graph.qq.com/", "p_skey")?;
    let gtk = get_gtk(&p_skey).to_string();

    let resp = self.client
        .post("https://graph.qq.com/oauth2.0/authorize")
        .form(&[
            ("response_type", "code"),
            ("client_id", "101944512"),
            ("redirect_uri", "https://gamesafe.qq.com/login-ui/index.html?appId=101944512"),
            ("scope", "all"),
            ("state", "qqconnect_2"),
            ("g_tk", gtk.as_str()),
        ])
        .send()
        .await?;

    let location = resp.headers().get(reqwest::header::LOCATION).ok_or_else(|| anyhow::anyhow!("missing location"))?.to_str()?;
    let code = extract_query_param(location, "code")?;
    let _ = self.client
        .get("https://gamesafe.qq.com/connect")
        .query(&[("code", code.as_str()), ("appId", "101944512"), ("atype", "QQ")])
        .send()
        .await?;

    let gs_code = self.must_cookie("https://gamesafe.qq.com/", "gs_code")?;
    let openid = self.must_cookie("https://gamesafe.qq.com/", "gs_id")?;
    let payload = decode_jwt_middle(&gs_code)?;

    Ok(QqSafeAccess {
        access_token: payload["token"].as_str().unwrap_or_default().to_string(),
        openid,
        code: gs_code,
    })
}
```

### 10.4 游戏处罚列表 / `get_banned_list`

- 所属模块：`QQ安全中心`
- HTTP 方法与路径：`GET /qqsafe/bannedList`
- 请求参数：
  - `openid`
  - `access_token`
  - `code`
- 请求头：无
- 请求体示例：无

#### 响应结构

- 返回处罚记录数组，元素中包含 `strategy_desc`、`reason`、`duration`、`game_name` 等。

#### 成功示例

```json
{ "code": 0, "msg": "获取成功", "data": [{ "game_name": "三角洲行动", "type": "封号" }] }
```

#### 失败示例

```json
{ "code": -1, "msg": "获取失败", "data": [] }
```

#### 错误码

- `0`: 成功
- `-1`: 失败

#### PHP 业务逻辑提炼

- 通过 `openid/access_token/code` 组装 `.qq.com` 域 Cookie：`openid`、`access_token`、`gs_id`、`gs_code`。
- GET `https://gamesafe.qq.com/api/proxy/punish_query?query_type=4&limit=10`。
- 最终返回 `data.data`。

#### Rust 代码片段

```rust
pub async fn get_banned_list(&self, req: QqSafeAccess) -> anyhow::Result<serde_json::Value> {
    insert_cookie(&self.jar, "https://gamesafe.qq.com/", "openid", &req.openid);
    insert_cookie(&self.jar, "https://gamesafe.qq.com/", "access_token", &req.access_token);
    insert_cookie(&self.jar, "https://gamesafe.qq.com/", "gs_id", &req.openid);
    insert_cookie(&self.jar, "https://gamesafe.qq.com/", "gs_code", &req.code);

    let value: serde_json::Value = self.client
        .get("https://gamesafe.qq.com/api/proxy/punish_query")
        .query(&[("query_type", "4"), ("limit", "10")])
        .send()
        .await?
        .json()
        .await?;

    Ok(value["data"].clone())
}
```

## 11. PHP / Rust 对照表

| PHP 位置 | 现有职责 | Rust 建议位置 | Rust 命名 |
| --- | --- | --- | --- |
| `app/common.php::getMicroTime` | 毫秒时间戳 | `utils/time.rs` | `current_millis` |
| `app/common.php::getQrToken` | QQ 扫码 token | `utils/hashes.rs` | `get_qr_token` |
| `app/common.php::getGTK` | `g_tk` 计算 | `utils/hashes.rs` | `get_gtk` |
| `QQ.php::getQrSig` | QQ 二维码初始化 | `services/qq_auth.rs` | `get_login_qr` |
| `QQ.php::getAction` | QQ 登录状态轮询 | `services/qq_auth.rs` | `poll_login_status` |
| `QQ.php::getAccessToken` | QQ OAuth 换 token | `services/qq_auth.rs` | `get_access_token` |
| `Wechat.php::login` | 微信二维码初始化 | `services/wechat_auth.rs` | `get_wechat_login_qr` |
| `Wechat.php::status` | 微信扫码状态 | `services/wechat_auth.rs` | `poll_wechat_status` |
| `Wechat.php::getAccessToken` | 微信 code 换 token | `services/wechat_auth.rs` | `get_wechat_access_token` |
| `Wegame.php::gift` | Wegame 礼包领取 | `services/wegame_auth.rs` | `open_treasure_gift` |
| `Wegame.php::card` | Wegame 抽卡 | `services/wegame_auth.rs` | `draw_daily_card` |
| `QQSafe.php::bannedList` | QQ 安全中心处罚列表 | `services/qq_safe.rs` | `get_banned_list` |
| `Game.php::record` | 战绩聚合 | `services/game.rs` | `get_record` |
| `Game.php::bind` | 角色绑定 | `services/game.rs` | `get_bind` |
| `Game.php::guns` | 枪械配置映射 | `services/game.rs` | `get_guns` |

## 12. 测试建议与迁移风险

### 12.1 测试建议

- 单元测试
  - `get_qr_token` 算法对齐测试
  - `get_gtk` 算法对齐测试
  - `normalize_caliber_code` 正则标准化测试
  - `ptuiCB(...)` / `coolxitech(...)` 解析测试
  - `decode_jwt_middle` 测试
  - `bind()` 的 JS 对象片段解析与 `GBK -> UTF-8` 转码测试
- 集成测试
  - 使用录制响应或本地 fixture 验证 QQ / 微信 / Wegame / QQSafe 登录流程的解析逻辑
  - 用 mock server 验证 redirect 被禁止后仍能从 `Location` 抓到 `code`
  - 验证游戏数据 IDE 请求封装是否正确注入 Cookie 与 `acctype`
- 回归测试
  - 逐一覆盖本文 35 个有效接口
  - 明确断言不暴露废弃接口 `test` 与 `report`

### 12.2 迁移风险

- 登录链路不能简化
  - QQ、QQSafe、Wegame QQ 都依赖“初始化 Cookie -> 展示二维码 -> 轮询 -> 跟随跳转 -> 再换业务 token”这一完整状态流。
- Cookie 是核心状态
  - 当前 PHP 广泛依赖 CookieJar 与跨域 Cookie 组装，Rust 若只保存 `access_token` 会丢失关键上下文。
- 需要解析非标准响应
  - 包括 HTML、JSONP、JS 对象片段，以及 QQ 安全中心 `gs_code` 的中段 payload。
- redirect 策略必须显式控制
  - 当前 PHP 依赖 `allow_redirects=false` 并从 `Location` 读取 `code`，Rust 也需要保持这个行为。
- TLS 策略需显式决策
  - PHP 当前 `verify=false`，Rust 默认建议保留证书校验。如果需要兼容旧环境，应通过显式配置开关启用不安全模式。
- `guns()` 依赖本地配置映射
  - `config/ammo.php` 与 `config/accessory.php` 不是远端返回数据的一部分，Rust 迁移时必须同步保留这套映射资源。
- `/game/*` 路径说明
  - 本文按 API 文档中的 `/game/*` 路径编写；当前 `route/app.php` 未见显式注册，实际部署暴露方式建议在迁移上线前再次确认。

## 附：建议先实现的优先级

1. 公共模块：`client`、`cookies`、`hashes`、`jsonp`、`response`
2. `QQ鉴权`
3. `微信鉴权`
4. `QQ安全中心`
5. `Wegame鉴权`
6. `游戏数据`

这样可以先把最复杂的登录态基础设施落稳，再扩展到业务查询接口。
