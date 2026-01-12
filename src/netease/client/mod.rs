mod config;
mod cookie;
mod error;
mod types;

pub use config::{ClientState, NeteaseClientConfig};
pub use error::NeteaseError;
pub use types::{QrPlatform, ValidateCookieResult};

use crate::netease::crypto::{self, CryptoMode};
use crate::netease::util;
use cookie::{cookie_obj_to_string, create_header_cookie, process_cookie_object, update_cookies};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, REFERER, SET_COOKIE, USER_AGENT};
use serde_json::{Value, json};
use std::fs;
use types::{UA_API_IPHONE, UA_LINUX, UA_WEAPI_PC};

#[derive(Debug)]
pub struct NeteaseClient {
    http: reqwest::Client,
    pub cfg: NeteaseClientConfig,
    pub state: ClientState,
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
            state: config::load_state(&cfg.data_dir)?,
            cfg,
        };

        if client.state.device_id.is_none() {
            client.state.device_id = Some(util::generate_device_id());
            client.save_state()?;
        }

        Ok(client)
    }

    fn device_id(&self) -> &str {
        self.state.device_id.as_deref().unwrap_or("UNKNOWN")
    }

    fn save_state(&self) -> Result<(), NeteaseError> {
        config::save_state(&self.cfg.data_dir, &self.state)
    }

    // ========== Auth Methods ==========

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
        self.request(
            "/api/register/anonimous",
            json!({ "username": username }),
            CryptoMode::Weapi,
        )
        .await
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
        use cookie::generate_chain_id;
        let mut url = format!("https://music.163.com/login?codekey={key}");
        if matches!(platform, QrPlatform::Web) {
            let chain_id = generate_chain_id(&self.state.cookies);
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

    pub fn logout_local(&mut self) -> Result<(), NeteaseError> {
        self.state.cookies.clear();
        self.save_state()?;
        Ok(())
    }

    pub async fn set_cookie_and_validate(
        &mut self,
        music_u: &str,
    ) -> Result<ValidateCookieResult, NeteaseError> {
        self.state
            .cookies
            .insert("MUSIC_U".to_owned(), music_u.to_owned());
        self.save_state()?;

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
                self.state.cookies.remove("MUSIC_U");
                self.save_state()?;
                Err(NeteaseError::CookieValidationFailed(format!(
                    "Cookie 验证失败: {e}"
                )))
            }
        }
    }

    // ========== API Methods ==========

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
        let c = ids.iter().map(|id| json!({ "id": id })).collect::<Vec<_>>();
        let c = serde_json::to_string(&c).map_err(NeteaseError::Serde)?;
        self.request("/api/v3/song/detail", json!({ "c": c }), CryptoMode::Weapi)
            .await
    }

    // ========== Request Methods ==========

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

        let mut cookie = process_cookie_object(&self.state.cookies, self.device_id(), uri);

        let (url, form, _should_return_early) = match crypto {
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
                (url, form, false)
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
                (url, form, false)
            }
            CryptoMode::Eapi => {
                use cookie::build_eapi_header;
                headers.insert(USER_AGENT, HeaderValue::from_static(UA_API_IPHONE));
                let header = build_eapi_header(&cookie, self.device_id());
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

        update_cookies(&mut self.state.cookies, &set_cookies);
        self.save_state()?;

        Ok(body)
    }
}
