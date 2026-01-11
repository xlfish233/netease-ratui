use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "netease-ratui",
    version,
    about = "网易云音乐 TUI 客户端（Rust + ratatui）"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// 覆盖数据目录（默认走系统 data_local_dir）
    #[arg(long, env = "NETEASE_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// 覆盖日志目录（默认 `{data_dir}/logs`）
    #[arg(long, env = "NETEASE_LOG_DIR")]
    pub log_dir: Option<PathBuf>,

    /// 覆盖日志过滤（等价于设置 RUST_LOG）
    #[arg(long, env = "RUST_LOG")]
    pub log_filter: Option<String>,

    /// 覆盖网易 domain（默认 https://music.163.com）
    #[arg(long, env = "NETEASE_DOMAIN")]
    pub domain: Option<String>,

    /// 覆盖网易 api_domain（默认 https://interface.music.163.com）
    #[arg(long, env = "NETEASE_API_DOMAIN")]
    pub api_domain: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 运行 TUI（默认）
    Tui,

    /// 无交互快速自测：匿名搜索
    SkipLogin {
        #[arg(default_value = "周杰伦")]
        keywords: String,

        #[arg(long, default_value_t = 5)]
        limit: i64,
    },

    /// 打印二维码登录相关信息（便于排查接口返回）
    QrKey,
}
