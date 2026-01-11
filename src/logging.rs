use std::fs;
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

pub struct LogGuard(#[allow(dead_code)] Option<WorkerGuard>);

#[derive(Debug, Clone, Default)]
pub struct LogConfig {
    pub dir: Option<PathBuf>,
    pub filter: Option<String>,
}

pub fn init(data_dir: &Path, cfg: LogConfig) -> LogGuard {
    let log_dir = cfg.dir.unwrap_or_else(|| data_dir.join("logs"));

    let log_dir = match fs::create_dir_all(&log_dir) {
        Ok(()) => log_dir,
        Err(_) => std::env::temp_dir().join("netease-ratui-logs"),
    };
    let _ = fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "netease-ratui.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter = match cfg.filter {
        Some(s) if !s.trim().is_empty() => EnvFilter::new(s),
        _ => EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,reqwest=warn,hyper=warn")),
    };

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(file_writer);

    let subscriber = tracing_subscriber::registry().with(filter).with(file_layer);

    let _ = subscriber.try_init();
    tracing::info!(log_dir = %log_dir.display(), "tracing 已初始化");

    LogGuard(Some(guard))
}
