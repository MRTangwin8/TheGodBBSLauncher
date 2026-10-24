//! Mojang 的日志配置让控制台输出 log4j XML 事件，这里把它还原成普通文本行。
//!
//! ```xml
//! <log4j:Event logger="..." timestamp="1700000000000" level="INFO" thread="Render thread">
//!   <log4j:Message><![CDATA[Sound engine started]]></log4j:Message>
//! </log4j:Event>
//! ```

#[derive(Default)]
pub struct Log4jParser {
    buf: Option<String>,
}

impl Log4jParser {
    /// 喂一行输出；返回可以显示的行（XML 事件没结束时返回空）
    pub fn feed(&mut self, line: &str) -> Vec<String> {
        if let Some(buf) = &mut self.buf {
            buf.push_str(line);
            buf.push('\n');
            if line.contains("</log4j:Event>") {
                let event = self.buf.take().unwrap_or_default();
                return format_event(&event);
            }
            // 防止异常输出导致无限缓存
            if buf.len() > 256 * 1024 {
                return self.buf.take().unwrap_or_default().lines().map(str::to_string).collect();
            }
            return vec![];
        }
        if line.trim_start().starts_with("<log4j:Event") {
            if line.contains("</log4j:Event>") {
                return format_event(line);
            }
            self.buf = Some(format!("{line}\n"));
            return vec![];
        }
        vec![line.to_string()]
    }
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn cdata<'a>(xml: &'a str, element: &str) -> Option<&'a str> {
    let open = format!("<log4j:{element}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</log4j:{element}>"))? + start;
    let inner = xml[start..end].trim();
    Some(inner.strip_prefix("<![CDATA[").and_then(|s| s.strip_suffix("]]>")).unwrap_or(inner))
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn format_event(xml: &str) -> Vec<String> {
    let head_end = xml.find('>').unwrap_or(xml.len());
    let head = &xml[..head_end];
    let thread = attr(head, "thread").map(unescape).unwrap_or_default();
    let level = attr(head, "level").unwrap_or("INFO");
    let message = cdata(xml, "Message").unwrap_or_default();
    let mut out: Vec<String> = message
        .lines()
        .enumerate()
        .map(|(i, l)| if i == 0 { format!("[{thread}/{level}]: {l}") } else { l.to_string() })
        .collect();
    if out.is_empty() {
        out.push(format!("[{thread}/{level}]:"));
    }
    if let Some(t) = cdata(xml, "Throwable") {
        out.extend(t.lines().map(str::to_string));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_event() {
        let mut p = Log4jParser::default();
        assert!(p.feed(r#"<log4j:Event logger="dqr" timestamp="1" level="WARN" thread="Render thread">"#).is_empty());
        assert!(p.feed("  <log4j:Message><![CDATA[Missing sound for event: a & b]]></log4j:Message>").is_empty());
        assert_eq!(p.feed("</log4j:Event>"), ["[Render thread/WARN]: Missing sound for event: a & b"]);
        assert_eq!(p.feed("plain line"), ["plain line"]);
    }

    #[test]
    fn keeps_throwable() {
        let mut p = Log4jParser::default();
        let lines = p.feed(r#"<log4j:Event level="ERROR" thread="main"><log4j:Message><![CDATA[boom]]></log4j:Message><log4j:Throwable><![CDATA[java.lang.RuntimeException
	at a.b(C.java:1)]]></log4j:Throwable></log4j:Event>"#);
        assert_eq!(lines, ["[main/ERROR]: boom", "java.lang.RuntimeException", "\tat a.b(C.java:1)"]);
    }
}
