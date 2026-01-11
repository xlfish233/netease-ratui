use crate::netease::crypto::{self, CryptoMode};
use crate::netease::util;
use directories::ProjectDirs;
use rand::Rng;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, REFERER, SET_COOKIE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use urlencoding::encode;

#[derive(Debug, Clone)]
pub struct NeteaseClientConfig {
    pub domain: String,
    pub api_domain: String,
    pub data_dir: PathBuf,
}

impl Default for NeteaseClientConfig {
    fn default() -> Self {
        let data_dir = ProjectDirs::from("dev", "netease", "netease-ratui")
            .map(|p| p.data_local_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("netease-ratui"));
        Self {
            domain: "https://music.163.com".to_owned(),
            api_domain: "https://interface.music.163.com".to_owned(),
            data_dir,
        }
    }
}

#[derive(Debug)]
pub struct NeteaseClient {
    http: reqwest::Client,
    cfg: NeteaseClientConfig,
    state: ClientState,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ClientState {
    cookies: HashMap<String, String>,
    device_id: Option<String>,
}

impl NeteaseClient {
    pub fn new(cfg: NeteaseClientConfig) -> Result<Self, NeteaseError> {
        fs::create_dir_all(&cfg.data_dir).map_err(NeteaseError::Io)?;

        let http = reqwest::Client::builder()
            .user_agent("netease-ratui")
            .build()
            .map_err(NeteaseError::Reqwest)?;

        let mut client = Self {
            http,
            state: load_state(&cfg.data_dir)?,
            cfg,
        };

        if client.state.device_id.is_none() {
            client.state.device_id = Some(util::generate_device_id());
            client.save_state()?;
        }

        Ok(client)
    }

    pub fn is_logged_in(&self) -> bool {
        self.state.cookies.contains_key("MUSIC_U")
    }

    #[allow(dead_code)]
    pub fn cookie_string(&self) -> String {
        self.state
            .cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub async fn ensure_anonymous(&mut self) -> Result<(), NeteaseError> {
        if self.is_logged_in() {
            return Ok(());
        }
        if self.state.cookies.contains_key("MUSIC_A") {
            return Ok(());
        }
        self.register_anonymous().await?;
        Ok(())
    }

    pub async fn register_anonymous(&mut self) -> Result<Value, NeteaseError> {
        let device_id = self.device_id().to_owned();
        let username = util::build_anonymous_username(&device_id);
        let resp = self
            .request(
                "/api/register/anonimous",
                json!({ "username": username }),
                CryptoMode::Weapi,
            )
            .await?;

        Ok(resp)
    }

    pub async fn login_qr_key(&mut self) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request(
            "/api/login/qrcode/unikey",
            json!({ "type": 3 }),
            CryptoMode::Eapi,
        )
        .await
    }

    pub fn login_qr_url(&self, key: &str, platform: QrPlatform) -> String {
        let mut url = format!("https://music.163.com/login?codekey={key}");
        if matches!(platform, QrPlatform::Web) {
            let chain_id = self.generate_chain_id();
            url.push_str("&chainId=");
            url.push_str(&chain_id);
        }
        url
    }

    pub async fn login_qr_check(&mut self, key: &str) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request(
            "/api/login/qrcode/client/login",
            json!({ "key": key, "type": 3 }),
            CryptoMode::Eapi,
        )
        .await
    }

    pub async fn cloudsearch(
        &mut self,
        keywords: &str,
        kind: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request(
            "/api/cloudsearch/pc",
            json!({
              "s": keywords,
              "type": kind,
              "limit": limit,
              "offset": offset,
              "total": true,
            }),
            CryptoMode::Eapi,
        )
        .await
    }

    pub async fn song_url(&mut self, ids: &[i64], br: i64) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        let ids_str = serde_json::to_string(ids).map_err(NeteaseError::Serde)?;
        self.request(
            "/api/song/enhance/player/url",
            json!({ "ids": ids_str, "br": br }),
            CryptoMode::Eapi,
        )
        .await
    }

    pub async fn lyric(&mut self, id: i64) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request(
            "/api/song/lyric",
            json!({
              "id": id,
              "tv": -1,
              "lv": -1,
              "rv": -1,
              "kv": -1,
              "_nmclfl": 1,
            }),
            CryptoMode::Eapi,
        )
        .await
    }

    pub async fn user_account(&mut self) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request("/api/nuser/account/get", json!({}), CryptoMode::Weapi)
            .await
    }

    pub async fn user_playlist(
        &mut self,
        uid: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request(
            "/api/user/playlist",
            json!({
              "uid": uid,
              "limit": limit,
              "offset": offset,
              "includeVideo": true,
            }),
            CryptoMode::Weapi,
        )
        .await
    }

    pub async fn playlist_detail(&mut self, id: i64) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        self.request(
            "/api/v6/playlist/detail",
            json!({
              "id": id,
              "n": 100000,
              "s": 8,
            }),
            CryptoMode::Weapi,
        )
        .await
    }

    pub async fn song_detail_by_ids(&mut self, ids: &[i64]) -> Result<Value, NeteaseError> {
        self.ensure_anonymous().await?;
        // 注意：该接口的 `c` 习惯上传字符串形式的 JSON 数组（参考 api-enhanced 实现）
        let c = ids.iter().map(|id| json!({ "id": id })).collect::<Vec<_>>();
        let c = serde_json::to_string(&c).map_err(NeteaseError::Serde)?;
        self.request("/api/v3/song/detail", json!({ "c": c }), CryptoMode::Weapi)
            .await
    }

    async fn request(
        &mut self,
        uri: &str,
        mut data: Value,
        crypto: CryptoMode,
    ) -> Result<Value, NeteaseError> {
        if !data.is_object() {
            return Err(NeteaseError::BadInput("data 必须是 JSON object"));
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );

        let mut cookie = self.process_cookie_object(uri)?;

        let (url, form) = match crypto {
            CryptoMode::Weapi => {
                headers.insert(
                    REFERER,
                    HeaderValue::from_str(&self.cfg.domain)
                        .map_err(|e| NeteaseError::BadHeader(format!("REFERER: {e}")))?,
                );
                headers.insert(USER_AGENT, HeaderValue::from_static(UA_WEAPI_PC));
                let csrf = cookie.get("__csrf").cloned().unwrap_or_default();
                data.as_object_mut()
                    .ok_or(NeteaseError::BadInput("data 必须是 JSON object"))?
                    .insert("csrf_token".to_owned(), Value::String(csrf));

                let f = crypto::weapi(&data).map_err(NeteaseError::Crypto)?;
                cookie.insert("os".to_owned(), "pc".to_owned());

                let url = format!(
                    "{}/weapi/{}",
                    self.cfg.domain.trim_end_matches('/'),
                    uri.trim_start_matches("/api/"),
                );
                let form = vec![("params", f.params), ("encSecKey", f.enc_sec_key)];
                (url, form)
            }
            CryptoMode::Linuxapi => {
                headers.insert(USER_AGENT, HeaderValue::from_static(UA_LINUX));
                let url = format!(
                    "{}/api/linux/forward",
                    self.cfg.domain.trim_end_matches('/')
                );
                let linux_obj = json!({
                    "method": "POST",
                    "url": format!("{}{}", self.cfg.domain.trim_end_matches('/'), uri),
                    "params": data,
                });
                let f = crypto::linuxapi(&linux_obj).map_err(NeteaseError::Crypto)?;
                let form = vec![("eparams", f.eparams)];
                (url, form)
            }
            CryptoMode::Eapi => {
                headers.insert(USER_AGENT, HeaderValue::from_static(UA_API_IPHONE));
                let header = self.build_eapi_header(&cookie);
                let header_cookie = create_header_cookie(&header);

                data.as_object_mut()
                    .ok_or(NeteaseError::BadInput("data 必须是 JSON object"))?
                    .insert("header".to_owned(), json!(header));

                self.state.cookies.insert(
                    "os".to_owned(),
                    cookie.get("os").cloned().unwrap_or_else(|| "pc".to_owned()),
                );

                let f = crypto::eapi(uri, &data).map_err(NeteaseError::Crypto)?;
                let url = format!(
                    "{}/eapi/{}",
                    self.cfg.api_domain.trim_end_matches('/'),
                    uri.trim_start_matches("/api/"),
                );
                let form = vec![("params", f.params)];

                // eapi 的 Cookie 不是浏览器 cookie（是 header cookie），直接覆盖
                headers.insert(
                    "Cookie",
                    HeaderValue::from_str(&header_cookie).map_err(|e| {
                        NeteaseError::BadHeader(format!("Cookie(header cookie): {e}"))
                    })?,
                );
                return self.send(url, headers, form).await;
            }
        };

        headers.insert(
            "Cookie",
            HeaderValue::from_str(&cookie_obj_to_string(&cookie))
                .map_err(|e| NeteaseError::BadHeader(format!("Cookie: {e}")))?,
        );
        self.send(url, headers, form).await
    }

    async fn send(
        &mut self,
        url: String,
        headers: HeaderMap,
        form: Vec<(&'static str, String)>,
    ) -> Result<Value, NeteaseError> {
        let resp = match self
            .http
            .post(url.clone())
            .headers(headers.clone())
            .form(&form)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // 某些环境下 `interface.music.163.com` 可能 DNS 失败，降级到 `music.163.com`
                if url.contains("https://interface.music.163.com/") {
                    tracing::warn!(url = %url, err = %e, "请求失败，降级到 music.163.com");
                    let fallback =
                        url.replace("https://interface.music.163.com/", "https://music.163.com/");
                    self.http
                        .post(fallback)
                        .headers(headers)
                        .form(&form)
                        .send()
                        .await
                        .map_err(NeteaseError::Reqwest)?
                } else {
                    return Err(NeteaseError::Reqwest(e));
                }
            }
        };

        let set_cookies = resp
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok().map(ToOwned::to_owned))
            .collect::<Vec<String>>();

        let bytes = resp.bytes().await.map_err(NeteaseError::Reqwest)?;
        let body: Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()));

        self.update_cookies(&set_cookies);
        self.save_state()?;

        Ok(body)
    }

    fn device_id(&self) -> &str {
        self.state.device_id.as_deref().unwrap_or("UNKNOWN")
    }

    fn process_cookie_object(&self, uri: &str) -> Result<HashMap<String, String>, NeteaseError> {
        let mut cookie = self.state.cookies.clone();

        let now_ms = now_millis();
        let nuid = cookie
            .entry("_ntes_nuid".to_owned())
            .or_insert_with(|| util::random_hex_string(32))
            .clone();

        cookie.insert("__remember_me".to_owned(), "true".to_owned());
        cookie.insert("ntes_kaola_ad".to_owned(), "1".to_owned());
        cookie
            .entry("_ntes_nnid".to_owned())
            .or_insert_with(|| format!("{nuid},{now_ms}"));

        cookie
            .entry("WNMCID".to_owned())
            .or_insert_with(|| self.wnmcid());
        cookie
            .entry("WEVNSM".to_owned())
            .or_insert_with(|| "1.0.0".to_owned());

        let os = cookie.get("os").map(String::as_str).unwrap_or("pc");
        let os_profile = match os {
            "linux" => OsProfile::linux(),
            "android" => OsProfile::android(),
            "iphone" => OsProfile::iphone(),
            _ => OsProfile::pc(),
        };

        cookie
            .entry("osver".to_owned())
            .or_insert_with(|| os_profile.osver.to_owned());
        cookie
            .entry("deviceId".to_owned())
            .or_insert_with(|| self.device_id().to_owned());
        cookie
            .entry("os".to_owned())
            .or_insert_with(|| os_profile.os.to_owned());
        cookie
            .entry("channel".to_owned())
            .or_insert_with(|| os_profile.channel.to_owned());
        cookie
            .entry("appver".to_owned())
            .or_insert_with(|| os_profile.appver.to_owned());

        if !uri.contains("login") {
            cookie
                .entry("NMTID".to_owned())
                .or_insert_with(|| util::random_hex_string(16));
        }

        Ok(cookie)
    }

    fn build_eapi_header(&self, cookie: &HashMap<String, String>) -> HashMap<String, String> {
        let mut header = HashMap::new();
        let mut rng = rand::thread_rng();

        let csrf = cookie.get("__csrf").cloned().unwrap_or_default();
        header.insert(
            "osver".to_owned(),
            cookie
                .get("osver")
                .cloned()
                .unwrap_or_else(|| "undefined".to_owned()),
        );
        header.insert(
            "deviceId".to_owned(),
            cookie
                .get("deviceId")
                .cloned()
                .unwrap_or_else(|| self.device_id().to_owned()),
        );
        header.insert(
            "os".to_owned(),
            cookie.get("os").cloned().unwrap_or_else(|| "pc".to_owned()),
        );
        header.insert(
            "appver".to_owned(),
            cookie
                .get("appver")
                .cloned()
                .unwrap_or_else(|| "8.20.20.231215173437".to_owned()),
        );
        header.insert(
            "versioncode".to_owned(),
            cookie
                .get("versioncode")
                .cloned()
                .unwrap_or_else(|| "140".to_owned()),
        );
        header.insert(
            "mobilename".to_owned(),
            cookie.get("mobilename").cloned().unwrap_or_default(),
        );
        header.insert(
            "buildver".to_owned(),
            cookie
                .get("buildver")
                .cloned()
                .unwrap_or_else(|| now_secs().to_string()),
        );
        header.insert(
            "resolution".to_owned(),
            cookie
                .get("resolution")
                .cloned()
                .unwrap_or_else(|| "1920x1080".to_owned()),
        );
        header.insert("__csrf".to_owned(), csrf);
        header.insert(
            "channel".to_owned(),
            cookie
                .get("channel")
                .cloned()
                .unwrap_or_else(|| "netease".to_owned()),
        );

        header.insert(
            "requestId".to_owned(),
            format!("{}_{:04}", now_millis(), rng.gen_range(0..1000usize)),
        );

        if let Some(v) = cookie.get("MUSIC_U") {
            header.insert("MUSIC_U".to_owned(), v.to_owned());
        }
        if let Some(v) = cookie.get("MUSIC_A") {
            header.insert("MUSIC_A".to_owned(), v.to_owned());
        }

        header
    }

    fn wnmcid(&self) -> String {
        static CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
        let mut rng = rand::thread_rng();
        let mut s = String::with_capacity(6);
        for _ in 0..6 {
            let idx = rng.gen_range(0..CHARS.len());
            s.push(CHARS[idx] as char);
        }
        format!("{s}.{}.01.0", now_millis())
    }

    fn generate_chain_id(&self) -> String {
        let version = "v1";
        let random_num: u32 = rand::thread_rng().gen_range(0..1_000_000);
        let device_id = self
            .state
            .cookies
            .get("sDeviceId")
            .cloned()
            .unwrap_or_else(|| format!("unknown-{random_num}"));
        let platform = "web";
        let action = "login";
        let ts = now_millis();
        format!("{version}_{device_id}_{platform}_{action}_{ts}")
    }

    fn update_cookies(&mut self, set_cookie_headers: &[String]) {
        for sc in set_cookie_headers {
            if let Ok(c) = cookie::Cookie::parse(sc.to_owned()) {
                self.state
                    .cookies
                    .insert(c.name().to_owned(), c.value().to_owned());
            }
        }
    }

    fn save_state(&self) -> Result<(), NeteaseError> {
        save_state(&self.cfg.data_dir, &self.state)
    }

    pub fn logout_local(&mut self) -> Result<(), NeteaseError> {
        self.state.cookies.clear();
        self.save_state()?;
        Ok(())
    }

    /// 手动设置 MUSIC_U Cookie 并验证有效性
    pub async fn set_cookie_and_validate(
        &mut self,
        music_u: &str,
    ) -> Result<ValidateCookieResult, NeteaseError> {
        // 设置 MUSIC_U cookie
        self.state
            .cookies
            .insert("MUSIC_U".to_owned(), music_u.to_owned());
        self.save_state()?;

        // 立即验证 Cookie 有效性
        match self.user_account().await {
            Ok(v) => {
                let resp: crate::netease::models::dto::UserAccountResp =
                    serde_json::from_value(v).map_err(NeteaseError::Serde)?;
                let account = resp.account.ok_or_else(|| {
                    NeteaseError::CookieValidationFailed("未找到账号信息".to_owned())
                })?;
                let profile = resp.profile.ok_or_else(|| {
                    NeteaseError::CookieValidationFailed("未找到用户资料".to_owned())
                })?;
                Ok(ValidateCookieResult {
                    uid: account.id,
                    nickname: profile.nickname,
                })
            }
            Err(e) => {
                // 验证失败，清除刚设置的 cookie
                self.state.cookies.remove("MUSIC_U");
                self.save_state()?;
                Err(NeteaseError::CookieValidationFailed(format!(
                    "Cookie 验证失败: {e}"
                )))
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum QrPlatform {
    Pc,
    #[allow(dead_code)]
    Web,
}

struct OsProfile {
    os: &'static str,
    appver: &'static str,
    osver: &'static str,
    channel: &'static str,
}

impl OsProfile {
    const fn pc() -> Self {
        Self {
            os: "pc",
            appver: "3.1.17.204416",
            osver: "Microsoft-Windows-10-Professional-build-19045-64bit",
            channel: "netease",
        }
    }
    const fn linux() -> Self {
        Self {
            os: "linux",
            appver: "1.2.1.0428",
            osver: "Deepin 20.9",
            channel: "netease",
        }
    }
    const fn android() -> Self {
        Self {
            os: "android",
            appver: "8.20.20.231215173437",
            osver: "14",
            channel: "xiaomi",
        }
    }
    const fn iphone() -> Self {
        Self {
            os: "iPhone OS",
            appver: "9.0.90",
            osver: "16.2",
            channel: "distribution",
        }
    }
}

const UA_WEAPI_PC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0";
const UA_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36";
const UA_API_IPHONE: &str = "NeteaseMusic 9.0.90/5038 (iPhone; iOS 16.2; zh_CN)";

fn create_header_cookie(header: &HashMap<String, String>) -> String {
    let mut parts = Vec::with_capacity(header.len());
    for (k, v) in header {
        parts.push(format!("{}={}", encode(k), encode(v)));
    }
    parts.join("; ")
}

fn cookie_obj_to_string(cookie: &HashMap<String, String>) -> String {
    let mut parts = Vec::with_capacity(cookie.len());
    for (k, v) in cookie {
        parts.push(format!("{}={}", encode(k), encode(v)));
    }
    parts.join("; ")
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn state_path(data_dir: &Path) -> PathBuf {
    data_dir.join("netease_state.json")
}

fn load_state(data_dir: &Path) -> Result<ClientState, NeteaseError> {
    let p = state_path(data_dir);
    if !p.exists() {
        return Ok(ClientState::default());
    }
    let bytes = fs::read(p).map_err(NeteaseError::Io)?;
    serde_json::from_slice(&bytes).map_err(NeteaseError::Serde)
}

fn save_state(data_dir: &Path, state: &ClientState) -> Result<(), NeteaseError> {
    let p = state_path(data_dir);
    let bytes = serde_json::to_vec_pretty(state).map_err(NeteaseError::Serde)?;
    fs::write(p, bytes).map_err(NeteaseError::Io)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ValidateCookieResult {
    pub uid: i64,
    pub nickname: String,
}

#[derive(thiserror::Error, Debug)]
pub enum NeteaseError {
    #[error("reqwest 错误: {0}")]
    Reqwest(reqwest::Error),
    #[error("IO 错误: {0}")]
    Io(std::io::Error),
    #[error("serde 错误: {0}")]
    Serde(serde_json::Error),
    #[error("crypto 错误: {0}")]
    Crypto(crypto::CryptoError),
    #[error("Header 构造失败: {0}")]
    BadHeader(String),
    #[error("输入错误: {0}")]
    BadInput(&'static str),
    #[error("Cookie 验证失败: {0}")]
    CookieValidationFailed(String),
}
