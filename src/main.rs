mod api_worker;
mod app;
mod netease;
mod tui;

use app::App;
use netease::{NeteaseClient, NeteaseClientConfig};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = NeteaseClientConfig::default();

    // 便于无交互环境快速验证 service layer
    if env::var("NETEASE_SKIP_LOGIN").ok().as_deref() == Some("1") {
        let mut client = NeteaseClient::new(cfg)?;
        client.ensure_anonymous().await?;
        let search = client.cloudsearch("周杰伦", 1, 5, 0).await?;
        println!("搜索结果(前5首): {}", search);
        return Ok(());
    }

    let (tx, rx) = api_worker::spawn_api_worker(cfg);
    tui::run_tui(App::default(), tx, rx).await?;
    Ok(())
}
