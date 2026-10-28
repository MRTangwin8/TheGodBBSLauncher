//! 服务器列表：读写游戏目录里的 servers.dat（游戏「多人游戏」里那张列表），以及查询服务器状态（延迟、在线人数、MOTD）。
//!
//! servers.dat 是不压缩的 NBT。这里只改 name / ip 两个字段，其余字段（图标、是否接受资源包……）原样保留；
//! 写入前像游戏一样把旧文件备份成 servers.dat_old，再用临时文件替换，写到一半断电也不会丢列表。
//! 状态查询走 Server List Ping（1.7 起的协议），返回的内容来自对方服务器，全部按不可信数据处理。

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{Error, Result};

// ---------------- NBT ----------------

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    /// 元素类型 + 元素；空列表的元素类型可能是 0
    List(u8, Vec<Tag>),
    /// 保持原来的顺序
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn set(&mut self, key: &str, value: Tag) {
        if let Tag::Compound(items) = self {
            match items.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => *v = value,
                None => items.push((key.to_string(), value)),
            }
        }
    }

    fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }
}

const MAX_DEPTH: usize = 64;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or_else(bad_nbt)?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn arr<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("长度已检查"))
    }
    /// 数组长度：不能是负数，也不能比剩下的字节还多（防止伪造的超大长度把内存吃光）
    fn len(&mut self, elem: usize) -> Result<usize> {
        let n = i32::from_be_bytes(self.arr()?);
        let n = usize::try_from(n).map_err(|_| bad_nbt())?;
        if n.saturating_mul(elem.max(1)) > self.data.len() - self.pos {
            return Err(bad_nbt());
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String> {
        let n = u16::from_be_bytes(self.arr()?) as usize;
        Ok(decode_mutf8(self.take(n)?))
    }

    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag> {
        if depth > MAX_DEPTH {
            return Err(bad_nbt());
        }
        Ok(match id {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.arr()?)),
            3 => Tag::Int(i32::from_be_bytes(self.arr()?)),
            4 => Tag::Long(i64::from_be_bytes(self.arr()?)),
            5 => Tag::Float(f32::from_be_bytes(self.arr()?)),
            6 => Tag::Double(f64::from_be_bytes(self.arr()?)),
            7 => {
                let n = self.len(1)?;
                Tag::ByteArray(self.take(n)?.iter().map(|b| *b as i8).collect())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.len(1)?;
                if n > 0 && !(1..=12).contains(&elem) {
                    return Err(bad_nbt());
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.payload(elem, depth + 1)?);
                }
                Tag::List(elem, items)
            }
            10 => {
                let mut items = Vec::new();
                loop {
                    let id = self.u8()?;
                    if id == 0 {
                        break;
                    }
                    let name = self.string()?;
                    items.push((name, self.payload(id, depth + 1)?));
                }
                Tag::Compound(items)
            }
            11 => {
                let n = self.len(4)?;
                Tag::IntArray((0..n).map(|_| self.arr().map(i32::from_be_bytes)).collect::<Result<_>>()?)
            }
            12 => {
                let n = self.len(8)?;
                Tag::LongArray((0..n).map(|_| self.arr().map(i64::from_be_bytes)).collect::<Result<_>>()?)
            }
            _ => return Err(bad_nbt()),
        })
    }
}

fn bad_nbt() -> Error {
    Error::Other("servers.dat 格式不对，可能已经损坏".into())
}

/// 读一个根标签（名字丢掉，servers.dat 的根名字是空的）
pub fn read_nbt(data: &[u8]) -> Result<Tag> {
    let mut r = Reader { data, pos: 0 };
    let id = r.u8()?;
    if id != 10 {
        return Err(bad_nbt());
    }
    r.string()?;
    r.payload(10, 0)
}

pub fn write_nbt(root: &Tag) -> Vec<u8> {
    let mut out = vec![root.id()];
    put_string(&mut out, "");
    put_payload(&mut out, root);
    out
}

fn put_string(out: &mut Vec<u8>, s: &str) {
    let bytes = encode_mutf8(s);
    // 超过 65535 字节的字符串在 NBT 里放不下，截断（不会切坏字符：按字符截）
    let bytes = if bytes.len() > u16::MAX as usize {
        let mut cut = String::new();
        for c in s.chars() {
            if encode_mutf8(&cut).len() + encode_mutf8(&c.to_string()).len() > u16::MAX as usize {
                break;
            }
            cut.push(c);
        }
        encode_mutf8(&cut)
    } else {
        bytes
    };
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(&bytes);
}

fn put_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend(v.iter().map(|b| *b as u8));
        }
        Tag::String(s) => put_string(out, s),
        Tag::List(elem, items) => {
            let elem = items.first().map(Tag::id).unwrap_or(*elem);
            out.push(elem);
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for t in items {
                put_payload(out, t);
            }
        }
        Tag::Compound(items) => {
            for (k, v) in items {
                out.push(v.id());
                put_string(out, k);
                put_payload(out, v);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            v.iter().for_each(|x| out.extend_from_slice(&x.to_be_bytes()));
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            v.iter().for_each(|x| out.extend_from_slice(&x.to_be_bytes()));
        }
    }
}

/// Java 的 modified UTF-8：空字符写成 C0 80，四字节字符（表情等）拆成两个代理项各占三字节
fn encode_mutf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for unit in s.encode_utf16() {
        match unit {
            1..=0x7F => out.push(unit as u8),
            0 | 0x80..=0x7FF => {
                out.push(0xC0 | (unit >> 6) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (unit >> 12) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    out
}

fn decode_mutf8(b: &[u8]) -> String {
    let mut units = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as u16;
        let (unit, used) = if c < 0x80 {
            (c, 1)
        } else if c & 0xE0 == 0xC0 && i + 1 < b.len() {
            (((c & 0x1F) << 6) | (b[i + 1] as u16 & 0x3F), 2)
        } else if c & 0xF0 == 0xE0 && i + 2 < b.len() {
            (((c & 0x0F) << 12) | ((b[i + 1] as u16 & 0x3F) << 6) | (b[i + 2] as u16 & 0x3F), 3)
        } else {
            (0xFFFD, 1)
        };
        units.push(unit);
        i += used;
    }
    String::from_utf16_lossy(&units)
}

// ---------------- servers.dat ----------------

/// 列表里的一个服务器
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub name: String,
    pub ip: String,
    /// 游戏缓存的服务器图标（PNG 的 base64，不带 data: 前缀）
    pub icon: Option<String>,
    /// 游戏里「隐藏」的服务器（目前只有通过外部工具加的才会有）
    pub hidden: bool,
}

const MAX_FILE: u64 = 16 * 1024 * 1024;
const MAX_ICON: usize = 256 * 1024;

fn load_root(path: &Path) -> Result<Tag> {
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE => return Err(Error::Other("servers.dat 太大，不像是正常的服务器列表".into())),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Tag::Compound(vec![("servers".into(), Tag::List(10, vec![]))])),
        Err(e) => return Err(e.into()),
    }
    read_nbt(&std::fs::read(path)?)
}

fn servers_mut(root: &mut Tag) -> Result<&mut Vec<Tag>> {
    if root.get("servers").is_none() {
        root.set("servers", Tag::List(10, vec![]));
    }
    let Tag::Compound(items) = root else { return Err(bad_nbt()) };
    match items.iter_mut().find(|(k, _)| k == "servers") {
        Some((_, Tag::List(_, list))) => Ok(list),
        _ => Err(bad_nbt()),
    }
}

fn is_base64(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_ICON && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

pub fn list(path: &Path) -> Result<Vec<SavedServer>> {
    let mut root = load_root(path)?;
    Ok(servers_mut(&mut root)?
        .iter()
        .map(|s| SavedServer {
            name: s.str("name").unwrap_or("").to_string(),
            ip: s.str("ip").unwrap_or("").to_string(),
            icon: s.str("icon").filter(|i| is_base64(i)).map(str::to_string),
            hidden: matches!(s.get("hidden"), Some(Tag::Byte(b)) if *b != 0),
        })
        .collect())
}

/// 改列表：读出来、交给 f 改、写回去
fn modify(path: &Path, f: impl FnOnce(&mut Vec<Tag>) -> Result<()>) -> Result<()> {
    let mut root = load_root(path)?;
    f(servers_mut(&mut root)?)?;
    let data = write_nbt(&root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.is_file() {
        let _ = std::fs::copy(path, path.with_file_name("servers.dat_old"));
    }
    let tmp = path.with_file_name("servers.dat.godbbs-tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn check_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(Error::Other("服务器名称太长，或者含有控制字符".into()));
    }
    Ok(if name.is_empty() { "Minecraft 服务器".into() } else { name.to_string() })
}

fn index_ok(list: &[Tag], index: usize) -> Result<()> {
    if index >= list.len() {
        return Err(Error::Other("服务器列表已经变了（可能刚在游戏里改过），刷新一下再试".into()));
    }
    Ok(())
}

/// ip 由调用方先用 launch::server::parse_server 校验过
pub fn add(path: &Path, name: &str, ip: &str) -> Result<()> {
    let name = check_name(name)?;
    modify(path, |list| {
        list.push(Tag::Compound(vec![("name".into(), Tag::String(name)), ("ip".into(), Tag::String(ip.to_string()))]));
        Ok(())
    })
}

pub fn update(path: &Path, index: usize, name: &str, ip: &str) -> Result<()> {
    let name = check_name(name)?;
    modify(path, |list| {
        index_ok(list, index)?;
        let s = &mut list[index];
        if s.str("ip") != Some(ip) {
            // 地址换了，旧图标就不是这台服务器的了
            if let Tag::Compound(items) = s {
                items.retain(|(k, _)| k != "icon");
            }
        }
        s.set("name", Tag::String(name));
        s.set("ip", Tag::String(ip.to_string()));
        Ok(())
    })
}

pub fn remove(path: &Path, index: usize) -> Result<()> {
    modify(path, |list| {
        index_ok(list, index)?;
        list.remove(index);
        Ok(())
    })
}

/// 把 from 移到 to（上移 / 下移）
pub fn reorder(path: &Path, from: usize, to: usize) -> Result<()> {
    modify(path, |list| {
        index_ok(list, from)?;
        index_ok(list, to)?;
        let s = list.remove(from);
        list.insert(to, s);
        Ok(())
    })
}

// ---------------- 状态查询 ----------------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    /// 服务器自报的版本名，例如 "Paper 1.21.1"
    pub version: String,
    pub protocol: i64,
    pub online: i64,
    pub max: i64,
    /// 去掉颜色代码后的 MOTD，纯文本
    pub motd: String,
    /// 在线玩家样例（服务器愿意给的话）
    pub sample: Vec<String>,
    /// PNG 的 base64，不带 data: 前缀
    pub favicon: Option<String>,
    pub latency_ms: u64,
}

const TIMEOUT: Duration = Duration::from_secs(6);
const MAX_PACKET: usize = 2 * 1024 * 1024;

/// 查询服务器状态。port 为空时先查 SRV 记录（_minecraft._tcp.主机名），和游戏的做法一样
pub async fn ping(host: &str, port: Option<u16>) -> Result<ServerStatus> {
    let (connect_host, connect_port) = match port {
        Some(p) => (host.to_string(), p),
        None => srv_lookup(host).await.unwrap_or_else(|| (host.to_string(), 25565)),
    };
    tokio::time::timeout(TIMEOUT, ping_at(host, &connect_host, connect_port))
        .await
        .map_err(|_| Error::Other("连接超时：服务器没有响应".into()))?
}

async fn srv_lookup(host: &str) -> Option<(String, u16)> {
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let resolver = hickory_resolver::TokioAsyncResolver::tokio_from_system_conf().ok()?;
    let found = tokio::time::timeout(Duration::from_secs(3), resolver.srv_lookup(format!("_minecraft._tcp.{host}."))).await.ok()?.ok()?;
    let best = found.iter().min_by_key(|r| (r.priority(), std::cmp::Reverse(r.weight())))?;
    let target = best.target().to_utf8();
    let target = target.trim_end_matches('.');
    (!target.is_empty()).then(|| (target.to_string(), best.port()))
}

async fn ping_at(host: &str, connect_host: &str, port: u16) -> Result<ServerStatus> {
    let mut stream = TcpStream::connect((connect_host, port)).await.map_err(|e| Error::Other(format!("连不上服务器：{e}")))?;
    let _ = stream.set_nodelay(true);

    // 握手：协议号按约定填 -1（只查状态），下一步状态 1
    let mut hs = Vec::new();
    put_varint(&mut hs, 0);
    put_varint(&mut hs, -1);
    put_varint(&mut hs, host.len() as i32);
    hs.extend_from_slice(host.as_bytes());
    hs.extend_from_slice(&port.to_be_bytes());
    put_varint(&mut hs, 1);
    let mut out = Vec::new();
    put_varint(&mut out, hs.len() as i32);
    out.extend_from_slice(&hs);
    out.extend_from_slice(&[1, 0]); // 状态请求：长度 1，包 id 0
    let start = Instant::now();
    stream.write_all(&out).await?;

    let body = read_packet(&mut stream).await?;
    let status_rtt = start.elapsed();
    let mut r = Reader { data: &body, pos: 0 };
    if read_varint_buf(&mut r)? != 0 {
        return Err(bad_reply());
    }
    let len = usize::try_from(read_varint_buf(&mut r)?).map_err(|_| bad_reply())?;
    let json = String::from_utf8_lossy(r.take(len).map_err(|_| bad_reply())?).into_owned();
    let mut status = parse_status(&json)?;

    // Ping 包测延迟；有的服务器不回，就用状态请求的往返时间
    let t0 = Instant::now();
    let mut ping = vec![9, 1];
    ping.extend_from_slice(&0x6764_6262_7300_0000_i64.to_be_bytes());
    let pong = async {
        stream.write_all(&ping).await?;
        read_packet(&mut stream).await
    };
    status.latency_ms = match tokio::time::timeout(Duration::from_secs(2), pong).await {
        Ok(Ok(_)) => t0.elapsed().as_millis() as u64,
        _ => status_rtt.as_millis() as u64,
    };
    Ok(status)
}

fn bad_reply() -> Error {
    Error::Other("服务器的回应看不懂（可能是 1.7 以前的老服务器，或者不是 Minecraft 服务器）".into())
}

fn put_varint(out: &mut Vec<u8>, v: i32) {
    let mut v = v as u32;
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v & 0x7F | 0x80) as u8);
        v >>= 7;
    }
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32> {
    let mut v: u32 = 0;
    for i in 0..5 {
        let b = stream.read_u8().await.map_err(|_| bad_reply())?;
        v |= ((b & 0x7F) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(v as i32);
        }
    }
    Err(bad_reply())
}

fn read_varint_buf(r: &mut Reader) -> Result<i32> {
    let mut v: u32 = 0;
    for i in 0..5 {
        let b = r.u8().map_err(|_| bad_reply())?;
        v |= ((b & 0x7F) as u32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(v as i32);
        }
    }
    Err(bad_reply())
}

async fn read_packet(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let len = usize::try_from(read_varint(stream).await?).map_err(|_| bad_reply())?;
    if len == 0 || len > MAX_PACKET {
        return Err(bad_reply());
    }
    let mut body = vec![0; len];
    stream.read_exact(&mut body).await.map_err(|_| bad_reply())?;
    Ok(body)
}

fn parse_status(json: &str) -> Result<ServerStatus> {
    let v: Value = serde_json::from_str(json).map_err(|_| bad_reply())?;
    let players = &v["players"];
    let sample = players["sample"]
        .as_array()
        .map(|a| a.iter().filter_map(|p| p["name"].as_str()).map(|n| clean(n, 32)).filter(|n| !n.is_empty()).take(12).collect())
        .unwrap_or_default();
    let favicon = v["favicon"].as_str().and_then(|f| f.strip_prefix("data:image/png;base64,")).map(|f| f.replace(['\n', '\r'], "")).filter(|f| is_base64(f));
    Ok(ServerStatus {
        version: clean(v["version"]["name"].as_str().unwrap_or(""), 64),
        protocol: v["version"]["protocol"].as_i64().unwrap_or(0),
        online: players["online"].as_i64().unwrap_or(0).max(0),
        max: players["max"].as_i64().unwrap_or(0).max(0),
        motd: clean(&chat_text(&v["description"]), 300),
        sample,
        favicon,
        latency_ms: 0,
    })
}

/// 聊天组件转纯文本：字符串、{"text": …, "extra": […]}、数组都可能出现
fn chat_text(v: &Value) -> String {
    fn walk(v: &Value, out: &mut String, depth: usize) {
        if depth > 16 || out.len() > 4096 {
            return;
        }
        match v {
            Value::String(s) => out.push_str(s),
            Value::Array(items) => items.iter().for_each(|i| walk(i, out, depth + 1)),
            Value::Object(o) => {
                if let Some(t) = o.get("text") {
                    walk(t, out, depth + 1);
                } else if let Some(t) = o.get("translate").and_then(Value::as_str) {
                    out.push_str(t);
                }
                if let Some(extra) = o.get("extra") {
                    walk(extra, out, depth + 1);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    walk(v, &mut out, 0);
    out
}

/// 去掉 § 颜色代码和控制字符（保留换行），每行去掉首尾空白，限制长度
fn clean(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else if c == '\n' || !c.is_control() {
            out.push(c);
        }
    }
    let text = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    text.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_utf8_roundtrip() {
        for s in ["hello", "神权论坛 🎮", "a\0b", ""] {
            assert_eq!(decode_mutf8(&encode_mutf8(s)), s);
        }
        assert_eq!(encode_mutf8("\0"), [0xC0, 0x80], "空字符要写成两个字节");
        assert_eq!(encode_mutf8("🎮").len(), 6, "四字节字符拆成两个三字节代理项");
    }

    #[test]
    fn edits_keep_unknown_fields_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("servers.dat");
        assert!(list(&path).unwrap().is_empty(), "没有文件时是空列表");

        // 模拟游戏写的文件：带图标和 acceptTextures
        let root = Tag::Compound(vec![(
            "servers".into(),
            Tag::List(10, vec![Tag::Compound(vec![
                ("ip".into(), Tag::String("mc.example.com".into())),
                ("name".into(), Tag::String("老服务器".into())),
                ("icon".into(), Tag::String("iVBORw0KGgo=".into())),
                ("acceptTextures".into(), Tag::Byte(1)),
            ])]),
        )]);
        std::fs::write(&path, write_nbt(&root)).unwrap();
        assert_eq!(list(&path).unwrap()[0], SavedServer { name: "老服务器".into(), ip: "mc.example.com".into(), icon: Some("iVBORw0KGgo=".into()), hidden: false });

        add(&path, " 新服 🎮 ", "play.example.net:25570").unwrap();
        update(&path, 0, "改名了", "mc.example.com").unwrap();
        let servers = list(&path).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!((servers[0].name.as_str(), servers[0].icon.is_some()), ("改名了", true), "只改名字时图标保留");
        assert_eq!(servers[1].name, "新服 🎮");
        let raw = read_nbt(&std::fs::read(&path).unwrap()).unwrap();
        let first = match raw.get("servers") {
            Some(Tag::List(_, l)) => l[0].clone(),
            _ => panic!(),
        };
        assert_eq!(first.get("acceptTextures"), Some(&Tag::Byte(1)), "不认识的字段原样保留");
        assert!(dir.path().join("servers.dat_old").is_file(), "写之前备份旧文件");

        update(&path, 0, "改名了", "other.example.com").unwrap();
        assert!(list(&path).unwrap()[0].icon.is_none(), "换地址后旧图标作废");
        reorder(&path, 1, 0).unwrap();
        assert_eq!(list(&path).unwrap()[0].name, "新服 🎮");
        remove(&path, 0).unwrap();
        assert_eq!(list(&path).unwrap().len(), 1);
        assert!(remove(&path, 5).is_err());
        assert!(add(&path, "坏\u{7}名字", "a.com").is_err());
    }

    #[test]
    fn rejects_corrupt_files() {
        assert!(read_nbt(b"").is_err());
        assert!(read_nbt(&[10, 0, 0, 7, 0, 1, b'a', 0x7F, 0xFF, 0xFF, 0xFF]).is_err(), "伪造的超大数组长度");
        assert!(read_nbt(&[8, 0, 0]).is_err(), "根必须是复合标签");
        // 嵌套太深
        let mut deep = vec![10, 0, 0];
        for _ in 0..100 {
            deep.extend_from_slice(&[10, 0, 1, b'x']);
        }
        assert!(read_nbt(&deep).is_err());
    }

    #[test]
    fn parses_status_json_defensively() {
        let s = parse_status(r#"{
            "version": {"name": "Paper 1.21.1", "protocol": 767},
            "players": {"max": 100, "online": 7, "sample": [{"name": "Steve", "id": "x"}, {"name": "§cAlex"}]},
            "description": {"text": "§a欢迎来到", "extra": [{"text": " 神权服务器\n  第二行  "}]},
            "favicon": "data:image/png;base64,iVBOR\nw0KGgo="
        }"#).unwrap();
        assert_eq!((s.version.as_str(), s.protocol, s.online, s.max), ("Paper 1.21.1", 767, 7, 100));
        assert_eq!(s.motd, "欢迎来到 神权服务器\n第二行");
        assert_eq!(s.sample, ["Steve", "Alex"]);
        assert_eq!(s.favicon.as_deref(), Some("iVBORw0KGgo="));

        let plain = parse_status(r#"{"description": "§l老式 MOTD", "favicon": "javascript:alert(1)", "players": {"online": -5}}"#).unwrap();
        assert_eq!(plain.motd, "老式 MOTD");
        assert!(plain.favicon.is_none(), "只接受 PNG data URL");
        assert_eq!(plain.online, 0);
        assert!(parse_status("不是 JSON").is_err());
    }

    /// 本地起一个假服务器，按协议走一遍握手、状态、Ping
    #[tokio::test]
    async fn pings_a_local_fake_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let hs = read_packet(&mut s).await.unwrap();
            assert_eq!(hs[0], 0, "握手包 id");
            assert_eq!(*hs.last().unwrap(), 1, "下一步是状态");
            assert_eq!(read_packet(&mut s).await.unwrap(), [0], "状态请求");
            let json = r#"{"version":{"name":"测试服","protocol":767},"players":{"max":20,"online":3},"description":"hi"}"#;
            let mut body = vec![0];
            put_varint(&mut body, json.len() as i32);
            body.extend_from_slice(json.as_bytes());
            let mut out = Vec::new();
            put_varint(&mut out, body.len() as i32);
            out.extend_from_slice(&body);
            s.write_all(&out).await.unwrap();
            let ping = read_packet(&mut s).await.unwrap();
            assert_eq!(ping.len(), 9);
            let mut pong = vec![9];
            pong.extend_from_slice(&ping);
            s.write_all(&pong).await.unwrap();
        });
        let st = ping("127.0.0.1", Some(port)).await.unwrap();
        assert_eq!((st.version.as_str(), st.online, st.max, st.motd.as_str()), ("测试服", 3, 20, "hi"));
    }

    #[tokio::test]
    async fn closed_port_fails_fast_with_a_message() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let e = ping("127.0.0.1", Some(port)).await.unwrap_err().to_string();
        assert!(e.contains("连不上"), "{e}");
    }
}
