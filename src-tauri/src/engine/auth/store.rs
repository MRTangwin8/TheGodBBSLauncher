//! 账号列表持久化：accounts.json，令牌字段用 Vault 加密。

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::vault::Vault;
use super::{Account, Secret};
use crate::engine::{Error, Result};

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountsFile {
    #[serde(default)]
    accounts: Vec<Account>,
    #[serde(default)]
    selected: Option<String>,
    /// 账号 id → 加密后的 Secret JSON
    #[serde(default)]
    secrets: HashMap<String, String>,
}

pub struct AccountStore {
    path: PathBuf,
    vault: Vault,
    data: AccountsFile,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountList {
    pub accounts: Vec<Account>,
    pub selected: Option<String>,
}

impl AccountStore {
    pub fn load(path: PathBuf, vault: Vault) -> Self {
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, vault, data }
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, serde_json::to_string_pretty(&self.data)?)?;
        Ok(())
    }

    pub fn list(&self) -> AccountList {
        AccountList { accounts: self.data.accounts.clone(), selected: self.data.selected.clone() }
    }

    pub fn selected(&self) -> Option<&Account> {
        let id = self.data.selected.as_ref()?;
        self.data.accounts.iter().find(|a| &a.id == id)
    }

    /// 新增或更新（同类型、同 UUID、同服务器视为同一个账号），并设为当前账号
    pub fn upsert(&mut self, mut account: Account, secret: Option<&Secret>) -> Result<Account> {
        if let Some(existing) = self
            .data
            .accounts
            .iter_mut()
            .find(|a| a.kind == account.kind && a.uuid == account.uuid && a.server == account.server)
        {
            account.id = existing.id.clone();
            *existing = account.clone();
        } else {
            self.data.accounts.push(account.clone());
        }
        if let Some(s) = secret {
            self.data.secrets.insert(account.id.clone(), self.vault.encrypt(&serde_json::to_string(s)?)?);
        }
        self.data.selected = Some(account.id.clone());
        self.save()?;
        Ok(account)
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.data.accounts.retain(|a| a.id != id);
        self.data.secrets.remove(id);
        if self.data.selected.as_deref() == Some(id) {
            self.data.selected = self.data.accounts.first().map(|a| a.id.clone());
        }
        self.save()
    }

    pub fn select(&mut self, id: &str) -> Result<()> {
        if !self.data.accounts.iter().any(|a| a.id == id) {
            return Err(Error::Other("账号不存在".into()));
        }
        self.data.selected = Some(id.to_string());
        self.save()
    }

    pub fn secret(&self, id: &str) -> Result<Option<Secret>> {
        match self.data.secrets.get(id) {
            Some(blob) => Ok(Some(serde_json::from_str(&self.vault.decrypt(blob)?)?)),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::auth::AccountKind;

    fn acc(name: &str, uuid: &str) -> Account {
        Account { id: uuid::Uuid::new_v4().to_string(), kind: AccountKind::Microsoft, name: name.into(), uuid: uuid.into(), server: None, server_name: None }
    }

    #[test]
    fn persists_and_encrypts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let secret = Secret { access_token: "tok-123".into(), ..Default::default() };
        let mut s = AccountStore::load(path.clone(), Vault::from_key(&[1; 32]));
        let a = s.upsert(acc("Alex", "u1"), Some(&secret)).unwrap();
        // 同一个人重新登录：更新而不是新增
        let renamed = s.upsert(acc("Alex2", "u1"), None).unwrap();
        assert_eq!(renamed.id, a.id);
        assert_eq!(s.list().accounts.len(), 1);

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("tok-123"), "令牌不能明文落盘");

        let s2 = AccountStore::load(path, Vault::from_key(&[1; 32]));
        assert_eq!(s2.selected().unwrap().name, "Alex2");
        assert_eq!(s2.secret(&a.id).unwrap().unwrap().access_token, "tok-123");
    }

    #[test]
    fn removing_selected_picks_another() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AccountStore::load(dir.path().join("a.json"), Vault::from_key(&[1; 32]));
        let a = s.upsert(acc("A", "u1"), None).unwrap();
        let b = s.upsert(acc("B", "u2"), None).unwrap();
        s.remove(&b.id).unwrap();
        assert_eq!(s.list().selected.as_deref(), Some(a.id.as_str()));
    }
}
