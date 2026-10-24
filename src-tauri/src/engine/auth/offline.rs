use md5::{Digest, Md5};

/// 和 Java `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)` 一致，服务器离线模式也用这个算法
pub fn offline_uuid(name: &str) -> String {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // IETF variant
    hex::encode(bytes)
}

/// 离线名字：3~16 位字母数字下划线（和正版规则一致，避免服务器拒绝）
pub fn is_valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_offline_uuid() {
        // 由 Java UUID.nameUUIDFromBytes("OfflinePlayer:Steve".getBytes(UTF_8)) 得到
        assert_eq!(offline_uuid("Steve"), "5627dd98e6be3c21b8a8e92344183641");
    }

    #[test]
    fn validates_names() {
        assert!(is_valid_name("Steve_01"));
        assert!(!is_valid_name("ab"));
        assert!(!is_valid_name("中文名字"));
        assert!(!is_valid_name("a-b-c"));
    }
}
