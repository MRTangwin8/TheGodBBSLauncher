//! 用内置规则分析一个日志 / 崩溃报告文件。
//! 用法：cargo run --example analyze -- <文件路径> [退出码]

use godbbs_launcher_lib::engine::crash::{analyze_run, RuleSet, BUILTIN_RULES};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("缺少文件路径");
    let code = args.next().and_then(|c| c.parse().ok());
    let rules = RuleSet::parse(BUILTIN_RULES).expect("内置规则");
    let analysis = analyze_run(&rules, "", &[path.into()], code);
    if analysis.findings.is_empty() {
        println!("未识别出原因");
    }
    for f in &analysis.findings {
        println!("[{:?}] {}\n  {}", f.severity, f.title, f.description);
        for (i, s) in f.solutions.iter().enumerate() {
            println!("  {}. {s}", i + 1);
        }
    }
}
