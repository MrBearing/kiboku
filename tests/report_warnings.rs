use std::process::Command;

use tempfile::tempdir;

#[test]
fn report_emits_warnings_for_ignored_section_heights_entries() {
    let td = tempdir().expect("tempdir");
    let base = td.path();

    let input_json = base.join("report.json");
    let out_dir = base.join("results");
    let cfg_path = base.join("report.toml");

    // Minimal AnalysisReport JSON (matches src/models/report.rs + src/models/package.rs)
    let report_json = format!(
        "{{\n  \"summary\": {{\"total_packages\": 1, \"total_findings\": 0}},\n  \"packages\": [{{\n    \"name\": \"a\",\n    \"version\": null,\n    \"path\": \"{}\",\n    \"build_type\": null,\n    \"dependencies\": [],\n    \"format\": 2\n  }}],\n  \"findings\": []\n}}\n",
        base.join("src/a").to_string_lossy()
    );
    std::fs::create_dir_all(base.join("src/a")).expect("mkdir a");
    std::fs::write(&input_json, report_json).expect("write report.json");

    std::fs::write(
        &cfg_path,
        r#"title = "Test"

[section_heights]
empty_string = ""
whitespace_only = "   "
invalid_css = "nope"
ignored_bool = true
ignored_array = ["a", "b"]
ignored_table = { a = 1 }
"#,
    )
    .expect("write report.toml");

    // Run the CLI in a subprocess so we can reliably capture stderr.
    let exe = env!("CARGO_BIN_EXE_bok");
    let output = Command::new(exe)
        .args([
            "report",
            input_json.to_str().unwrap(),
            "--output",
            out_dir.to_str().unwrap(),
            "--config",
            cfg_path.to_str().unwrap(),
        ])
        .output()
        .expect("run bok report");

    assert!(output.status.success(), "bok report failed: {output:?}");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("empty_string") && stderr.contains("empty after trimming"),
        "expected warning for empty_string, got stderr: {stderr}"
    );
    assert!(
        stderr.contains("whitespace_only") && stderr.contains("empty after trimming"),
        "expected warning for whitespace_only, got stderr: {stderr}"
    );
    assert!(
        stderr.contains("ignored_bool") && stderr.contains("has unsupported type"),
        "expected warning for ignored_bool, got stderr: {stderr}"
    );
    assert!(
        stderr.contains("ignored_array") && stderr.contains("has unsupported type"),
        "expected warning for ignored_array, got stderr: {stderr}"
    );
    assert!(
        stderr.contains("ignored_table") && stderr.contains("has unsupported type"),
        "expected warning for ignored_table, got stderr: {stderr}"
    );

    assert!(
        stderr.contains("invalid_css") && stderr.contains("looks like an invalid CSS length"),
        "expected warning for invalid_css, got stderr: {stderr}"
    );
}
