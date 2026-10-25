//! 令牌加密：AES-256-GCM，密钥放系统钥匙串（Windows 凭据管理器 / macOS 钥匙串 / Secret Service）。
//! 令牌本身写在 accounts.json 里而不是直接进钥匙串，因为 Windows 凭据单条上限 2560 字节，
//! 微软的 refresh token 有时会超。

use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;

use crate::engine::{Error, Result};

const SERVICE: &str = "co.mcbbs.godbbs";
/// 旧版本的钥匙串条目，找不到新条目时从这里接过密钥
const LEGACY_SERVICE: &str = "me.mcbbs.godbbs";
const KEY_USER: &str = "account-vault-key";

pub struct Vault {
    cipher: Aes256Gcm,
}

impl Vault {
    pub fn from_key(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new(key.into()) }
    }

    /// 从钥匙串取密钥，没有就生成一个。钥匙串不可用（比如 Linux 没装 Secret Service）时
    /// 退回到配置目录下的密钥文件，安全性会差一些。
    pub fn open(fallback_key_file: &Path) -> Self {
        let key = keychain_key().unwrap_or_else(|| file_key(fallback_key_file));
        Self::from_key(&key)
    }

    pub fn encrypt(&self, plain: &str) -> Result<String> {
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ct = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plain.as_bytes())
            .map_err(|_| Error::Other("加密失败".into()))?;
        let mut out = nonce.to_vec();
        out.extend(ct);
        Ok(B64.encode(out))
    }

    pub fn decrypt(&self, blob: &str) -> Result<String> {
        let raw = B64.decode(blob).map_err(|_| Error::Other("令牌数据损坏".into()))?;
        if raw.len() < 12 {
            return Err(Error::Other("令牌数据损坏".into()));
        }
        let (nonce, ct) = raw.split_at(12);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|_| Error::Other("无法解密令牌（钥匙串密钥变了？），请重新登录".into()))?;
        String::from_utf8(plain).map_err(|_| Error::Other("令牌数据损坏".into()))
    }
}

fn random_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

fn decode_key(s: &str) -> Option<[u8; 32]> {
    B64.decode(s.trim()).ok()?.try_into().ok()
}

fn keychain_key() -> Option<[u8; 32]> {
    let entry = keyring::Entry::new(SERVICE, KEY_USER).ok()?;
    match entry.get_password() {
        Ok(s) => decode_key(&s),
        Err(keyring::Error::NoEntry) => {
            let legacy = keyring::Entry::new(LEGACY_SERVICE, KEY_USER).ok();
            let legacy_key = legacy.as_ref().and_then(|e| e.get_password().ok()).and_then(|s| decode_key(&s));
            let k = legacy_key.unwrap_or_else(random_key);
            entry.set_password(&B64.encode(k)).ok()?;
            // 写完读一遍，确认钥匙串真的能用；确认之后才删旧条目，免得密钥丢失
            let saved = entry.get_password().ok().and_then(|s| decode_key(&s))?;
            if let (Some(e), Some(_)) = (legacy, legacy_key) {
                let _ = e.delete_credential();
            }
            Some(saved)
        }
        Err(_) => None,
    }
}

fn file_key(path: &Path) -> [u8; 32] {
    if let Some(k) = std::fs::read_to_string(path).ok().and_then(|s| decode_key(&s)) {
        return k;
    }
    let k = random_key();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, B64.encode(k));
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper_detection() {
        let v = Vault::from_key(&[7u8; 32]);
        let blob = v.encrypt("secret-token").unwrap();
        assert_eq!(v.decrypt(&blob).unwrap(), "secret-token");
        assert_ne!(v.encrypt("secret-token").unwrap(), blob, "每次加密 nonce 不同");
        let other = Vault::from_key(&[8u8; 32]);
        assert!(other.decrypt(&blob).is_err());
        assert!(v.decrypt("bm9wZQ==").is_err());
    }
}
