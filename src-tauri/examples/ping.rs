//! 命令行冒烟测试：查询一个服务器的状态（延迟、在线人数、MOTD），没写端口时先查 SRV 记录。
//! 用法：cargo run --example ping -- <服务器地址，如 mc.example.com 或 1.2.3.4:25565>

use godbbs_launcher_lib::engine::launch::server::parse_server;
use godbbs_launcher_lib::engine::servers::ping;

#[tokio::main]
async fn main() {
    let address = std::env::args().nth(1).expect("缺少服务器地址");
    let (host, port) = parse_server(&address).expect("地址不合法");
    match ping(&host, port).await {
        Ok(s) => {
            println!("{}（协议 {}）：{}/{} 人在线，{} ms", s.version, s.protocol, s.online, s.max, s.latency_ms);
            println!("MOTD：{}", s.motd);
            println!("图标：{}", if s.favicon.is_some() { "有" } else { "无" });
        }
        Err(e) => println!("查询失败：{e}"),
    }
}
