use reqwest::StatusCode;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

pub(super) fn clear_dir_files(dir: &Path, keep: Option<&Path>) -> (usize, u64) {
    let mut removed_files = 0usize;
    let mut removed_bytes = 0u64;

    let keep = keep.and_then(|p| p.file_name().map(|n| dir.join(n)));

    let Ok(rd) = fs::read_dir(dir) else {
        return (0, 0);
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            continue;
        }
        if p.file_name().is_some_and(|n| n == "index.json") {
            continue;
        }
        if keep.as_ref().is_some_and(|kp| kp == &p) {
            continue;
        }

        if let Ok(md) = ent.metadata() {
            removed_bytes = removed_bytes.saturating_add(md.len());
        }
        if fs::remove_file(&p).is_ok() {
            removed_files += 1;
        }
    }

    (removed_files, removed_bytes)
}

pub async fn download_to_path_with_config(
    http: &reqwest::Client,
    out_path: &Path,
    url: &str,
    title: &str,
    retries: u32,
    backoff_ms: u64,
    backoff_max_ms: u64,
) -> Result<(), String> {
    for attempt in 0..=retries {
        // Ensure each attempt starts from a clean file.
        let _ = tokio::fs::remove_file(out_path).await;

        let resp = match http.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                if attempt < retries {
                    sleep_backoff(attempt, backoff_ms, backoff_max_ms).await;
                    continue;
                }
                return Err(format!("下载音频失败({title}): {e}"));
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            if attempt < retries && is_retryable_status(status) {
                sleep_backoff(attempt, backoff_ms, backoff_max_ms).await;
                continue;
            }
            return Err(format!("下载音频失败({title}): HTTP {status}"));
        }

        let mut file = match tokio::fs::File::create(out_path).await {
            Ok(f) => f,
            Err(e) => {
                if attempt < retries {
                    sleep_backoff(attempt, backoff_ms, backoff_max_ms).await;
                    continue;
                }
                return Err(format!("创建临时文件失败({title}): {e}"));
            }
        };

        let mut stream = resp.bytes_stream();
        let mut failed = None::<String>;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Err(e) = file.write_all(&bytes).await {
                        failed = Some(format!("写入临时文件失败({title}): {e}"));
                        break;
                    }
                }
                Err(e) => {
                    failed = Some(format!("下载音频失败({title}): {e}"));
                    break;
                }
            }
        }

        if failed.is_none()
            && let Err(e) = file.flush().await
        {
            failed = Some(format!("写入临时文件失败({title}): {e}"));
        }

        if let Some(err) = failed {
            if attempt < retries {
                sleep_backoff(attempt, backoff_ms, backoff_max_ms).await;
                continue;
            }
            return Err(err);
        }

        return Ok(());
    }

    Ok(())
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

async fn sleep_backoff(attempt: u32, base_ms: u64, max_ms: u64) {
    let exp = base_ms.saturating_mul(2u64.saturating_pow(attempt.min(6)));
    let mut ms = exp.min(max_ms);

    // Tiny jitter (0..=250ms) without pulling in RNG deps.
    let jitter = now_ms() % 251;
    ms = ms.saturating_add(jitter).min(max_ms);

    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}
