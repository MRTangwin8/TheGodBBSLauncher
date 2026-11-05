//! 任务取消。
//!
//! 命令把整个任务放进 `scope(token, …)` 里跑，里面任何地方都能用 `current()` / `check()` 查到这个任务有没有被取消，
//! 不用一层层传参数：下载器（`Downloader::download_all`）会自动停下还没下完的文件，
//! Forge 安装器步骤之间、解压整合包时也会检查。取消后已经下了一半的文件留着 `.part`，下次接着下。
//!
//! `spawn_blocking` 里拿不到 task-local，要先在外面 `current()` 取出来再传进去。

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Notify;

use crate::engine::Error;

#[derive(Clone, Default)]
pub struct CancelToken(Arc<(AtomicBool, Notify)>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0 .0.store(true, Ordering::SeqCst);
        self.0 .1.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.0 .0.load(Ordering::SeqCst)
    }

    /// 取消时完成
    pub async fn cancelled(&self) {
        loop {
            // 先登记等待再检查标志，免得错过在两者之间发出的通知
            let notified = self.0 .1.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }

    pub fn same(&self, other: &CancelToken) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

tokio::task_local! {
    static CURRENT: CancelToken;
}

pub fn error() -> Error {
    Error::coded("cancelled", &[], "已取消")
}

/// 在 `token` 下运行 `fut`
pub async fn scope<F: Future>(token: CancelToken, fut: F) -> F::Output {
    CURRENT.scope(token, fut).await
}

/// 当前任务的取消标志（不在 `scope` 里时为 None）
pub fn current() -> Option<CancelToken> {
    CURRENT.try_with(|t| t.clone()).ok()
}

/// 当前任务已经取消就返回错误
pub fn check() -> crate::engine::Result<()> {
    match current() {
        Some(t) if t.is_cancelled() => Err(error()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn scope_exposes_token_and_check_fails_after_cancel() {
        assert!(current().is_none());
        assert!(check().is_ok(), "不在任务里时不算取消");
        let t = CancelToken::default();
        let t2 = t.clone();
        scope(t.clone(), async move {
            assert!(current().unwrap().same(&t2));
            assert!(check().is_ok());
            t2.cancel();
            assert!(check().is_err());
        })
        .await;
        assert!(t.is_cancelled());
    }

    #[tokio::test]
    async fn cancelled_wakes_waiters_and_returns_at_once_when_already_cancelled() {
        let t = CancelToken::default();
        let waiter = tokio::spawn({
            let t = t.clone();
            async move { t.cancelled().await }
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        t.cancel();
        tokio::time::timeout(Duration::from_secs(1), waiter).await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_millis(50), t.cancelled()).await.unwrap();
    }
}
