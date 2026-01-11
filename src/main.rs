mod app;
mod audio_worker;
mod domain;
mod logging;
mod messages;
mod netease;
mod settings;
mod tui;
mod usecases;

use app::App;
use netease::{NeteaseClient, NeteaseClientConfig};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = NeteaseClientConfig::default();
    let _log_guard = logging::init(&cfg.data_dir);
    tracing::info!(data_dir = %cfg.data_dir.display(), "netease-ratui 启动");

    // 便于无交互环境快速验证 service layer
    if env::var("NETEASE_SKIP_LOGIN").ok().as_deref() == Some("1") {
        tracing::info!("启动模式: NETEASE_SKIP_LOGIN=1");
        let mut client = NeteaseClient::new(cfg)?;
        client.ensure_anonymous().await?;
        let search = client.cloudsearch("周杰伦", 1, 5, 0).await?;
        println!("搜索结果(前5首): {}", search);
        return Ok(());
    }

    // 便于排查二维码登录接口返回结构
    if env::var("NETEASE_QR_KEY").ok().as_deref() == Some("1") {
        tracing::info!("启动模式: NETEASE_QR_KEY=1");
        let mut client = NeteaseClient::new(cfg)?;
        let v = client.login_qr_key().await?;
        println!("login_qr_key 响应: {v}");
        let unikey = v
            .pointer("/unikey")
            .and_then(|x| x.as_str())
            .or_else(|| v.pointer("/data/unikey").and_then(|x| x.as_str()))
            .ok_or("未找到 unikey")?;
        println!("unikey: {unikey}");
        println!(
            "qrurl: {}",
            client.login_qr_url(unikey, netease::QrPlatform::Pc)
        );
        return Ok(());
    }

    let (tx, rx) = usecases::actor::spawn_app_actor(cfg);
    tui::run_tui(App::default(), tx, rx).await?;
    Ok(())
}
