//! 启动和结束陶瓦进程。
//!
//! 用 `--hmcl <端口文件>` 启动（这是陶瓦给启动器集成留的模式，名字是历史原因），
//! 它在 HTTP 服务就绪后把 `{"port": N}` 写进文件（先写 .tmp 再改名，读到的一定完整）。
//!
//! 陶瓦全局只跑一个实例：已经有一个在跑时（比如别的程序拉起的），新进程只把已有实例的端口写进文件就退出。
//! 这种实例不归我们管，启动器退出时不能把它关掉，所以要分清 `owned`：
//! 运行中的陶瓦把自己的端口（2 字节大端）写在 `<临时目录>/terracotta/terracotta.lock` 里，
//! 启动前按这个端口问一下 `/meta`，答得上来就说明已经有实例在跑。
//! Windows 上 `--hmcl` 进程只负责引导，总会自己退出，真正干活的是它拉起的子进程；其他系统上我们拉起的就是服务本身。

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::api::Api;
use crate::engine::{Error, Result};

const START_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Process {
    pub port: u16,
    /// 是我们拉起的实例（启动器退出时负责关掉）
    pub owned: bool,
    child: Option<Child>,
}

/// 读端口文件；还没写好或内容不对返回 None
fn read_port(path: &Path) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    v.get("port")?.as_u64().and_then(|p| u16::try_from(p).ok()).filter(|p| *p > 0)
}

fn lock_port(lock: &Path) -> Option<u16> {
    match std::fs::read(lock).ok()?.as_slice() {
        [hi, lo] => Some(u16::from_be_bytes([*hi, *lo])).filter(|p| *p > 0),
        _ => None,
    }
}

/// 已经有陶瓦实例在跑（锁文件里的端口上真的是陶瓦）
async fn instance_running(client: &reqwest::Client) -> bool {
    let lock = std::env::temp_dir().join("terracotta").join("terracotta.lock");
    match lock_port(&lock) {
        Some(port) => Api::new(client.clone(), port).meta().await.is_ok(),
        None => false,
    }
}

pub async fn start(exe: &Path, work_dir: &Path, client: reqwest::Client) -> Result<Process> {
    std::fs::create_dir_all(work_dir)?;
    let port_file = work_dir.join("port.json");
    let _ = std::fs::remove_file(&port_file);
    let _ = std::fs::remove_file(work_dir.join("port.json.tmp"));
    let owned = !instance_running(&client).await;

    let mut cmd = Command::new(exe);
    cmd.arg("--hmcl").arg(&port_file).current_dir(work_dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::coded("mp.start", &[("detail", &e)], format!("陶瓦联机启动失败：{e}")))?;

    let deadline = Instant::now() + START_TIMEOUT;
    let port = loop {
        if let Some(p) = read_port(&port_file) {
            break p;
        }
        let exited = matches!(child.try_wait(), Ok(Some(_)));
        if exited || Instant::now() > deadline {
            // 退出的那一刻文件可能刚写好
            if let Some(p) = read_port(&port_file) {
                break p;
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::coded("mp.startTimeout", &[], "陶瓦联机没能启动（没有按时报告端口）。可以重试一次，或者检查杀毒软件是否拦截了它"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    let _ = std::fs::remove_file(&port_file);

    // Windows 的引导进程很快自己退出；用已有实例时我们拉起的进程写完端口也会退出。
    // 这两种不用留着，后台等它结束就行（免得在 Unix 上留下僵尸进程）
    let child = if owned && !cfg!(windows) {
        Some(child)
    } else {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        None
    };

    Api::new(client, port).meta().await?;
    Ok(Process { port, owned, child })
}

impl Process {
    /// 让陶瓦退出；它不理会就直接结束我们拉起的进程
    pub async fn stop(mut self, api: &Api) {
        let _ = api.shutdown().await;
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(3);
            while matches!(child.try_wait(), Ok(None)) {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_port_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("port.json");
        assert_eq!(read_port(&f), None);
        std::fs::write(&f, r#"{"port": 41234}"#).unwrap();
        assert_eq!(read_port(&f), Some(41234));
        std::fs::write(&f, r#"{"port": 70000}"#).unwrap();
        assert_eq!(read_port(&f), None);
        std::fs::write(&f, r#"{"po"#).unwrap();
        assert_eq!(read_port(&f), None);
    }

    #[test]
    fn reads_lock_file_port() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("terracotta.lock");
        assert_eq!(lock_port(&f), None);
        std::fs::write(&f, [0xfb, 0x8c]).unwrap();
        assert_eq!(lock_port(&f), Some(64396));
        std::fs::write(&f, []).unwrap();
        assert_eq!(lock_port(&f), None, "陶瓦刚启动时会先清空");
    }
}
