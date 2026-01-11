#[derive(Debug, Clone, Copy)]
pub enum QrPlatform {
    Pc,
    #[allow(dead_code)]
    Web,
}

pub struct OsProfile {
    pub os: &'static str,
    pub appver: &'static str,
    pub osver: &'static str,
    pub channel: &'static str,
}

impl OsProfile {
    pub const fn pc() -> Self {
        Self {
            os: "pc",
            appver: "3.1.17.204416",
            osver: "Microsoft-Windows-10-Professional-build-19045-64bit",
            channel: "netease",
        }
    }

    pub const fn linux() -> Self {
        Self {
            os: "linux",
            appver: "1.2.1.0428",
            osver: "Deepin 20.9",
            channel: "netease",
        }
    }

    pub const fn android() -> Self {
        Self {
            os: "android",
            appver: "8.20.20.231215173437",
            osver: "14",
            channel: "xiaomi",
        }
    }

    pub const fn iphone() -> Self {
        Self {
            os: "iPhone OS",
            appver: "9.0.90",
            osver: "16.2",
            channel: "distribution",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ValidateCookieResult {
    pub uid: i64,
    pub nickname: String,
}

pub const UA_WEAPI_PC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0";
pub const UA_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36";
pub const UA_API_IPHONE: &str = "NeteaseMusic 9.0.90/5038 (iPhone; iOS 16.2; zh_CN)";
