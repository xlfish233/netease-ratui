use super::types::OsProfile;
use crate::netease::util;
use rand::Rng;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use urlencoding::encode;

pub fn create_header_cookie(header: &HashMap<String, String>) -> String {
    let mut parts = Vec::with_capacity(header.len());
    for (k, v) in header {
        parts.push(format!("{}={}", encode(k), encode(v)));
    }
    parts.join("; ")
}

pub fn cookie_obj_to_string(cookie: &HashMap<String, String>) -> String {
    let mut parts = Vec::with_capacity(cookie.len());
    for (k, v) in cookie {
        parts.push(format!("{}={}", encode(k), encode(v)));
    }
    parts.join("; ")
}

pub fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn wnmcid() -> String {
    static CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();
    let mut s = String::with_capacity(6);
    for _ in 0..6 {
        let idx = rng.gen_range(0..CHARS.len());
        s.push(CHARS[idx] as char);
    }
    format!("{s}.{}.01.0", now_millis())
}

pub fn process_cookie_object(
    cookies: &HashMap<String, String>,
    device_id: &str,
    uri: &str,
) -> HashMap<String, String> {
    let mut cookie = cookies.clone();

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

    cookie.entry("WNMCID".to_owned()).or_insert_with(wnmcid);
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
        .or_insert_with(|| device_id.to_owned());
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

    cookie
}

pub fn build_eapi_header(
    cookie: &HashMap<String, String>,
    device_id: &str,
) -> HashMap<String, String> {
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
            .unwrap_or_else(|| device_id.to_owned()),
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

pub fn generate_chain_id(cookies: &HashMap<String, String>) -> String {
    let version = "v1";
    let random_num: u32 = rand::thread_rng().gen_range(0..1_000_000);
    let device_id = cookies
        .get("sDeviceId")
        .cloned()
        .unwrap_or_else(|| format!("unknown-{random_num}"));
    let platform = "web";
    let action = "login";
    let ts = now_millis();
    format!("{version}_{device_id}_{platform}_{action}_{ts}")
}

pub fn update_cookies(cookies: &mut HashMap<String, String>, set_cookie_headers: &[String]) {
    for sc in set_cookie_headers {
        if let Ok(c) = cookie::Cookie::parse(sc.to_owned()) {
            cookies.insert(c.name().to_owned(), c.value().to_owned());
        }
    }
}
