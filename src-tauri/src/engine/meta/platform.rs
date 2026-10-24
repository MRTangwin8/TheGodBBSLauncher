/// 当前运行平台，用 Mojang version.json 里的命名（windows / osx / linux，x86 / x86_64 / arm64）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
    /// 系统版本号，只用于少数 `os.version` 规则；拿不到时留空，规则视为匹配。
    pub os_version: String,
}

impl Platform {
    pub fn current() -> Self {
        let os = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            _ => "linux",
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "x86",
            "aarch64" => "arm64",
            "arm" => "arm32",
            _ => "x86_64",
        };
        Self {
            os,
            arch,
            os_version: String::new(),
        }
    }

    /// 旧版 natives 分类名里的 `${arch}` 占位符
    pub fn arch_bits(&self) -> &'static str {
        if self.arch == "x86" || self.arch == "arm32" {
            "32"
        } else {
            "64"
        }
    }
}
