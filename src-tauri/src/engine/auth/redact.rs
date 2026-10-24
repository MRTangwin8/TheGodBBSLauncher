//! Remove credentials before game output crosses the IPC boundary or enters diagnostics.

use std::sync::OnceLock;

use regex::Regex;

#[derive(Clone, Default)]
pub struct LogRedactor {
    secrets: Vec<String>,
}

impl LogRedactor {
    pub fn new(secrets: impl IntoIterator<Item = String>) -> Self {
        let mut secrets: Vec<String> = secrets.into_iter().filter(|s| !s.is_empty()).collect();
        secrets.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        secrets.dedup();
        Self { secrets }
    }

    pub fn redact(&self, text: &str) -> String {
        let mut text = text.to_string();
        for secret in &self.secrets {
            text = text.replace(secret, "******");
        }

        static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
        let patterns = PATTERNS.get_or_init(|| {
            [
                r#"(?i)(\b(?:access[_-]?token|refresh[_-]?token|client[_-]?token|device[_-]?code|password)\b["']?\s*(?::|=|\s)\s*)(?:"[^"]*"|'[^']*'|[^\s,;<>"']+)"#,
                r"(?i)(\b(?:bearer|basic)\s+)[A-Za-z0-9._~+/\-=]+",
                r#"(?i)(\btoken:)[^:\s"'<>]+"#,
            ]
            .into_iter()
            .map(|pattern| Regex::new(pattern).expect("fixed credential pattern"))
            .collect()
        });
        for pattern in patterns {
            text = pattern.replace_all(&text, |caps: &regex::Captures| format!("{}******", &caps[1])).into_owned();
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_known_tokens_including_overlapping_values() {
        let redactor = LogRedactor::new(["short".into(), "short-long".into(), String::new()]);
        assert_eq!(redactor.redact("short-long / short / normal"), "****** / ****** / normal");
        assert_eq!(LogRedactor::new([String::new()]).redact("normal"), "normal");
    }

    #[test]
    fn removes_named_credentials_from_arguments_and_json() {
        let redactor = LogRedactor::default();
        for text in [
            "--accessToken secret-value --version 1.20.1",
            "--access-token=secret-value",
            r#"{"access_token":"secret-value","refreshToken":"secret-refresh"}"#,
            "clientToken = secret-value",
            r#"device_code: 'secret value with spaces'"#,
            r#"password="secret value with spaces""#,
        ] {
            let clean = redactor.redact(text);
            assert!(!clean.contains("secret"), "{clean}");
            assert!(clean.contains("******"), "{clean}");
        }
    }

    #[test]
    fn removes_authorization_headers_and_legacy_sessions() {
        let redactor = LogRedactor::default();
        assert_eq!(redactor.redact("Authorization: Bearer abc.123_+-/="), "Authorization: Bearer ******");
        assert_eq!(redactor.redact("Authorization: Basic YWJjOmRlZg=="), "Authorization: Basic ******");
        assert_eq!(redactor.redact("--session token:secret:player-uuid"), "--session token:******:player-uuid");
    }

    #[test]
    fn preserves_diagnostic_context_and_is_idempotent() {
        let redactor = LogRedactor::default();
        let ordinary = "[ERROR] Missing dependency fabric-api; java.lang.OutOfMemoryError";
        assert_eq!(redactor.redact(ordinary), ordinary);
        let clean = redactor.redact(r#"{"access_token":"secret","version":"1.20.1"}"#);
        assert!(clean.contains(r#""version":"1.20.1""#));
        assert_eq!(redactor.redact(&clean), clean);
    }
}
