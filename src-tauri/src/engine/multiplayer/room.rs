//! 房间码：`U/NNNN-NNNN-SSSS-SSSS`（Scaffolding 协议）。
//!
//! 字符是去掉 I、O 的大写字母和数字，按 0-9、A-H、J-N、P-Z 映射成 0~33，
//! 按小端（第一个字符是最低位）读出的整数必须能被 7 整除，用来发现输错的码。
//! 后端先校验一遍再交给陶瓦，免得把乱七八糟的内容拼进请求里。

const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// 整理用户输入（去空白、转大写、补上 `U/`），合法就返回标准形式
pub fn normalize(input: &str) -> Option<String> {
    let s: String = input.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_uppercase();
    let s = if s.starts_with("U/") { s } else { format!("U/{s}") };
    is_valid(&s).then_some(s)
}

pub fn is_valid(code: &str) -> bool {
    let Some(body) = code.strip_prefix("U/") else { return false };
    let groups: Vec<&str> = body.split('-').collect();
    if groups.len() != 4 || groups.iter().any(|g| g.len() != 4) {
        return false;
    }
    let mut value: u128 = 0;
    let mut weight: u128 = 1;
    for b in groups.concat().bytes() {
        let Some(d) = ALPHABET.iter().position(|&a| a == b) else { return false };
        value += d as u128 * weight;
        weight *= ALPHABET.len() as u128;
    }
    value.is_multiple_of(7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_protocol_example() {
        assert!(is_valid("U/YNZE-U61D-2206-HXRG"));
        assert_eq!(normalize(" u/ynze-u61d-2206-hxrg ").as_deref(), Some("U/YNZE-U61D-2206-HXRG"));
        assert_eq!(normalize("YNZE-U61D-2206-HXRG").as_deref(), Some("U/YNZE-U61D-2206-HXRG"), "忘了写 U/ 也认");
    }

    #[test]
    fn rejects_typos_and_bad_shapes() {
        assert!(!is_valid("U/YNZE-U61D-2206-HXRH"), "改一个字符后校验不过");
        assert!(!is_valid("U/YNZE-U61D-2206-HXR"));
        assert!(!is_valid("U/YNZE-U61D-2206-HXRG-AAAA"));
        assert!(!is_valid("U/YNZE-U61D-22O6-HXRG"), "没有字母 O");
        assert!(!is_valid("X/YNZE-U61D-2206-HXRG"));
        assert!(normalize("U/YNZE&U61D-2206-HXRG").is_none());
        assert!(normalize("").is_none());
    }
}
