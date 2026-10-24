//! 命令行检查模组兼容性。
//! 用法：cargo run --example compat -- <游戏目录> <版本id> <模组jar>...

use godbbs_launcher_lib::engine::install::GameDir;
use godbbs_launcher_lib::engine::resources::compat::{check_jar, GameEnv, RuntimeJar};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let game = GameDir::new(&args[0]);
    let env = GameEnv::detect(&game, &args[1]).await.expect("版本不存在");
    println!("环境：MC {} {:?} {:?}，游戏 jar：{:?}", env.mc, env.loader, env.loader_version, env.runtime_jar);
    let runtime = env.runtime_jar.as_deref().and_then(RuntimeJar::open);
    for jar in &args[2..] {
        let r = check_jar(std::path::Path::new(jar), &env, runtime.as_ref());
        println!("{:?}  {}", r.status, std::path::Path::new(jar).file_name().unwrap().to_string_lossy());
        for p in &r.problems {
            println!("    ✘ {p}");
        }
        for n in &r.notes {
            println!("    · {n}");
        }
    }
}
