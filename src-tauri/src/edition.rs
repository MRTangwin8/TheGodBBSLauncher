//! 发行版本：普通版（GitHub 发布）和微软商店版（`--features store` 编译，CI 打成 MSIX）。
//!
//! 商店版的规矩：
//! - 只能用微软正版账号启动，离线账号和外置登录都不能添加、也不能启动。正版身份在登录和每次刷新令牌时
//!   都会向 Minecraft 服务确认这个账号买了 Java 版（`engine::auth::microsoft` 里查游戏档案）。
//! - 不在应用内更新，由商店负责更新（商店政策要求）。
//!
//! 界面那边用 `VITE_STORE=1` 构建出对应的样子（`src/edition.ts`）；真正的限制都在后端，界面只是不显示用不了的入口。

use crate::engine::auth::AccountKind;
use crate::engine::{Error, Result};

pub const STORE: bool = cfg!(feature = "store");

/// 商店版不允许的账号类型
pub fn check_account_kind(kind: AccountKind) -> Result<()> {
    check_kind(STORE, kind)
}

fn check_kind(store: bool, kind: AccountKind) -> Result<()> {
    if store && kind != AccountKind::Microsoft {
        return Err(Error::Other("微软商店版只能使用正版 Minecraft 账号：请在「个人空间」用拥有 Java 版的微软账号登录".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_edition_accepts_only_microsoft_accounts() {
        assert!(check_kind(true, AccountKind::Microsoft).is_ok());
        assert!(check_kind(true, AccountKind::Offline).is_err());
        assert!(check_kind(true, AccountKind::Yggdrasil).is_err());
        for kind in [AccountKind::Microsoft, AccountKind::Offline, AccountKind::Yggdrasil] {
            assert!(check_kind(false, kind).is_ok(), "普通版不限制");
        }
    }
}
