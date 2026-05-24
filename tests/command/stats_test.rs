use std::fs;

use super::*;

#[test]
fn test_stats_cli_outside_repository_works() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("hello.rs"), "fn main() {}").unwrap();

    let output = run_libra_command(&["stats"], temp.path());
    assert_cli_success(&output, "stats should work outside a repository");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(".rs"),
        "expected .rs in output, got: {stdout}"
    );
}

#[test]
fn test_stats_counts_extensions_in_workdir() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("foo.rs"), "fn a() {}").unwrap();
    fs::write(temp.path().join("bar.rs"), "fn b() {}").unwrap();
    fs::write(temp.path().join("readme.md"), "# Title").unwrap();
    fs::write(temp.path().join("LICENSE"), "MIT").unwrap();

    let output = run_libra_command(&["stats"], temp.path());
    assert_cli_success(&output, "stats should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("   2  .rs"),
        "expected 2 .rs files, got: {stdout}"
    );
    assert!(
        stdout.contains("   1  .md"),
        "expected 1 .md file, got: {stdout}"
    );
    assert!(
        stdout.contains("   1  no_extension"),
        "expected 1 no_extension file, got: {stdout}"
    );
}

#[test]
fn test_stats_ignores_libra_and_target() {
    let temp = tempdir().unwrap();

    fs::create_dir_all(temp.path().join(".libra/objects")).unwrap();
    fs::create_dir_all(temp.path().join("target/debug")).unwrap();
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join(".libra/objects/abc123"), "data").unwrap();
    fs::write(temp.path().join("target/debug/libra"), "binary").unwrap();
    fs::write(temp.path().join("src/lib.rs"), "pub fn f() {}").unwrap();

    let output = run_libra_command(&["stats"], temp.path());
    assert_cli_success(&output, "stats should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("   1  .rs"),
        "expected only .rs file, got: {stdout}"
    );
    assert!(
        !stdout.contains("no_extension"),
        "should not count files in .libra or target, got: {stdout}"
    );
}

#[test]
fn test_stats_json_output() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("a.rs"), "fn a() {}").unwrap();
    fs::write(temp.path().join("b.md"), "# B").unwrap();

    let output = run_libra_command(&["stats", "--json"], temp.path());
    assert_cli_success(&output, "stats --json should succeed");
    let json = parse_json_stdout(&output);

    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "stats");

    let data = &json["data"];
    assert_eq!(data["total_files"], 2);

    let stats = data["stats"].as_array().unwrap();
    let mut ext_map: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for entry in stats {
        let ext = entry["extension"].as_str().unwrap();
        let count = entry["count"].as_u64().unwrap() as usize;
        ext_map.insert(ext, count);
    }
    assert_eq!(ext_map.get(".rs"), Some(&1));
    assert_eq!(ext_map.get(".md"), Some(&1));
}

#[test]
fn test_stats_empty_directory() {
    let temp = tempdir().unwrap();
    let output = run_libra_command(&["stats"], temp.path());
    assert_cli_success(&output, "stats on empty directory should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.is_empty() || stdout.trim().is_empty(),
        "expected empty output, got: {stdout}"
    );
}

#[test]
fn test_stats_json_empty_directory() {
    let temp = tempdir().unwrap();
    let output = run_libra_command(&["stats", "--json"], temp.path());
    assert_cli_success(&output, "stats --json on empty directory should succeed");
    let json = parse_json_stdout(&output);

    assert_eq!(json["ok"], true);
    assert_eq!(json["command"], "stats");

    let data = &json["data"];
    assert_eq!(data["total_files"], 0);
    assert!(data["stats"].as_array().unwrap().is_empty());
}
