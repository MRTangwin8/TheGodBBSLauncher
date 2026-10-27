//! 存档管理：列出 saves/ 里的世界，备份成 zip，从备份恢复。
//!
//! - 备份放在游戏目录的 `godbbs-backups/` 里，文件名是 `<世界文件夹>_<unix 秒>.zip`，zip 里第一层就是世界文件夹。
//! - 恢复时世界已经存在：先把现在的样子自动备份一份，再替换；解压先放到临时文件夹，成功了才换上去。
//! - 解压只接受落在世界文件夹里面的条目，备份文件当作不可信数据处理（可能是别人发来的）。

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::engine::{Error, Result};

pub const BACKUP_DIR: &str = "godbbs-backups";
/// 游戏运行时会锁住这个文件，备份里也不需要它
const SKIP: &[&str] = &["session.lock"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// saves 下的文件夹名
    pub folder: String,
    pub size: u64,
    /// 最后修改时间（unix 秒），取 level.dat 的
    pub modified: Option<u64>,
    /// 有没有 icon.png
    pub has_icon: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub file: String,
    pub world: String,
    pub size: u64,
    /// 备份时间（unix 秒）
    pub created: u64,
}

fn io_err(what: &str, path: &Path, e: io::Error) -> Error {
    Error::Other(format!("{what}：{}（{e}）", path.display()))
}

/// 名字只能是单个文件夹名 / 文件名，防止 ../ 越出目录
fn plain_name(name: &str) -> Result<&str> {
    let ok = !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\']) && !name.starts_with('.');
    if ok {
        Ok(name)
    } else {
        Err(Error::Other(format!("名字不合法：{name}")))
    }
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// 列出存档（有 level.dat 的文件夹），最近玩过的在前
pub fn list(game_dir: &Path) -> Vec<World> {
    let Ok(entries) = fs::read_dir(game_dir.join("saves")) else { return vec![] };
    let mut out: Vec<World> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let path = e.path();
            let level = fs::metadata(path.join("level.dat")).ok()?;
            Some(World {
                folder: e.file_name().to_string_lossy().to_string(),
                size: dir_size(&path),
                modified: level.modified().ok().map(unix),
                has_icon: path.join("icon.png").is_file(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.folder.cmp(&b.folder)));
    out
}

/// `<世界>_<unix 秒>.zip` → (世界, 时间)
fn parse_backup_name(file: &str) -> Option<(String, u64)> {
    let stem = file.strip_suffix(".zip")?;
    let (world, ts) = stem.rsplit_once('_')?;
    (!world.is_empty()).then_some(())?;
    Some((world.to_string(), ts.parse().ok()?))
}

/// 列出备份，新的在前
pub fn backups(game_dir: &Path) -> Vec<Backup> {
    let Ok(entries) = fs::read_dir(game_dir.join(BACKUP_DIR)) else { return vec![] };
    let mut out: Vec<Backup> = entries
        .flatten()
        .filter_map(|e| {
            let file = e.file_name().to_string_lossy().to_string();
            let (world, created) = parse_backup_name(&file)?;
            let size = e.metadata().ok().filter(|m| m.is_file())?.len();
            Some(Backup { file, world, size, created })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.file.cmp(&b.file)));
    out
}

/// 把一个世界打包成 zip，返回备份信息
pub fn backup(game_dir: &Path, world: &str) -> Result<Backup> {
    let world = plain_name(world)?;
    let src = game_dir.join("saves").join(world);
    if !src.join("level.dat").is_file() {
        return Err(Error::Other(format!("找不到这个存档：{world}")));
    }
    let dir = game_dir.join(BACKUP_DIR);
    fs::create_dir_all(&dir).map_err(|e| io_err("没法创建备份文件夹", &dir, e))?;
    // 同一秒备份两次时往后挪一秒，不覆盖已有的备份
    let mut created = unix(SystemTime::now());
    let mut out = dir.join(format!("{world}_{created}.zip"));
    while out.exists() {
        created += 1;
        out = dir.join(format!("{world}_{created}.zip"));
    }
    let tmp = out.with_extension("zip.part");
    let result = write_zip(&src, world, &tmp).and_then(|_| fs::rename(&tmp, &out).map_err(|e| io_err("没法保存备份", &out, e)));
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let size = fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    Ok(Backup { file: out.file_name().unwrap().to_string_lossy().to_string(), world: world.to_string(), size, created })
}

fn write_zip(src: &Path, world: &str, out: &Path) -> Result<()> {
    let zerr = |e: zip::result::ZipError| Error::Other(format!("打包失败：{e}"));
    let file = fs::File::create(out).map_err(|e| io_err("没法写备份文件", out, e))?;
    let mut zip = zip::ZipWriter::new(io::BufWriter::new(file));
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
    let mut stack = vec![(src.to_path_buf(), world.to_string())];
    while let Some((dir, prefix)) = stack.pop() {
        zip.add_directory(format!("{prefix}/"), opt).map_err(zerr)?;
        for e in fs::read_dir(&dir).map_err(|e| io_err("读不了存档文件夹", &dir, e))? {
            let e = e.map_err(|e| io_err("读不了存档文件夹", &dir, e))?;
            let name = e.file_name().to_string_lossy().to_string();
            let t = e.file_type().map_err(|err| io_err("读不了文件信息", &e.path(), err))?;
            if t.is_dir() {
                stack.push((e.path(), format!("{prefix}/{name}")));
            } else if t.is_file() && !SKIP.contains(&name.as_str()) {
                zip.start_file(format!("{prefix}/{name}"), opt).map_err(zerr)?;
                let mut f = fs::File::open(e.path()).map_err(|err| io_err("读不了存档文件（游戏是不是正开着这个世界？）", &e.path(), err))?;
                io::copy(&mut f, &mut zip).map_err(|err| io_err("打包失败", &e.path(), err))?;
            }
        }
    }
    zip.finish().map_err(zerr)?;
    Ok(())
}

/// 从备份恢复。世界已存在时先自动备份现在的样子，返回那份自动备份（没有就是 None）
pub fn restore(game_dir: &Path, file: &str) -> Result<Option<Backup>> {
    let file = plain_name(file)?;
    let (world, _) = parse_backup_name(file).ok_or_else(|| Error::Other(format!("不是启动器做的备份：{file}")))?;
    plain_name(&world)?;
    let zip_path = game_dir.join(BACKUP_DIR).join(file);
    let saves = game_dir.join("saves");
    fs::create_dir_all(&saves).map_err(|e| io_err("没法创建 saves 文件夹", &saves, e))?;

    // 先解压到临时文件夹
    let staging = saves.join(format!(".{world}.restoring"));
    let _ = fs::remove_dir_all(&staging);
    if let Err(e) = extract(&zip_path, &world, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    let target = saves.join(&world);
    let auto = if target.exists() {
        let b = match backup(game_dir, &world) {
            Ok(b) => b,
            Err(e) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(Error::Other(format!("恢复前没能先备份现在的存档，所以没有动它：{e}")));
            }
        };
        // 旧的先挪开，换上新的以后再删；中途失败就挪回来
        let old = saves.join(format!(".{world}.old"));
        let _ = fs::remove_dir_all(&old);
        fs::rename(&target, &old).map_err(|e| {
            let _ = fs::remove_dir_all(&staging);
            io_err("没法替换现在的存档（游戏是不是正开着这个世界？）", &target, e)
        })?;
        if let Err(e) = fs::rename(&staging, &target) {
            let _ = fs::rename(&old, &target);
            let _ = fs::remove_dir_all(&staging);
            return Err(io_err("恢复失败", &target, e));
        }
        let _ = fs::remove_dir_all(&old);
        Some(b)
    } else {
        fs::rename(&staging, &target).map_err(|e| {
            let _ = fs::remove_dir_all(&staging);
            io_err("恢复失败", &target, e)
        })?;
        None
    };
    Ok(auto)
}

/// 解压：只要 `<world>/` 下面的条目，路径里不能有 .. 或绝对路径
fn extract(zip_path: &Path, world: &str, dest: &Path) -> Result<()> {
    let zerr = |e: zip::result::ZipError| Error::Other(format!("备份文件损坏：{e}"));
    let file = fs::File::open(zip_path).map_err(|e| io_err("打不开备份", zip_path, e))?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file)).map_err(zerr)?;
    fs::create_dir_all(dest).map_err(|e| io_err("没法创建临时文件夹", dest, e))?;
    let mut found_level = false;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(zerr)?;
        let Some(path) = entry.enclosed_name() else {
            return Err(Error::Other(format!("备份里有不安全的路径：{}", entry.name())));
        };
        let mut parts = path.components();
        if parts.next() != Some(Component::Normal(world.as_ref())) {
            return Err(Error::Other(format!("备份里有不属于「{world}」的文件：{}", entry.name())));
        }
        let rel: PathBuf = parts.collect();
        if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(Error::Other(format!("备份里有不安全的路径：{}", entry.name())));
        }
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| io_err("解压失败", &out, e))?;
        } else {
            if let Some(p) = out.parent() {
                fs::create_dir_all(p).map_err(|e| io_err("解压失败", p, e))?;
            }
            let mut f = fs::File::create(&out).map_err(|e| io_err("解压失败", &out, e))?;
            io::copy(&mut entry, &mut f).map_err(|e| io_err("解压失败", &out, e))?;
            found_level |= rel == Path::new("level.dat");
        }
    }
    if !found_level {
        return Err(Error::Other("备份里没有 level.dat，不是一个完整的存档".into()));
    }
    Ok(())
}

pub fn delete_backup(game_dir: &Path, file: &str) -> Result<()> {
    let file = plain_name(file)?;
    parse_backup_name(file).ok_or_else(|| Error::Other(format!("不是启动器做的备份：{file}")))?;
    let p = game_dir.join(BACKUP_DIR).join(file);
    fs::remove_file(&p).map_err(|e| io_err("删除备份失败", &p, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(game: &Path, name: &str, marker: &str) {
        let w = game.join("saves").join(name);
        fs::create_dir_all(w.join("region")).unwrap();
        fs::write(w.join("level.dat"), marker).unwrap();
        fs::write(w.join("region/r.0.0.mca"), vec![7u8; 5000]).unwrap();
        fs::write(w.join("session.lock"), b"x").unwrap();
    }

    #[test]
    fn lists_only_real_worlds() {
        let t = tempfile::tempdir().unwrap();
        world(t.path(), "我的世界_1", "a");
        fs::create_dir_all(t.path().join("saves/空文件夹")).unwrap();
        let worlds = list(t.path());
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].folder, "我的世界_1");
        assert_eq!(worlds[0].size, 1 + 5000 + 1);
        assert!(list(&t.path().join("nope")).is_empty());
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let g = t.path();
        world(g, "生存_1", "v1");
        let b1 = backup(g, "生存_1").unwrap();
        assert_eq!(b1.world, "生存_1", "世界名里有下划线也能认出来");
        let b2 = backup(g, "生存_1").unwrap();
        assert_ne!(b1.file, b2.file, "同一秒两次备份不能互相覆盖");
        assert_eq!(backups(g).len(), 2);

        // 改坏了再恢复：会先自动备份现在的样子
        fs::write(g.join("saves/生存_1/level.dat"), "v2").unwrap();
        let auto = restore(g, &b1.file).unwrap().expect("世界已存在时要自动备份");
        assert_eq!(fs::read_to_string(g.join("saves/生存_1/level.dat")).unwrap(), "v1");
        assert_eq!(fs::read(g.join("saves/生存_1/region/r.0.0.mca")).unwrap().len(), 5000);
        assert!(!g.join("saves/生存_1/session.lock").exists(), "session.lock 不进备份");
        assert_eq!(backups(g).len(), 3);
        // 自动备份里是 v2
        fs::remove_dir_all(g.join("saves/生存_1")).unwrap();
        assert!(restore(g, &auto.file).unwrap().is_none(), "世界不存在时直接恢复");
        assert_eq!(fs::read_to_string(g.join("saves/生存_1/level.dat")).unwrap(), "v2");
        // 没留下临时文件夹
        let names: Vec<_> = fs::read_dir(g.join("saves")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names.len(), 1);

        delete_backup(g, &auto.file).unwrap();
        assert_eq!(backups(g).len(), 2);
    }

    fn evil_zip(g: &Path, file: &str, entries: &[(&str, &[u8])]) {
        let dir = g.join(BACKUP_DIR);
        fs::create_dir_all(&dir).unwrap();
        let mut w = zip::ZipWriter::new(fs::File::create(dir.join(file)).unwrap());
        for (n, data) in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            io::Write::write_all(&mut w, data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn refuses_unsafe_backups_and_keeps_the_world() {
        let t = tempfile::tempdir().unwrap();
        let g = t.path();
        world(g, "w", "keep");
        evil_zip(g, "w_1.zip", &[("w/level.dat", b"x"), ("w/../../escape.txt", b"x")]);
        evil_zip(g, "w_2.zip", &[("w/level.dat", b"x"), ("other/level.dat", b"x")]);
        evil_zip(g, "w_3.zip", &[("w/region/r.0.0.mca", b"x")]);
        for f in ["w_1.zip", "w_2.zip", "w_3.zip"] {
            assert!(restore(g, f).is_err(), "{f}");
            assert_eq!(fs::read_to_string(g.join("saves/w/level.dat")).unwrap(), "keep", "{f}");
        }
        assert!(!g.join("escape.txt").exists() && !g.parent().unwrap().join("escape.txt").exists());
        for bad in ["../x_1.zip", "x/y_1.zip", "noext", ".hidden_1.zip"] {
            assert!(restore(g, bad).is_err(), "{bad}");
            assert!(delete_backup(g, bad).is_err(), "{bad}");
        }
        assert!(backup(g, "..").is_err() && backup(g, "不存在").is_err());
    }
}
