//! Mojang 没有提供的 Java 从 Azul Zulu 下载（Apple 芯片 / Linux ARM 上的 Java 8、Linux ARM 上的所有版本）。
//!
//! - 版本信息来自 api.azul.com，下载地址只接受 https://cdn.azul.com/，下完按接口给的 SHA256 核对。
//! - 解压到临时文件夹，找到 bin/java 所在的 Java 根目录后整个挪到 `runtime/zulu-<主版本>`，
//!   这样下次扫描本机 Java 时直接能找到，不会重复下载。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{find_java_under, JAVA_EXE};
use crate::engine::download::{DownloadTask, Downloader, Progress};
use crate::engine::meta::Platform;
use crate::engine::{Error, Result};

const API: &str = "https://api.azul.com/metadata/v1/zulu/packages/";
const CDN: &str = "https://cdn.azul.com/";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Package {
    pub name: String,
    pub download_url: String,
    pub sha256_hash: String,
    pub size: u64,
    pub java_version: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Archive {
    Zip,
    TarGz,
}

/// Azul 接口里的系统、架构、压缩包格式
fn target(p: &Platform) -> Option<(&'static str, &'static str, Archive)> {
    let os = match p.os {
        "windows" => "windows",
        "osx" => "macos",
        "linux" => "linux",
        _ => return None,
    };
    let arch = match p.arch {
        "x86_64" => "x64",
        "x86" => "i686",
        "arm64" => "aarch64",
        _ => return None,
    };
    Some((os, arch, if os == "linux" { Archive::TarGz } else { Archive::Zip }))
}

/// 依次尝试的主版本：老版本只认原版本，16 及以上可以用更高的长期支持版
fn majors_to_try(major: u32) -> Vec<u32> {
    let mut out = vec![major];
    if major >= 16 {
        out.extend([17, 21, 25].into_iter().filter(|m| *m > major));
    }
    out
}

fn query_url(os: &str, arch: &str, archive: Archive, major: u32) -> String {
    let ext = match archive {
        Archive::Zip => "zip",
        Archive::TarGz => "tar.gz",
    };
    format!(
        "{API}?java_version={major}&os={os}&arch={arch}&archive_type={ext}&java_package_type=jre&javafx_bundled=false\
         &release_status=ga&availability_types=CA&latest=true&page_size=10&include_fields=sha256_hash,size"
    )
}

/// 接口返回的东西不全信：下载地址必须在 Azul 的 CDN 上，文件名不能带路径，校验值得是 SHA256
fn validate(p: &Package, major: u32) -> Result<()> {
    let ok = p.download_url.starts_with(CDN)
        && !p.name.is_empty()
        && !p.name.contains(['/', '\\'])
        && !p.name.starts_with('.')
        && p.sha256_hash.len() == 64
        && p.sha256_hash.chars().all(|c| c.is_ascii_hexdigit())
        && p.java_version.first() == Some(&major);
    if ok {
        Ok(())
    } else {
        Err(Error::Other(format!("Azul 返回的下载信息不对，没有下载：{}", p.download_url)))
    }
}

async fn find_package(client: &reqwest::Client, platform: &Platform, major: u32) -> Result<Option<(Package, Archive)>> {
    let (os, arch, archive) = target(platform).ok_or_else(|| Error::Other("这个平台没有可以自动下载的 Java，请手动安装".into()))?;
    for m in majors_to_try(major) {
        let resp = client.get(query_url(os, arch, archive, m)).send().await?;
        if !resp.status().is_success() {
            return Err(Error::BadStatus { url: API.into(), status: resp.status().as_u16() });
        }
        // Linux 上有 musl 版（Alpine 用的），常见发行版跑不起来，跳过
        if let Some(p) = resp.json::<Vec<Package>>().await?.into_iter().find(|p| !p.name.contains("musl")) {
            validate(&p, m)?;
            return Ok(Some((p, archive)));
        }
    }
    Ok(None)
}

/// 下载 Zulu JRE，返回 java 路径
pub(super) async fn install(
    downloader: &Downloader,
    client: &reqwest::Client,
    runtime_root: &Path,
    major: u32,
    platform: &Platform,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<PathBuf> {
    let query = crate::engine::http::within(crate::engine::http::SEARCH_TIMEOUT, find_package(client, platform, major));
    let (pkg, archive) = query.await?.ok_or_else(|| {
        Error::Other(format!("官方和 Azul 都没有这个平台（{} {}）的 Java {major}，请手动安装后在设置里选择", platform.os, platform.arch))
    })?;
    let dl_dir = runtime_root.join(".download");
    let file = dl_dir.join(&pkg.name);
    let task = DownloadTask { url: pkg.download_url.clone(), path: file.clone(), sha1: None, size: Some(pkg.size) };
    let report = downloader.download_all(vec![task], on_progress).await;
    if let Some(f) = report.failed.first() {
        return Err(Error::Other(format!("Java 下载失败：{}", f.reason)));
    }
    let dir_name = format!("zulu-{}", pkg.java_version[0]);
    let (root, sha) = (runtime_root.to_path_buf(), pkg.sha256_hash.clone());
    tokio::task::spawn_blocking(move || {
        let r = verify_and_place(&file, &sha, archive, &root, &dir_name);
        let _ = std::fs::remove_file(&file);
        r
    })
    .await
    .map_err(|e| Error::Other(format!("解压 Java 时出错：{e}")))?
}

fn verify_and_place(file: &Path, sha256: &str, archive: Archive, runtime_root: &Path, dir_name: &str) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut std::fs::File::open(file)?, &mut hasher)?;
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(sha256) {
        return Err(Error::Other(format!("下载的 Java 校验失败（SHA256 期望 {sha256}，实际 {actual}），已丢弃")));
    }
    unpack_and_place(file, archive, runtime_root, dir_name)
}

/// 解压到临时文件夹 → 找到 Java 根目录 → 挪到 `runtime_root/dir_name`
fn unpack_and_place(file: &Path, archive: Archive, runtime_root: &Path, dir_name: &str) -> Result<PathBuf> {
    let tmp = runtime_root.join(format!(".{dir_name}.unpacking"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let result = (|| {
        let zerr = |e: zip::result::ZipError| Error::Other(format!("Java 压缩包损坏：{e}"));
        match archive {
            // zip / tar 的解压都会拒绝 ../ 和绝对路径
            Archive::Zip => zip::ZipArchive::new(std::fs::File::open(file)?).map_err(zerr)?.extract(&tmp).map_err(zerr)?,
            Archive::TarGz => tar::Archive::new(flate2::read::GzDecoder::new(std::fs::File::open(file)?)).unpack(&tmp)?,
        }
        let mut found = Vec::new();
        find_java_under(&tmp, 6, &mut found);
        let java = found.into_iter().next().ok_or_else(|| Error::Other("压缩包里没找到 java".into()))?;
        let home = java.parent().and_then(Path::parent).ok_or_else(|| Error::Other("压缩包结构不对".into()))?.to_path_buf();
        let dest = runtime_root.join(dir_name);
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::rename(&home, &dest)?;
        Ok(dest.join("bin").join(JAVA_EXE))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn maps_platforms_and_versions() {
        let p = |os: &'static str, arch: &'static str| Platform { os, arch, os_version: String::new() };
        assert_eq!(target(&p("osx", "arm64")), Some(("macos", "aarch64", Archive::Zip)));
        assert_eq!(target(&p("linux", "arm64")), Some(("linux", "aarch64", Archive::TarGz)));
        assert_eq!(majors_to_try(8), vec![8]);
        assert_eq!(majors_to_try(16), vec![16, 17, 21, 25]);
        assert_eq!(majors_to_try(21), vec![21, 25]);
        let url = query_url("macos", "aarch64", Archive::Zip, 8);
        assert!(url.starts_with(API) && url.contains("java_version=8&os=macos&arch=aarch64&archive_type=zip&java_package_type=jre"));
        assert!(url.contains("include_fields=sha256_hash,size"));
    }

    #[test]
    fn rejects_untrusted_package_info() {
        let good = Package {
            name: "zulu8.98.0.107-ca-jre8.0.512-macosx_aarch64.zip".into(),
            download_url: "https://cdn.azul.com/zulu/bin/zulu8.98.0.107-ca-jre8.0.512-macosx_aarch64.zip".into(),
            sha256_hash: "5c44f6a707ab847172e32b35a4879aabc0777330e594a2cdf04fa0437b797b20".into(),
            size: 40_348_000,
            java_version: vec![8, 0, 512],
        };
        assert!(validate(&good, 8).is_ok());
        assert!(validate(&good, 17).is_err(), "主版本对不上");
        assert!(validate(&Package { download_url: "https://evil.example/zulu.zip".into(), ..good.clone() }, 8).is_err());
        assert!(validate(&Package { download_url: "https://cdn.azul.com.evil.example/x.zip".into(), ..good.clone() }, 8).is_err());
        assert!(validate(&Package { name: "../x.zip".into(), ..good.clone() }, 8).is_err());
        assert!(validate(&Package { sha256_hash: "abc".into(), ..good }, 8).is_err());
    }

    /// 造一个和 Zulu macOS 包一样深的 zip：<包名>/zulu-8.jre/Contents/Home/bin/java
    fn sample_zip(path: &Path) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let opt = zip::write::SimpleFileOptions::default();
        let home = "zulu8-ca-jre8.0.512-macosx_aarch64/zulu-8.jre/Contents/Home";
        w.start_file(format!("{home}/bin/{JAVA_EXE}"), opt).unwrap();
        w.write_all(b"java").unwrap();
        w.start_file(format!("{home}/lib/rt.jar"), opt).unwrap();
        w.write_all(b"rt").unwrap();
        w.start_file(format!("{home}/release"), opt).unwrap();
        w.write_all(b"JAVA_VERSION=\"1.8.0_512\"\n").unwrap();
        w.finish().unwrap();
    }

    #[test]
    fn verifies_and_places_zip() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("pkg.zip");
        sample_zip(&file);
        let sha = hex::encode(Sha256::digest(std::fs::read(&file).unwrap()));
        let runtime = t.path().join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();

        assert!(verify_and_place(&file, &"0".repeat(64), Archive::Zip, &runtime, "zulu-8").is_err(), "校验值不对不解压");
        assert!(!runtime.join("zulu-8").exists());

        let java = verify_and_place(&file, &sha, Archive::Zip, &runtime, "zulu-8").unwrap();
        assert_eq!(java, runtime.join("zulu-8/bin").join(JAVA_EXE));
        assert!(java.is_file() && runtime.join("zulu-8/lib/rt.jar").is_file());
        let left: Vec<_> = std::fs::read_dir(&runtime).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "临时文件夹要清掉：{left:?}");
        // 再装一次覆盖旧的
        verify_and_place(&file, &sha, Archive::Zip, &runtime, "zulu-8").unwrap();
    }

    #[test]
    fn places_tar_gz() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("pkg.tar.gz");
        {
            let gz = flate2::write::GzEncoder::new(std::fs::File::create(&file).unwrap(), flate2::Compression::fast());
            let mut b = tar::Builder::new(gz);
            for (name, data) in [("zulu21-linux_aarch64/bin/java", &b"java"[..]), ("zulu21-linux_aarch64/lib/modules", b"m")] {
                let mut h = tar::Header::new_gnu();
                h.set_size(data.len() as u64);
                h.set_mode(0o755);
                h.set_cksum();
                b.append_data(&mut h, name, data).unwrap();
            }
            b.into_inner().unwrap().finish().unwrap();
        }
        let runtime = t.path().join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        // Windows 上 JAVA_EXE 是 java.exe，tar 里的 bin/java 找不到 → 报错；其他系统正常放好
        let r = unpack_and_place(&file, Archive::TarGz, &runtime, "zulu-21");
        if cfg!(windows) {
            assert!(r.is_err());
        } else {
            assert!(r.unwrap().is_file());
        }
    }

    #[test]
    fn rejects_zip_without_java() {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("bad.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&file).unwrap());
        w.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
        w.finish().unwrap();
        assert!(unpack_and_place(&file, Archive::Zip, t.path(), "zulu-8").is_err());
        assert!(!t.path().join("zulu-8").exists() && !t.path().join(".zulu-8.unpacking").exists());
    }
}
