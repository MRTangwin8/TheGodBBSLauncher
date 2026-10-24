//! 并发下载器：限并发、断点续传、SHA1 校验、失败重试、镜像回退。

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use reqwest::{header, StatusCode};
use serde::Serialize;
use sha1::{Digest, Sha1};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::engine::mirror::{self, Source};
use crate::engine::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadTask {
    /// 官方地址，镜像地址由下载器自动推导
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub total_files: usize,
    /// 已完成（含跳过）
    pub finished_files: usize,
    pub skipped_files: usize,
    pub failed_files: usize,
    /// 已知大小的文件总字节数
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    /// 字节/秒
    pub speed: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadReport {
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: Vec<FailedFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedFile {
    pub path: String,
    pub reason: String,
}

pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

#[derive(Default)]
struct Counters {
    finished: AtomicUsize,
    skipped: AtomicUsize,
    failed: AtomicUsize,
    bytes: AtomicU64,
}

enum Outcome {
    Downloaded,
    Skipped,
}

#[derive(Clone)]
pub struct Downloader {
    client: reqwest::Client,
    pub primary: Source,
    pub concurrency: usize,
    /// 每个源最多尝试几轮
    pub retries: usize,
    /// 已存在的文件是否重新算 SHA1。快速补全仅在已知大小时跳过哈希
    pub verify_existing: bool,
}

impl Downloader {
    pub fn new(client: reqwest::Client, primary: Source, concurrency: usize) -> Self {
        Self {
            client,
            primary,
            concurrency: concurrency.max(1),
            retries: 3,
            verify_existing: true,
        }
    }

    /// 下载一批文件。单个文件失败不会中断其它文件，失败列表放在报告里。
    pub async fn download_all(&self, tasks: Vec<DownloadTask>, on_progress: ProgressFn) -> DownloadReport {
        let mut seen = HashSet::new();
        let tasks: Vec<DownloadTask> = tasks.into_iter().filter(|t| seen.insert(t.path.clone())).collect();

        let total_files = tasks.len();
        let total_bytes: u64 = tasks.iter().filter_map(|t| t.size).sum();
        let counters = Arc::new(Counters::default());
        let done = Arc::new(AtomicBool::new(false));

        let snapshot = {
            let counters = counters.clone();
            move |speed: u64| Progress {
                total_files,
                finished_files: counters.finished.load(Ordering::Relaxed),
                skipped_files: counters.skipped.load(Ordering::Relaxed),
                failed_files: counters.failed.load(Ordering::Relaxed),
                total_bytes,
                downloaded_bytes: counters.bytes.load(Ordering::Relaxed),
                speed,
            }
        };

        // 每 250ms 推一次进度，避免把前端刷爆
        let ticker = {
            let (done, snapshot, on_progress, counters) = (done.clone(), snapshot.clone(), on_progress.clone(), counters.clone());
            tokio::spawn(async move {
                const TICK: Duration = Duration::from_millis(250);
                let mut last = 0u64;
                while !done.load(Ordering::Relaxed) {
                    let now = counters.bytes.load(Ordering::Relaxed);
                    let speed = now.saturating_sub(last) * 1000 / TICK.as_millis() as u64;
                    last = now;
                    on_progress(snapshot(speed));
                    tokio::time::sleep(TICK).await;
                }
            })
        };

        let results: Vec<(DownloadTask, Result<Outcome>)> = futures::stream::iter(tasks)
            .map(|task| {
                let counters = counters.clone();
                async move {
                    let r = self.download_one(&task, &counters).await;
                    match &r {
                        Ok(Outcome::Skipped) => {
                            counters.skipped.fetch_add(1, Ordering::Relaxed);
                            counters.finished.fetch_add(1, Ordering::Relaxed);
                        }
                        Ok(Outcome::Downloaded) => {
                            counters.finished.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            counters.failed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    (task, r)
                }
            })
            .buffer_unordered(self.concurrency)
            .collect()
            .await;

        done.store(true, Ordering::Relaxed);
        let _ = ticker.await;
        on_progress(snapshot(0));

        let mut report = DownloadReport { total: total_files, ..Default::default() };
        for (task, r) in results {
            match r {
                Ok(Outcome::Downloaded) => report.downloaded += 1,
                Ok(Outcome::Skipped) => report.skipped += 1,
                Err(e) => report.failed.push(FailedFile {
                    path: task.path.display().to_string(),
                    reason: e.to_string(),
                }),
            }
        }
        report
    }

    async fn download_one(&self, task: &DownloadTask, counters: &Counters) -> Result<Outcome> {
        if is_complete(task, self.verify_existing).await {
            counters.bytes.fetch_add(task.size.unwrap_or(0), Ordering::Relaxed);
            return Ok(Outcome::Skipped);
        }
        let urls = mirror::candidates(&task.url, self.primary);
        let mut last_err = None;
        for attempt in 0..self.retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
            }
            for url in &urls {
                let mut counted = 0u64;
                match self.fetch(url, task, counters, &mut counted).await {
                    Ok(()) => return Ok(Outcome::Downloaded),
                    Err(e) => {
                        // 失败那次计入的字节退回去，下次续传会重新计入
                        counters.bytes.fetch_sub(counted, Ordering::Relaxed);
                        last_err = Some(e);
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Other("没有可用的下载地址".into())))
    }

    async fn fetch(&self, url: &str, task: &DownloadTask, counters: &Counters, counted: &mut u64) -> Result<()> {
        if let Some(parent) = task.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let part = part_path(&task.path);

        // 断点续传：已有 .part 时先把已有内容喂给 hasher，再用 Range 请求剩下的部分
        let mut hasher = Sha1::new();
        let mut existing = fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
        if task.size.is_some_and(|s| existing >= s) {
            existing = 0;
        }
        if existing > 0 {
            hash_file_into(&part, &mut hasher).await?;
        }

        let mut req = self.client.get(url);
        if existing > 0 {
            req = req.header(header::RANGE, format!("bytes={existing}-"));
        }
        let resp = req.send().await?;
        let status = resp.status();

        let response_size = if status == StatusCode::PARTIAL_CONTENT {
            let total = resp.headers().get(header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| resume_size(value, existing, task.size));
            if existing == 0 || total.is_none() {
                let _ = fs::remove_file(&part).await;
                return Err(Error::Other("服务器返回的续传范围不正确，已清除临时文件，将重新下载".into()));
            }
            total
        } else {
            None
        };

        let mut file = if status == StatusCode::PARTIAL_CONTENT {
            fs::OpenOptions::new().append(true).open(&part).await?
        } else {
            if status == StatusCode::RANGE_NOT_SATISFIABLE {
                let _ = fs::remove_file(&part).await;
            }
            if status != StatusCode::OK {
                return Err(Error::BadStatus { url: url.to_string(), status: status.as_u16() });
            }
            // 服务器不支持续传，从头开始
            hasher = Sha1::new();
            existing = 0;
            fs::File::create(&part).await?
        };
        counters.bytes.fetch_add(existing, Ordering::Relaxed);
        *counted += existing;

        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            counters.bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            *counted += chunk.len() as u64;
        }
        file.flush().await?;
        drop(file);

        if let Some(expected) = &task.sha1 {
            let actual = hex::encode(hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                let _ = fs::remove_file(&part).await;
                return Err(Error::HashMismatch {
                    path: task.path.display().to_string(),
                    expected: expected.clone(),
                    actual,
                });
            }
        }
        if let Some(size) = task.size.or(response_size) {
            let actual = fs::metadata(&part).await?.len();
            if actual != size {
                let _ = fs::remove_file(&part).await;
                return Err(Error::Other(format!("{} 大小不对：期望 {size}，实际 {actual}", task.path.display())));
            }
        }
        fs::rename(&part, &task.path).await?;
        Ok(())
    }
}

/// A suffix response must start at our offset and reach the end of the same file.
fn resume_size(content_range: &str, offset: u64, expected_size: Option<u64>) -> Option<u64> {
    let (unit, value) = content_range.split_once(' ')?;
    if !unit.eq_ignore_ascii_case("bytes") {
        return None;
    }
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u64>().ok()).flatten()
    };
    let (start, end) = (number(start)?, number(end)?);
    let total = if total == "*" { expected_size? } else { number(total)? };
    if start != offset || end < start || end.checked_add(1)? != total || expected_size.is_some_and(|size| size != total) {
        return None;
    }
    Some(total)
}

fn part_path(path: &Path) -> PathBuf {
    let mut s = OsString::from(path.as_os_str());
    s.push(".part");
    PathBuf::from(s)
}

async fn hash_file_into(path: &Path, hasher: &mut Sha1) -> Result<()> {
    let mut f = fs::File::open(path).await?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&buf[..n]);
    }
}

pub async fn sha1_file(path: &Path) -> Result<String> {
    let mut hasher = Sha1::new();
    hash_file_into(path, &mut hasher).await?;
    Ok(hex::encode(hasher.finalize()))
}

/// 本地文件已存在且校验通过（有 SHA1 比 SHA1，否则比大小，都没有就只看存在）
async fn is_complete(task: &DownloadTask, verify_hash: bool) -> bool {
    let Ok(meta) = fs::metadata(&task.path).await else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    if task.size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    match &task.sha1 {
        Some(_) if !verify_hash && task.size.is_some() => true,
        Some(expected) => sha1_file(&task.path).await.is_ok_and(|h| h.eq_ignore_ascii_case(expected)),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sha1_of_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"hello").unwrap();
        assert_eq!(sha1_file(&p).await.unwrap(), "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[tokio::test]
    async fn completeness_check_uses_sha1_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"hello").unwrap();
        let mk = |sha1: Option<&str>, size: Option<u64>| DownloadTask {
            url: String::new(),
            path: p.clone(),
            sha1: sha1.map(str::to_string),
            size,
        };
        assert!(is_complete(&mk(Some("AAF4C61DDCC5E8A2DABEDE0F3B482CD9AEA9434D"), Some(5)), true).await);
        assert!(!is_complete(&mk(Some("0000000000000000000000000000000000000000"), None), true).await);
        assert!(!is_complete(&mk(Some("0000000000000000000000000000000000000000"), None), false).await);
        assert!(is_complete(&mk(Some("0000000000000000000000000000000000000000"), Some(5)), false).await);
        assert!(!is_complete(&mk(None, Some(6)), false).await);
        assert!(is_complete(&mk(None, None), true).await);
    }

    #[test]
    fn part_suffix() {
        assert_eq!(part_path(Path::new("a/b.jar")), PathBuf::from("a/b.jar.part"));
    }
}

#[cfg(test)]
mod http_tests;
