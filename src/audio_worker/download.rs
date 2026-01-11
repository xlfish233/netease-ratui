use reqwest::blocking::Client;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;

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

pub(super) fn download_to_file(
    http: &Client,
    out: &mut NamedTempFile,
    url: &str,
    title: &str,
) -> Result<(), String> {
    let mut resp = http
        .get(url)
        .send()
        .map_err(|e| format!("下载音频失败({title}): {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("下载音频失败({title}): HTTP {}", resp.status()));
    }

    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| format!("下载音频失败({title}): {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])
            .map_err(|e| format!("写入临时文件失败({title}): {e}"))?;
    }
    Ok(())
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
