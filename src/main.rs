mod app;
mod audio_worker;
mod core;
mod domain;
mod error;
mod features;
mod logging;
mod messages;
mod netease;
mod player_state;
mod settings;
mod source;
mod ui;

use app::{App, AppSnapshot};
use audio_worker::AudioBackend;
use clap::Parser;
use error::AppError;
use netease::{NeteaseClient, NeteaseClientConfig};
use std::env;
use ui::{Cli, Command, run_tui};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();

    let mut cfg = NeteaseClientConfig::default();
    if let Some(v) = cli.data_dir.clone() {
        cfg.data_dir = v;
    }
    if let Some(v) = cli.domain.clone() {
        cfg.domain = v;
    }
    if let Some(v) = cli.api_domain.clone() {
        cfg.api_domain = v;
    }

    let no_audio_env = env::var("NETEASE_NO_AUDIO")
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    let audio_backend = if cli.no_audio || no_audio_env {
        AudioBackend::Null
    } else {
        AudioBackend::Real
    };

    let _log_guard = logging::init(
        &cfg.data_dir,
        logging::LogConfig {
            dir: cli.log_dir.clone(),
            filter: cli.log_filter.clone(),
        },
    );
    tracing::info!(data_dir = %cfg.data_dir.display(), "netease-ratui 启动");

    // 兼容旧环境变量（后续可考虑 deprecate）
    if cli.command.is_none() && env::var("NETEASE_SKIP_LOGIN").ok().as_deref() == Some("1") {
        tracing::info!("启动模式: NETEASE_SKIP_LOGIN=1");
        let mut client = NeteaseClient::new(cfg)?;
        client.ensure_anonymous().await?;
        let search = client.cloudsearch("周杰伦", 1, 5, 0).await?;
        println!("搜索结果(前5首): {}", search);
        return Ok(());
    }
    if cli.command.is_none() && env::var("NETEASE_QR_KEY").ok().as_deref() == Some("1") {
        tracing::info!("启动模式: NETEASE_QR_KEY=1");
        let mut client = NeteaseClient::new(cfg)?;
        let v = client.login_qr_key().await?;
        println!("login_qr_key 响应: {v}");
        let unikey = v
            .pointer("/unikey")
            .and_then(|x| x.as_str())
            .or_else(|| v.pointer("/data/unikey").and_then(|x| x.as_str()))
            .ok_or_else(|| AppError::Other("未找到 unikey".to_owned()))?;
        println!("unikey: {unikey}");
        println!(
            "qrurl: {}",
            client.login_qr_url(unikey, netease::QrPlatform::Pc)
        );
        return Ok(());
    }

    match cli.command.unwrap_or(Command::Tui) {
        Command::Tui => {
            let (tx, rx) = core::spawn_app_actor(cfg, audio_backend);
            run_tui(AppSnapshot::from_app(&App::default()), tx, rx).await?;
            Ok(())
        }
        Command::SkipLogin { keywords, limit } => {
            tracing::info!("启动模式: SkipLogin");
            let mut client = NeteaseClient::new(cfg)?;
            client.ensure_anonymous().await?;
            let search = client.cloudsearch(&keywords, 1, limit, 0).await?;
            println!("搜索结果(前{limit}首): {search}");
            Ok(())
        }
        Command::QrKey => {
            tracing::info!("启动模式: QrKey");
            let mut client = NeteaseClient::new(cfg)?;
            let v = client.login_qr_key().await?;
            println!("login_qr_key 响应: {v}");
            let unikey = v
                .pointer("/unikey")
                .and_then(|x| x.as_str())
                .or_else(|| v.pointer("/data/unikey").and_then(|x| x.as_str()))
                .ok_or_else(|| AppError::Other("未找到 unikey".to_owned()))?;
            println!("unikey: {unikey}");
            println!(
                "qrurl: {}",
                client.login_qr_url(unikey, netease::QrPlatform::Pc)
            );
            Ok(())
        }
    }
}
