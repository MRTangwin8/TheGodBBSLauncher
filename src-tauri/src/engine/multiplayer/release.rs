//! 陶瓦的发行包：版本和 SHA256 写死在这里，不读接口给的校验值。
//! 陶瓦会在用户机器上运行，只用我们看过的版本；升级陶瓦就是改这张表。
//!
//! 先从 GitHub 下，失败再换 Gitee 镜像（同名文件）；用 BMCLAPI 的用户多半在国内，反过来先试 Gitee。
//! 解压时只取表里列出的文件名，包里的其他条目和目录结构一概不管。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::engine::download::{DownloadTask, Downloader, Progress};
use crate::engine::meta::Platform;
use crate::engine::mirror::Source;
use crate::engine::{Error, Result};

pub const VERSION: &str = "0.4.2";
const GITHUB: &str = "https://github.com/burningtnt/Terracotta/releases/download";
const GITEE: &str = "https://gitee.com/burningtnt/Terracotta/releases/download";

pub struct Asset {
    /// 发行包里的平台名，如 `windows-x86_64`
    target: &'static str,
    sha256: &'static str,
    size: u64,
    /// 解压出来要用的文件：第一个是程序本身
    files: &'static [&'static str],
}

// macOS 版要配合安装包里的后台服务（/Library/LaunchAgents）才能运行，单独的程序起不来，第一期不支持
const ASSETS: &[(&str, &str, Asset)] = &[
    ("windows", "x86_64", Asset {
        target: "windows-x86_64",
        sha256: "07ebe139e3ca5f74576e58b1a96efe59abdfbe148d3f1a49bfdca8b6f70745f0",
        size: 8_135_914,
        files: &["terracotta-0.4.2-windows-x86_64.exe", "VCRUNTIME140.DLL"],
    }),
    ("windows", "arm64", Asset {
        target: "windows-arm64",
        sha256: "acfab0a87a02dedc6dab7c05303186c8907f56f815548b693fb3324358da7d14",
        size: 7_211_225,
        files: &["terracotta-0.4.2-windows-arm64.exe", "VCRUNTIME140.DLL"],
    }),
    ("linux", "x86_64", Asset {
        target: "linux-x86_64",
        sha256: "675c4fd6c74d49ed8165151ba2be5b6582e0af20fb6d912074543c2484b1e10a",
        size: 10_165_199,
        files: &["terracotta-0.4.2-linux-x86_64"],
    }),
    ("linux", "arm64", Asset {
        target: "linux-arm64",
        sha256: "845285ff264ac5fbc16db1a1605ad190e7fa64196516068cc309de5a1d2bf66d",
        size: 8_834_068,
        files: &["terracotta-0.4.2-linux-arm64"],
    }),
];

pub fn asset(p: &Platform) -> Option<&'static Asset> {
    ASSETS.iter().find(|(os, arch, _)| *os == p.os && *arch == p.arch).map(|(_, _, a)| a)
}

impl Asset {
    fn archive_name(&self) -> String {
        format!("terracotta-{VERSION}-{}-pkg.tar.gz", self.target)
    }

    fn urls(&self, source: Source) -> [String; 2] {
        let name = self.archive_name();
        let github = format!("{GITHUB}/v{VERSION}/{name}");
        let gitee = format!("{GITEE}/v{VERSION}/{name}");
        match source {
            Source::Bmclapi => [gitee, github],
            _ => [github, gitee],
        }
    }
}

/// 陶瓦放在 `runtime/terracotta/<版本>/`
pub fn install_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("terracotta").join(VERSION)
}

/// 已经下载好的程序路径
pub fn installed_exe(runtime_dir: &Path, p: &Platform) -> Option<PathBuf> {
    let a = asset(p)?;
    let dir = install_dir(runtime_dir);
    a.files.iter().all(|f| dir.join(f).is_file()).then(|| dir.join(a.files[0]))
}

pub async fn install(
    downloader: &Downloader,
    runtime_dir: &Path,
    p: &Platform,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<PathBuf> {
    let a = asset(p).ok_or_else(|| unsupported(p))?;
    let tmp_dir = runtime_dir.join("terracotta").join(".download");
    let file = tmp_dir.join(a.archive_name());
    let mut reason = String::new();
    for url in a.urls(downloader.primary) {
        let _ = std::fs::remove_file(&file);
        let task = DownloadTask { url, path: file.clone(), sha1: None, size: Some(a.size) };
        let report = downloader.download_all(vec![task], on_progress.clone()).await;
        match report.failed.first() {
            None => {
                reason.clear();
                break;
            }
            Some(f) => reason = f.reason.clone(),
        }
    }
    if !reason.is_empty() {
        return Err(Error::coded("mp.download", &[("reason", &reason)], format!("陶瓦联机下载失败：{reason}")));
    }
    let dest = install_dir(runtime_dir);
    let (archive, dest2) = (file.clone(), dest.clone());
    let result = tokio::task::spawn_blocking(move || verify_and_unpack(&archive, a, &dest2))
        .await
        .map_err(|e| Error::Other(format!("解压陶瓦联机时出错：{e}")))?;
    let _ = std::fs::remove_file(&file);
    result?;
    Ok(dest.join(a.files[0]))
}

pub fn unsupported(p: &Platform) -> Error {
    let platform = format!("{} {}", p.os, p.arch);
    Error::coded("mp.unsupported", &[("platform", &platform)], format!("联机功能暂时不支持这个平台（{platform}）"))
}

fn verify_and_unpack(file: &Path, a: &Asset, dest: &Path) -> Result<()> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::fs::File::open(file)?, &mut hasher)?;
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(a.sha256) {
        return Err(Error::coded("mp.checksum", &[], format!("陶瓦联机安装包校验失败（SHA256 期望 {}，实际 {actual}），已丢弃", a.sha256)));
    }
    unpack(std::fs::File::open(file)?, a.files, dest)
}

/// 只取 `wanted` 里的文件（按文件名匹配，不认目录），全部取到才放到 `dest`
fn unpack(reader: impl Read, wanted: &[&str], dest: &Path) -> Result<()> {
    let parent = dest.parent().ok_or_else(|| Error::Other("陶瓦联机的安装目录不对".into()))?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(".unpacking");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let result = (|| {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
        let mut found = Vec::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = entry.path()?.into_owned();
            let Some(name) = path.file_name().and_then(|n| n.to_str()).filter(|n| wanted.contains(n)) else { continue };
            if path.components().count() != 1 || found.iter().any(|f| f == name) {
                continue;
            }
            let out = tmp.join(name);
            std::io::copy(&mut entry, &mut std::fs::File::create(&out)?)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755))?;
            }
            found.push(name.to_string());
        }
        if let Some(missing) = wanted.iter().find(|w| !found.iter().any(|f| f == *w)) {
            return Err(Error::Other(format!("陶瓦联机安装包里缺少 {missing}")));
        }
        let _ = std::fs::remove_dir_all(dest);
        std::fs::rename(&tmp, dest)?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tgz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (name, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            builder.append_data(&mut h, name, *data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn picks_only_listed_top_level_files() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("terracotta").join(VERSION);
        let data = tgz(&[("app.exe", b"exe"), ("extra.txt", b"x"), ("sub/lib.dll", b"evil"), ("lib.dll", b"dll")]);
        unpack(&data[..], &["app.exe", "lib.dll"], &dest).unwrap();
        assert_eq!(std::fs::read(dest.join("app.exe")).unwrap(), b"exe");
        assert_eq!(std::fs::read(dest.join("lib.dll")).unwrap(), b"dll", "子目录里的同名文件不算");
        assert!(!dest.join("extra.txt").exists());
        assert!(!dest.join("sub").exists());
    }

    #[test]
    fn missing_file_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("terracotta").join(VERSION);
        let data = tgz(&[("app.exe", b"exe")]);
        assert!(unpack(&data[..], &["app.exe", "lib.dll"], &dest).is_err());
        assert!(!dest.exists());
        assert!(!dir.path().join("terracotta").join(".unpacking").exists());
    }

    #[test]
    fn asset_table_is_consistent() {
        for (os, arch, a) in ASSETS {
            assert!(a.archive_name().contains(a.target));
            assert!(a.files[0].starts_with(&format!("terracotta-{VERSION}-")), "{os} {arch}");
            assert_eq!(a.sha256.len(), 64);
            assert!(a.urls(Source::Official)[0].starts_with(GITHUB));
            assert!(a.urls(Source::Bmclapi)[0].starts_with(GITEE));
        }
        let mac = Platform { os: "osx", arch: "arm64", os_version: String::new() };
        assert!(asset(&mac).is_none(), "macOS 第一期不支持");
    }
}
