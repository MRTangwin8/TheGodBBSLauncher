//! 版本元数据：版本清单、version.json、资源索引、运行平台规则。

pub mod manifest;
pub mod platform;
pub mod version;

pub use manifest::{ManifestEntry, VersionManifest, MANIFEST_URL};
pub use platform::Platform;
pub use version::{AssetIndex, Library, VersionJson};
