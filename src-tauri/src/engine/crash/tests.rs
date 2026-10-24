use super::*;

fn rules() -> RuleSet {
    RuleSet::parse(BUILTIN_RULES).expect("内置规则必须能解析")
}

fn ids(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.id.as_str()).collect()
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/", $name))
    };
}

#[test]
fn builtin_rules_are_valid() {
    let r = rules();
    assert!(r.len() >= 20);
    // 规则 id 用于覆盖合并，不能重复
    let file: RuleFile = serde_json::from_str(BUILTIN_RULES).unwrap();
    let mut seen = HashSet::new();
    for rule in &file.rules {
        assert!(seen.insert(rule.id.clone()), "规则 id 重复：{}", rule.id);
    }
}

#[test]
fn fabric_missing_and_wrong_version() {
    let f = rules().analyze(fixture!("fabric_deps.log"), Some(1));
    assert!(ids(&f).contains(&"fabric-missing-dependency"));
    let missing = f.iter().find(|x| x.id == "fabric-missing-dependency").unwrap();
    assert_eq!(missing.title, "模组缺少前置：fabric-api");
    assert!(missing.description.contains("Sodium Extra"));
    let wrong = f.iter().find(|x| x.id == "fabric-wrong-dependency-version").unwrap();
    assert!(wrong.description.contains("1.20.4"), "{}", wrong.description);
    assert!(wrong.description.contains("1.20.1"));
    assert!(wrong.description.contains("Minecraft"));
}

#[test]
fn fabric_chinese_locale_messages() {
    // 系统语言是中文时 Fabric 会输出中文错误（取自真实日志）
    let f = rules().analyze(fixture!("fabric_deps_zh.log"), Some(1));
    let missing = f.iter().find(|x| x.id == "fabric-missing-dependency").unwrap();
    assert_eq!(missing.title, "模组缺少前置：fabric-api");
    assert!(missing.description.contains("GodBBS Crash Test"), "{}", missing.description);
    let wrong = f.iter().find(|x| x.id == "fabric-wrong-dependency-version").unwrap();
    assert!(wrong.description.contains("Mod Menu"));
    assert!(wrong.description.contains("1.20.1"), "{}", wrong.description);
}

#[test]
fn fabric_language_neutral_reason() {
    let f = rules().analyze(fixture!("fabric_deps_neutral.log"), Some(1));
    let missing = f.iter().find(|x| x.id == "fabric-missing-dependency").unwrap();
    assert!(missing.description.contains("sodium-extra"));
    assert_eq!(missing.title, "模组缺少前置：fabric-api");
}

#[test]
fn forge_missing_dependency() {
    let f = rules().analyze(fixture!("forge_deps.log"), Some(1));
    let d = f.iter().find(|x| x.id == "forge-missing-dependency").unwrap();
    assert_eq!(d.title, "模组缺少前置：geckolib");
    assert!(d.description.contains("alexsmobs"));
    assert!(d.description.contains("[4.0,)"));
    assert!(!d.description.contains('{'), "未替换的占位符：{}", d.description);
}

#[test]
fn java_too_old_computes_required_version() {
    let f = rules().analyze(fixture!("java_too_old.log"), Some(1));
    assert_eq!(f[0].id, "java-too-old");
    assert!(f[0].description.contains("Java 21"), "{}", f[0].description);
    // 已经有具体结论，兜底的"缺少游戏文件"不应出现
    assert!(!ids(&f).contains(&"class-not-found"));
}

#[test]
fn legacy_forge_on_new_java() {
    let f = rules().analyze(fixture!("legacy_java.log"), Some(1));
    assert_eq!(f[0].id, "java-too-new-legacy");
}

#[test]
fn out_of_memory() {
    let f = rules().analyze(fixture!("oom.log"), Some(-1));
    assert_eq!(f[0].id, "out-of-memory");
    assert!(f[0].description.contains("Java heap space"));
    assert_eq!(f[0].evidence.as_deref(), Some("java.lang.OutOfMemoryError: Java heap space"));
}

#[test]
fn natives_blocked() {
    let f = rules().analyze(fixture!("natives.log"), Some(1));
    assert_eq!(f[0].id, "natives-load-failed");
    assert!(f[0].description.contains("lwjgl.dll"));
}

#[test]
fn graphics_driver_from_hs_err() {
    let f = rules().analyze(fixture!("hs_err_intel.log"), Some(-1073741819));
    assert_eq!(f[0].id, "graphics-driver-crash");
    assert!(f[0].description.contains("ig9icd64.dll"));
    assert!(!ids(&f).contains(&"access-violation"), "有具体原因时不显示兜底");
}

#[test]
fn access_violation_by_exit_code_only() {
    let f = rules().analyze("nothing useful here", Some(-1073741819));
    assert_eq!(ids(&f), ["access-violation"]);
    assert!(f[0].description.contains("-1073741819"));
    assert!(rules().analyze("nothing useful here", Some(1)).is_empty());
}

#[test]
fn mixin_conflict_names_mod() {
    let f = rules().analyze(fixture!("mixin.log"), Some(1));
    let m = f.iter().find(|x| x.id == "mixin-failed").unwrap();
    assert!(m.description.contains("create"));
}

#[test]
fn manual_crash_suppresses_generic() {
    let f = rules().analyze(fixture!("manual.log"), Some(-1));
    assert_eq!(ids(&f), ["manual-crash"]);
    assert_eq!(f[0].severity, Severity::Info);
}

#[test]
fn bad_jvm_argument() {
    let f = rules().analyze(fixture!("jvm_arg.log"), Some(1));
    assert_eq!(f[0].id, "jvm-bad-argument");
    assert!(f[0].description.contains("UseZGCC"));
    assert!(!f[0].description.contains("{opt2}"));
}

#[test]
fn clean_exit_keeps_only_specific_errors() {
    let text = format!("{}\njava.lang.ClassNotFoundException: optional.Thing", fixture!("forge_deps.log"));
    let mut a = analyze_run(&rules(), &text, &[], Some(0));
    keep_for_clean_exit(&mut a);
    assert_eq!(ids(&a.findings), ["forge-missing-dependency"]);
    let mut noise = analyze_run(&rules(), "java.lang.ClassNotFoundException: optional.Thing", &[], Some(0));
    keep_for_clean_exit(&mut noise);
    assert!(noise.findings.is_empty(), "正常退出时日志噪音不报");
}

#[test]
fn clean_log_has_no_findings() {
    assert!(rules().analyze(fixture!("clean.log"), Some(0)).is_empty());
}

#[test]
fn errors_sort_before_warnings() {
    let text = format!("{}\n{}", fixture!("oom.log"), "java.lang.ClassNotFoundException: a.b");
    let f = rules().analyze(&text, Some(1));
    assert_eq!(f[0].severity, Severity::Error);
}

#[test]
fn user_rules_override_and_extend() {
    let custom = r#"{"version": 2, "rules": [
        {"id": "out-of-memory", "title": "自定义内存规则", "severity": "warning", "patterns": ["OutOfMemoryError"], "description": "改过了"},
        {"id": "my-rule", "title": "我的规则", "severity": "info", "patterns": ["SpecialMarker"], "description": "命中"}
    ]}"#;
    let r = RuleSet::with_override(BUILTIN_RULES, Some(custom)).unwrap();
    assert_eq!(r.version, 2);
    let f = r.analyze("java.lang.OutOfMemoryError\nSpecialMarker", None);
    assert!(f.iter().any(|x| x.title == "自定义内存规则" && x.description == "改过了"));
    assert!(f.iter().any(|x| x.id == "my-rule"));
}

#[test]
fn reports_which_rule_has_bad_regex() {
    let bad = r#"{"version": 1, "rules": [{"id": "broken", "title": "t", "severity": "error", "patterns": ["(unclosed"], "description": "d"}]}"#;
    let err = RuleSet::parse(bad).err().unwrap().to_string();
    assert!(err.contains("broken"), "{err}");
}

#[test]
fn collects_new_crash_files_only() {
    let dir = tempfile::tempdir().unwrap();
    let reports = dir.path().join("crash-reports");
    std::fs::create_dir_all(&reports).unwrap();
    std::fs::write(reports.join("crash-2020-01-01_00.00.00-client.txt"), "old").unwrap();
    let since = SystemTime::now() + std::time::Duration::from_secs(3600);
    assert!(collect_crash_files(dir.path(), since).is_empty(), "启动前的旧报告不算");
    std::fs::write(dir.path().join("hs_err_pid42.log"), "x").unwrap();
    let files = collect_crash_files(dir.path(), SystemTime::UNIX_EPOCH);
    assert_eq!(files.len(), 2);
}

#[test]
fn tail_respects_char_boundaries() {
    let s = "中文日志";
    assert_eq!(tail(s, 4), "志");
    assert_eq!(tail("abc", 10), "abc");
}
