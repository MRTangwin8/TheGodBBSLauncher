use std::io::{Cursor, Write};
use std::sync::Arc;

use super::export::{candidates, export, ExportEnv, ExportOptions};
use super::*;

fn make_zip(dir: &Path, name: &str, files: Vec<(&str, &[u8])>) -> PathBuf {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, data) in files {
        w.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    let path = dir.join(name);
    std::fs::write(&path, w.finish().unwrap().into_inner()).unwrap();
    path
}

const MRPACK_INDEX: &str = r#"{
    "formatVersion": 1, "game": "minecraft", "versionId": "1.2.0", "name": "神权测试包", "summary": "小包",
    "files": [
        {"path": "mods/sodium.jar", "hashes": {"sha1": "aaa", "sha512": "bbb"}, "env": {"client": "required", "server": "unsupported"},
         "downloads": ["https://cdn.modrinth.com/data/AANobbMI/sodium.jar"], "fileSize": 100},
        {"path": "mods/server-only.jar", "hashes": {"sha1": "ccc"}, "env": {"client": "unsupported", "server": "required"},
         "downloads": ["https://cdn.modrinth.com/x.jar"], "fileSize": 1},
        {"path": "../../evil.jar", "hashes": {"sha1": "ddd"}, "downloads": ["https://cdn.modrinth.com/evil.jar"], "fileSize": 1}
    ],
    "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.9"}
}"#;

#[tokio::test]
async fn reads_mrpack_and_extracts_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let pack = make_zip(dir.path(), "a.mrpack", vec![
        ("modrinth.index.json", MRPACK_INDEX.as_bytes()),
        ("overrides/config/sodium.json", b"{}"),
        ("overrides/options.txt", b"lang:en_us"),
        ("client-overrides/options.txt", b"lang:zh_cn"),
        ("overrides/../../escape.txt", b"nope"),
    ]);
    let p = Modpack::open(&pack).unwrap();
    assert_eq!(p.info.format, PackFormat::Modrinth);
    assert_eq!(p.info.name, "神权测试包");
    assert_eq!(p.info.mc, "1.21.1");
    assert_eq!(p.info.loader, Some(LoaderKind::Fabric));
    assert_eq!(p.info.loader_version.as_deref(), Some("0.16.9"));
    assert_eq!(p.info.file_count, 2, "仅服务端的文件不算");

    let api = ResourceApi { client: reqwest::Client::new(), curseforge_key: None };
    let (files, warnings) = p.files(&api).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "mods/sodium.jar");
    assert_eq!(files[0].sha1.as_deref(), Some("aaa"));
    assert!(warnings[0].contains("evil.jar"), "跳出游戏目录的路径要拒绝");

    let game = dir.path().join("game");
    let n = p.extract_overrides(&game).unwrap();
    assert_eq!(n, 3, "不安全的条目被跳过");
    assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "lang:zh_cn", "client-overrides 覆盖 overrides");
    assert!(game.join("config/sodium.json").is_file());
    assert!(!dir.path().join("escape.txt").exists());
}

#[test]
fn reads_curseforge_manifest_in_subfolder() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = r#"{"minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}]},
        "manifestType": "minecraftModpack", "manifestVersion": 1, "name": "CF Pack", "version": "3.0", "author": "Steve",
        "files": [{"projectID": 238222, "fileID": 4712866, "required": true}, {"projectID": 1, "fileID": 2, "required": false}],
        "overrides": "overrides"}"#;
    let pack = make_zip(dir.path(), "cf.zip", vec![("My Pack/manifest.json", manifest.as_bytes()), ("My Pack/overrides/config/a.toml", b"x")]);
    let p = Modpack::open(&pack).unwrap();
    assert_eq!(p.info.format, PackFormat::CurseForge);
    assert_eq!(p.info.loader, Some(LoaderKind::Forge));
    assert_eq!(p.info.loader_version.as_deref(), Some("47.2.0"));
    assert_eq!(p.info.file_count, 1, "required=false 的不装");
    assert_eq!(p.info.override_count, 1);
    let game = dir.path().join("g");
    assert_eq!(p.extract_overrides(&game).unwrap(), 1);
    assert!(game.join("config/a.toml").is_file(), "外层文件夹前缀要去掉");
}

#[test]
fn reads_mcbbs_packmeta_with_launch_info() {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"manifestType": "minecraftModpack", "manifestVersion": 2, "name": "论坛包", "version": "1.0", "author": "坛友",
        "description": "MCBBS 格式", "files": [{"path": "mods/a.jar", "hash": "abc", "force": true, "type": "addon"}],
        "addons": [{"id": "game", "version": "1.12.2"}, {"id": "forge", "version": "14.23.5.2859"}, {"id": "optifine", "version": "HD_U_G5"}],
        "launchInfo": {"minMemory": 4096, "launchArgument": [], "javaArgument": ["-XX:+UseG1GC"]}}"#;
    let pack = make_zip(dir.path(), "m.zip", vec![("mcbbs.packmeta", meta.as_bytes()), ("manifest.json", br#"{"manifestType":"minecraftModpack","minecraft":{"version":"1.12.2"}}"#), ("overrides/mods/a.jar", b"jar")]);
    let p = Modpack::open(&pack).unwrap();
    assert_eq!(p.info.format, PackFormat::Mcbbs, "同时有 manifest.json 时优先认 mcbbs.packmeta");
    assert_eq!(p.info.mc, "1.12.2");
    assert_eq!(p.info.loader, Some(LoaderKind::Forge));
    assert_eq!(p.info.min_memory_mb, Some(4096));
    assert_eq!(p.info.jvm_args, ["-XX:+UseG1GC"]);
    assert_eq!(p.info.file_count, 0, "addon 文件已经在 overrides 里");
    assert!(p.info.warnings[0].contains("OptiFine"));
}

#[test]
fn rejects_non_packs() {
    let dir = tempfile::tempdir().unwrap();
    let z = make_zip(dir.path(), "x.zip", vec![("readme.txt", b"hi")]);
    assert!(Modpack::open(&z).is_err());
    let bad = make_zip(dir.path(), "y.zip", vec![("manifest.json", br#"{"name":"not a pack"}"#)]);
    assert!(Modpack::open(&bad).is_err());
}

#[test]
fn safe_paths() {
    assert_eq!(safe_rel("mods/a.jar").as_deref(), Some("mods/a.jar"));
    assert_eq!(safe_rel("mods\\a.jar").as_deref(), Some("mods/a.jar"));
    assert_eq!(safe_rel("./config//x").as_deref(), Some("config/x"));
    assert!(safe_rel("../a").is_none());
    assert!(safe_rel("mods/../../a").is_none());
    assert!(safe_rel("/etc/passwd").is_none());
    assert!(safe_rel("C:/Windows/x").is_none());
    assert!(safe_rel("").is_none());
}

#[tokio::test]
async fn mcbbs_export_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("versions/pack");
    for (rel, data) in [("mods/a.jar", "jar"), ("config/a.toml", "x=1"), ("logs/latest.log", "log"), ("pack.json", "{}"), ("saves/w/level.dat", "s")] {
        let p = game.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }
    let c = candidates(&game, "pack");
    let names: Vec<&str> = c.iter().map(|e| e.name.as_str()).collect();
    assert!(!names.contains(&"logs") && !names.contains(&"pack.json"), "日志和版本自身文件不导出：{names:?}");
    assert!(c.iter().find(|e| e.name == "saves").is_some_and(|e| !e.default_on), "存档可选但默认不勾");

    let opts = ExportOptions { format: PackFormat::Mcbbs, name: "导出包".into(), version: "2.0".into(), author: "我".into(), summary: String::new(), include: vec!["mods".into(), "config".into(), "../x".into()] };
    let env = ExportEnv { mc: "1.20.1".into(), loader: Some(LoaderKind::Forge), loader_version: Some("47.4.28".into()), min_memory_mb: Some(4096), jvm_args: vec![], game_args: vec![] };
    let api = ResourceApi { client: reqwest::Client::new(), curseforge_key: None };
    let out = dir.path().join("out.zip");
    let s = export(&api, &game, &opts, &env, &out, Arc::new(|_| {})).await.unwrap();
    assert_eq!((s.linked, s.bundled), (0, 2));

    let p = Modpack::open(&out).unwrap();
    assert_eq!(p.info.format, PackFormat::Mcbbs);
    assert_eq!(p.info.name, "导出包");
    assert_eq!(p.info.loader_version.as_deref(), Some("47.4.28"));
    assert_eq!(p.info.min_memory_mb, Some(4096));
    let target = dir.path().join("imported");
    assert_eq!(p.extract_overrides(&target).unwrap(), 2);
    assert_eq!(std::fs::read_to_string(target.join("config/a.toml")).unwrap(), "x=1");
}

fn cf_version(filename: &str) -> ResourceVersion {
    serde_json::from_value(serde_json::json!({
        "source": "curseforge", "id": "20", "projectId": "10",
        "name": "Test", "versionNumber": "1", "gameVersions": [], "loaders": [],
        "published": "", "releaseType": "release", "dependencies": [],
        "file": { "url": "https://edge.forgecdn.net/files/1/2/mod.jar", "filename": filename, "sha1": null, "size": 5 }
    })).unwrap()
}

#[test]
fn required_curseforge_files_cannot_be_silently_skipped() {
    let classes = HashMap::from([("10".into(), 6)]);
    let refs = [(10, 20)];
    assert!(curseforge_pack_files(&[], &refs, &classes).is_err());
    let mut version = cf_version("mod.jar");
    version.file = None;
    assert!(curseforge_pack_files(&[version], &refs, &classes).is_err());
    let mut version = cf_version("mod.jar");
    version.project_id = "999".into();
    assert!(curseforge_pack_files(&[version], &refs, &classes).is_err());
}

#[test]
fn curseforge_pack_files_have_safe_paths_and_known_resource_types() {
    let refs = [(10, 20)];
    for (class, folder) in [(6, "mods"), (12, "resourcepacks"), (6552, "shaderpacks")] {
        let classes = HashMap::from([("10".into(), class)]);
        let files = curseforge_pack_files(&[cf_version("test.zip")], &refs, &classes).unwrap();
        assert_eq!(files[0].path, format!("{folder}/test.zip"));
    }
    let classes = HashMap::from([("10".into(), 6)]);
    for filename in ["../outside.jar", r"..\outside.jar", "C:outside.jar", "/outside.jar"] {
        assert!(curseforge_pack_files(&[cf_version(filename)], &refs, &classes).is_err(), "{filename}");
    }
    assert!(curseforge_pack_files(&[cf_version("mod.jar")], &refs, &HashMap::new()).is_err());
}
