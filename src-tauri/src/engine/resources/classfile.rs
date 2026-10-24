//! 极简 Java class 文件解析：只取类名、方法（名字 + 描述符）和注解，够检查 Mixin 注入点用。
//! 规范：https://docs.oracle.com/javase/specs/jvms/se21/html/jvms-4.html

use crate::engine::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum ElementValue {
    Int(i64),
    Str(String),
    /// 类字面量，值是描述符，如 `Lnet/minecraft/client/Minecraft;`
    Class(String),
    Enum { type_desc: String, name: String },
    Annotation(Annotation),
    Array(Vec<ElementValue>),
    /// 浮点等用不到的常量
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// 注解类型描述符，如 `Lorg/spongepowered/asm/mixin/Mixin;`
    pub desc: String,
    pub values: Vec<(String, ElementValue)>,
}

impl Annotation {
    pub fn get(&self, key: &str) -> Option<&ElementValue> {
        self.values.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub desc: String,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// 内部名，如 `net/minecraft/client/Minecraft`
    pub name: String,
    pub methods: Vec<MethodInfo>,
    pub annotations: Vec<Annotation>,
}

impl ClassInfo {
    pub fn annotation(&self, desc: &str) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.desc == desc)
    }
}

#[derive(Clone)]
enum Constant {
    Utf8(String),
    Int(i64),
    /// Class 常量，指向名字的下标
    Class(u16),
    Other,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or_else(|| Error::Other("class 文件被截断".into()))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

struct Pool(Vec<Constant>);

impl Pool {
    fn utf8(&self, i: u16) -> Result<String> {
        match self.0.get(i as usize) {
            Some(Constant::Utf8(s)) => Ok(s.clone()),
            _ => Err(Error::Other(format!("常量池 #{i} 不是字符串"))),
        }
    }
}

fn read_pool(r: &mut Reader) -> Result<Pool> {
    let count = r.u16()? as usize;
    let mut pool = vec![Constant::Other; count.max(1)];
    let mut i = 1;
    while i < count {
        let tag = r.u8()?;
        match tag {
            // Java 的"修改版 UTF-8"和标准 UTF-8 只在 \0 和补充字符上有差别，宽松解码即可
            1 => {
                let len = r.u16()? as usize;
                pool[i] = Constant::Utf8(String::from_utf8_lossy(r.bytes(len)?).into_owned());
            }
            3 => pool[i] = Constant::Int(r.u32()? as i32 as i64),
            4 => {
                r.u32()?;
            }
            5 | 6 => {
                let hi = r.u32()? as u64;
                let lo = r.u32()? as u64;
                if tag == 5 {
                    pool[i] = Constant::Int(((hi << 32) | lo) as i64);
                }
                // long/double 占两个槽位
                i += 1;
            }
            7 => pool[i] = Constant::Class(r.u16()?),
            8 | 16 | 19 | 20 => {
                r.u16()?;
            }
            15 => {
                r.bytes(3)?;
            }
            9 | 10 | 11 | 12 | 17 | 18 => {
                r.u32()?;
            }
            _ => return Err(Error::Other(format!("未知的常量池类型 {tag}"))),
        }
        i += 1;
    }
    Ok(Pool(pool))
}

fn read_element(r: &mut Reader, pool: &Pool) -> Result<ElementValue> {
    let tag = r.u8()?;
    Ok(match tag {
        b'B' | b'C' | b'I' | b'S' | b'Z' | b'J' => match pool.0.get(r.u16()? as usize) {
            Some(Constant::Int(v)) => ElementValue::Int(*v),
            _ => ElementValue::Other,
        },
        b'D' | b'F' => {
            r.u16()?;
            ElementValue::Other
        }
        b's' => ElementValue::Str(pool.utf8(r.u16()?)?),
        b'e' => {
            let type_desc = pool.utf8(r.u16()?)?;
            let name = pool.utf8(r.u16()?)?;
            ElementValue::Enum { type_desc, name }
        }
        b'c' => ElementValue::Class(pool.utf8(r.u16()?)?),
        b'@' => ElementValue::Annotation(read_annotation(r, pool)?),
        b'[' => {
            let n = r.u16()?;
            ElementValue::Array((0..n).map(|_| read_element(r, pool)).collect::<Result<_>>()?)
        }
        _ => return Err(Error::Other(format!("未知的注解值类型 {}", tag as char))),
    })
}

fn read_annotation(r: &mut Reader, pool: &Pool) -> Result<Annotation> {
    let desc = pool.utf8(r.u16()?)?;
    let n = r.u16()?;
    let mut values = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let key = pool.utf8(r.u16()?)?;
        values.push((key, read_element(r, pool)?));
    }
    Ok(Annotation { desc, values })
}

/// 读一组属性，只解析可见/不可见注解，其余跳过
fn read_attributes(r: &mut Reader, pool: &Pool) -> Result<Vec<Annotation>> {
    let mut out = Vec::new();
    for _ in 0..r.u16()? {
        let name = pool.utf8(r.u16()?)?;
        let len = r.u32()? as usize;
        let body = r.bytes(len)?;
        if name == "RuntimeVisibleAnnotations" || name == "RuntimeInvisibleAnnotations" {
            let mut sub = Reader { data: body, pos: 0 };
            for _ in 0..sub.u16()? {
                out.push(read_annotation(&mut sub, pool)?);
            }
        }
    }
    Ok(out)
}

pub fn parse(data: &[u8]) -> Result<ClassInfo> {
    let mut r = Reader { data, pos: 0 };
    if r.u32()? != 0xCAFEBABE {
        return Err(Error::Other("不是 class 文件".into()));
    }
    r.u32()?; // minor + major
    let pool = read_pool(&mut r)?;
    r.u16()?; // access
    let this_class = r.u16()?;
    let name = match pool.0.get(this_class as usize) {
        Some(Constant::Class(name_index)) => pool.utf8(*name_index)?,
        _ => return Err(Error::Other("找不到类名".into())),
    };
    r.u16()?; // super
    let interfaces = r.u16()?;
    r.bytes(interfaces as usize * 2)?;
    for _ in 0..r.u16()? {
        r.bytes(6)?;
        read_attributes(&mut r, &pool)?;
    }
    let mut methods = Vec::new();
    for _ in 0..r.u16()? {
        r.u16()?;
        let name = pool.utf8(r.u16()?)?;
        let desc = pool.utf8(r.u16()?)?;
        let annotations = read_attributes(&mut r, &pool)?;
        methods.push(MethodInfo { name, desc, annotations });
    }
    let annotations = read_attributes(&mut r, &pool)?;
    Ok(ClassInfo { name, methods, annotations })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 测试用的小型 class 文件生成器
    #[derive(Default)]
    pub struct ClassBuilder {
        pool: Vec<Vec<u8>>,
        methods: Vec<(u16, u16, Vec<u8>)>,
        class_annotations: Vec<u8>,
        class_annotation_count: u16,
    }

    impl ClassBuilder {
        pub fn utf8(&mut self, s: &str) -> u16 {
            let mut e = vec![1];
            e.extend((s.len() as u16).to_be_bytes());
            e.extend(s.as_bytes());
            self.pool.push(e);
            self.pool.len() as u16
        }
        pub fn int(&mut self, v: i32) -> u16 {
            let mut e = vec![3];
            e.extend(v.to_be_bytes());
            self.pool.push(e);
            self.pool.len() as u16
        }
        fn class(&mut self, name: &str) -> u16 {
            let n = self.utf8(name);
            let mut e = vec![7];
            e.extend(n.to_be_bytes());
            self.pool.push(e);
            self.pool.len() as u16
        }
        /// 注解：(键, 已编码的值)
        pub fn annotation(&mut self, desc: &str, values: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
            let mut out = self.utf8(desc).to_be_bytes().to_vec();
            out.extend((values.len() as u16).to_be_bytes());
            for (k, v) in values {
                out.extend(self.utf8(k).to_be_bytes());
                out.extend(v);
            }
            out
        }
        pub fn str_value(&mut self, s: &str) -> Vec<u8> {
            let mut v = vec![b's'];
            v.extend(self.utf8(s).to_be_bytes());
            v
        }
        pub fn class_value(&mut self, desc: &str) -> Vec<u8> {
            let mut v = vec![b'c'];
            v.extend(self.utf8(desc).to_be_bytes());
            v
        }
        pub fn int_value(&mut self, i: i32) -> Vec<u8> {
            let mut v = vec![b'I'];
            v.extend(self.int(i).to_be_bytes());
            v
        }
        pub fn array_value(items: Vec<Vec<u8>>) -> Vec<u8> {
            let mut v = vec![b'['];
            v.extend((items.len() as u16).to_be_bytes());
            for i in items {
                v.extend(i);
            }
            v
        }
        pub fn method(&mut self, name: &str, desc: &str, annotations: Vec<Vec<u8>>) {
            let n = self.utf8(name);
            let d = self.utf8(desc);
            let mut attrs = Vec::new();
            if !annotations.is_empty() {
                attrs.extend(self.annotations_attr(annotations));
            }
            self.methods.push((n, d, attrs));
        }
        pub fn class_annotation(&mut self, a: Vec<u8>) {
            self.class_annotations.extend(a);
            self.class_annotation_count += 1;
        }
        fn annotations_attr(&mut self, annotations: Vec<Vec<u8>>) -> Vec<u8> {
            let name = self.utf8("RuntimeInvisibleAnnotations");
            let mut body = (annotations.len() as u16).to_be_bytes().to_vec();
            for a in annotations {
                body.extend(a);
            }
            let mut out = name.to_be_bytes().to_vec();
            out.extend((body.len() as u32).to_be_bytes());
            out.extend(body);
            out
        }
        pub fn build(mut self, name: &str) -> Vec<u8> {
            let this = self.class(name);
            let class_attr = if self.class_annotation_count > 0 {
                let name = self.utf8("RuntimeInvisibleAnnotations");
                let mut body = self.class_annotation_count.to_be_bytes().to_vec();
                body.extend(&self.class_annotations);
                let mut out = name.to_be_bytes().to_vec();
                out.extend((body.len() as u32).to_be_bytes());
                out.extend(body);
                Some(out)
            } else {
                None
            };
            let mut out = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 65];
            out.extend(((self.pool.len() + 1) as u16).to_be_bytes());
            for e in &self.pool {
                out.extend(e);
            }
            out.extend([0, 0x21]);
            out.extend(this.to_be_bytes());
            out.extend([0, 0, 0, 0, 0, 0]); // super, interfaces, fields
            out.extend((self.methods.len() as u16).to_be_bytes());
            for (n, d, attrs) in &self.methods {
                out.extend([0, 1]);
                out.extend(n.to_be_bytes());
                out.extend(d.to_be_bytes());
                out.extend(if attrs.is_empty() { 0u16 } else { 1u16 }.to_be_bytes());
                out.extend(attrs);
            }
            match class_attr {
                Some(a) => {
                    out.extend(1u16.to_be_bytes());
                    out.extend(a);
                }
                None => out.extend(0u16.to_be_bytes()),
            }
            out
        }
    }

    #[test]
    fn parses_methods_and_annotations() {
        let mut b = ClassBuilder::default();
        let target = b.class_value("Lnet/minecraft/client/Minecraft;");
        let mixin = b.annotation("Lorg/spongepowered/asm/mixin/Mixin;", vec![("value", ClassBuilder::array_value(vec![target]))]);
        b.class_annotation(mixin);
        let sel = b.str_value("tick()V");
        let req = b.int_value(1);
        let inject = b.annotation("Lorg/spongepowered/asm/mixin/injection/Inject;", vec![("method", ClassBuilder::array_value(vec![sel])), ("require", req)]);
        b.method("onTick", "(Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;)V", vec![inject]);
        b.method("plain", "()V", vec![]);
        let class = parse(&b.build("me/test/MinecraftMixin")).unwrap();

        assert_eq!(class.name, "me/test/MinecraftMixin");
        assert_eq!(class.methods.len(), 2);
        let m = class.annotation("Lorg/spongepowered/asm/mixin/Mixin;").unwrap();
        assert_eq!(m.get("value"), Some(&ElementValue::Array(vec![ElementValue::Class("Lnet/minecraft/client/Minecraft;".into())])));
        let inj = &class.methods[0].annotations[0];
        assert_eq!(inj.get("require"), Some(&ElementValue::Int(1)));
        assert_eq!(inj.get("method"), Some(&ElementValue::Array(vec![ElementValue::Str("tick()V".into())])));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"not a class").is_err());
        assert!(parse(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0]).is_err());
    }
}
