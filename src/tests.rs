#[cfg(test)]
mod cli_tests {
    use std::fs::{create_dir_all, write};
    use std::path::PathBuf;
    use tempfile::tempdir;

    use crate::analyzer::analyze;
    use crate::commands;
    use crate::models::AnalysisReport;
    use crate::plugins::loader::load_rules_from_path;
    use crate::scanner::scan_workspace;

    #[test]
    fn integration_rules_and_scan() {
        let td = tempdir().expect("tempdir");
        let base = td.path();

        // create test workspace structure
        let src = base.join("src");
        let my_robot = src.join("my_robot");
        let my_robot_src = my_robot.join("src");
        create_dir_all(&my_robot_src).expect("mkdir");

        // package.xml (ROS1 with roscpp)
        let pkg = r#"<package format="2">
  <name>my_robot</name>
  <version>0.1.0</version>
  <depend>roscpp</depend>
</package>"#;
        write(my_robot.join("package.xml"), pkg).expect("write package.xml");

        // CMakeLists.txt
        let cmake = r#"cmake_minimum_required(VERSION 3.0.2)
find_package(catkin REQUIRED COMPONENTS roscpp)
"#;
        write(my_robot.join("CMakeLists.txt"), cmake).expect("write cmake");

        // C++ file with ros include
        let cpp = r#"#include <ros/ros.h>
int main() { return 0; }
"#;
        write(my_robot_src.join("my_node.cpp"), cpp).expect("write cpp");

        // create a custom rules dir with a TOML rule (optional)
        let rules_dir = base.join("rules");
        create_dir_all(&rules_dir).expect("mkdir rules");
        let rule_toml = r###"[meta]
name = "test_rules"
version = "0.1"

[[rules]]
id = "custom-include-ros"
name = "custom ros include"
severity = "info"
target = "cpp"

[rules.match]
type = "include"
pattern = "ros/ros.h"

[rules.output]
message = "custom detect ros include"
suggestion = "#include <rclcpp/rclcpp.hpp>"
"###;
        write(rules_dir.join("custom.toml"), rule_toml).expect("write toml");

        // load rules (pass rules dir and platform=ros1)
        let rules = load_rules_from_path(Some(rules_dir.clone()), Some("ros1".to_string()), true)
            .expect("load rules");
        assert!(rules.iter().any(|r| r.id == "ros1-header-ros"));
        assert!(rules.iter().any(|r| r.id == "custom-include-ros"));

        // scan and analyze
        let scan = scan_workspace(base.to_str().unwrap());
        let report = analyze(&scan, &rules);

        // Expect at least two findings: built-in header and dependency (or custom)
        assert!(!report.findings.is_empty(), "expected some findings");

        // check that ros include finding exists
        let has_ros_include = report
            .findings
            .iter()
            .any(|v| v.rule_id == "ros1-header-ros" || v.rule_id == "custom-include-ros");
        assert!(has_ros_include, "expected ros include rule to trigger");

        // check that roscpp dependency finding exists
        let has_roscpp = report
            .findings
            .iter()
            .any(|v| v.rule_id == "ros1-dep-roscpp");
        assert!(has_roscpp, "expected roscpp dependency rule to trigger");
    }

    #[test]
    fn analyze_command_writes_json_report() {
        let td = tempdir().expect("tempdir");
        let base = td.path();

        // Minimal workspace
        let src = base.join("src");
        let pkg_dir = src.join("my_pkg");
        create_dir_all(pkg_dir.join("src")).expect("mkdir");
        write(
                    pkg_dir.join("package.xml"),
                    r#"<package format="2"><name>my_pkg</name><version>0.1.0</version><depend>roscpp</depend></package>"#,
            )
            .expect("write package.xml");

        // output path
        let out_json = base.join("out.json");

        let args = commands::analyze::AnalyzeArgs {
            workspace_path: Some(PathBuf::from(base)),
            format: "json".to_string(),
            output: Some(out_json.clone()),
            rules: None,
            platform: Some("ros1".to_string()),
            no_builtin: false,
            list_rules: false,
            verbose: 0,
        };

        commands::analyze::run(args).expect("analyze should succeed");

        let bytes = std::fs::read(out_json).expect("read out.json");
        let report: AnalysisReport = serde_json::from_slice(&bytes).expect("parse json");

        assert!(report.summary.get("total_packages").copied().unwrap_or(0) >= 1);
        // total_findings may be 0 depending on builtin rules and workspace contents
    }

    #[test]
    fn report_command_writes_html() {
        let td = tempdir().expect("tempdir");
        let base = td.path();

        let input_json = base.join("report.json");
        let output_html = base.join("report.html");

        let report = AnalysisReport {
            summary: std::collections::HashMap::from([
                ("total_packages".to_string(), 1usize),
                ("total_findings".to_string(), 1usize),
            ]),
            packages: Vec::new(),
            findings: vec![crate::models::Finding {
                rule_id: "test-rule".to_string(),
                severity: "warning".to_string(),
                file: "src/main.cpp".to_string(),
                line: Some(42),
                message: "something happened".to_string(),
                suggestion: Some("try something else".to_string()),
                effort_hours: None,
            }],
        };

        std::fs::write(&input_json, serde_json::to_string_pretty(&report).unwrap())
            .expect("write report.json");

        let args = commands::report::ReportArgs {
            input: input_json,
            output: Some(output_html.clone()),
            config: None,
        };

        commands::report::run(args).expect("report should succeed");

        let html = std::fs::read_to_string(output_html).expect("read report.html");
        assert!(html.contains("<h1>Kiboku Report</h1>"));
        assert!(html.contains("<caption>Findings list</caption>"));
        assert!(html.contains("test-rule"));
        assert!(html.contains("something happened"));
        assert!(html.contains("try something else"));
    }

    #[test]
    fn report_command_writes_results_bundle() {
        let td = tempdir().expect("tempdir");
        let base = td.path();

        let ws_pkg_a_path = base.join("src/a");
        let ws_pkg_b_path = base.join("src/b");
        create_dir_all(&ws_pkg_a_path).expect("mkdir a");
        create_dir_all(&ws_pkg_b_path).expect("mkdir b");

        let input_json = base.join("report.json");
        let out_dir = base.join("results");

        let pkg_a = crate::models::Package {
            name: "a".to_string(),
            version: None,
            path: ws_pkg_a_path.to_string_lossy().to_string(),
            build_type: None,
            dependencies: vec![
                crate::models::package::Dependency {
                    name: "b".to_string(),
                    version: None,
                    kind: Some("build".to_string()),
                },
                crate::models::package::Dependency {
                    name: "roscpp".to_string(),
                    version: None,
                    kind: Some("exec".to_string()),
                },
            ],
            format: Some(2),
        };
        let pkg_b = crate::models::Package {
            name: "b".to_string(),
            version: None,
            path: ws_pkg_b_path.to_string_lossy().to_string(),
            build_type: None,
            dependencies: vec![],
            format: Some(2),
        };

        let report = AnalysisReport {
            summary: std::collections::HashMap::from([
                ("total_packages".to_string(), 2usize),
                ("total_findings".to_string(), 1usize),
            ]),
            packages: vec![pkg_a, pkg_b],
            findings: vec![crate::models::Finding {
                rule_id: "test-rule".to_string(),
                severity: "warning".to_string(),
                file: base.join("src/a/file.cpp").to_string_lossy().to_string(),
                line: Some(1),
                message: "msg".to_string(),
                suggestion: None,
                effort_hours: None,
            }],
        };

        std::fs::write(&input_json, serde_json::to_string_pretty(&report).unwrap())
            .expect("write report.json");

        let args = commands::report::ReportArgs {
            input: input_json,
            output: Some(out_dir.clone()),
            config: None,
        };
        commands::report::run(args).expect("report should succeed");

        assert!(out_dir.join("index.html").exists());
        assert!(out_dir.join("THIRD_PARTY_NOTICES.txt").exists());
        assert!(out_dir.join("assets/style.css").exists());
        assert!(out_dir.join("assets/app.js").exists());
        assert!(out_dir.join("assets/cytoscape.min.js").exists());
        assert!(out_dir.join("assets/graph.json").exists());

        let index_html =
            std::fs::read_to_string(out_dir.join("index.html")).expect("read index.html");
        assert!(index_html.contains("id=\"report-data\""));
        assert!(index_html.contains("id=\"report-config\""));
        assert!(!index_html.contains("__REPORT_DATA_JSON__"));
        assert!(!index_html.contains("__REPORT_CONFIG_JSON__"));

        let graph_txt =
            std::fs::read_to_string(out_dir.join("assets/graph.json")).expect("read graph.json");
        let v: serde_json::Value = serde_json::from_str(&graph_txt).expect("parse graph.json");

        let nodes = v
            .get("nodes")
            .and_then(|x| x.as_array())
            .expect("nodes array");
        let edges = v
            .get("edges")
            .and_then(|x| x.as_array())
            .expect("edges array");

        let node_ids: std::collections::HashSet<String> = nodes
            .iter()
            .filter_map(|n| {
                n.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        assert!(node_ids.contains("a"));
        assert!(node_ids.contains("b"));
        assert!(node_ids.contains("roscpp"));

        let edge_pairs: std::collections::HashSet<(String, String)> = edges
            .iter()
            .filter_map(|e| {
                let s = e.get("source")?.as_str()?.to_string();
                let t = e.get("target")?.as_str()?.to_string();
                Some((s, t))
            })
            .collect();
        assert!(edge_pairs.contains(&("a".to_string(), "b".to_string())));
        assert!(edge_pairs.contains(&("a".to_string(), "roscpp".to_string())));

        // Spot-check dep_type exists
        let has_dep_type = edges.iter().any(|e| e.get("dep_type").is_some());
        assert!(has_dep_type, "expected dep_type on edges");
    }

    /// Test helper: extract the embedded JSON payload from a `<script>` element by id.
    ///
    /// The report generator embeds JSON blobs (e.g. `report-data`, `report-config`) into
    /// `<script id="...">...</script>` tags. These tests use this helper to keep assertions
    /// focused on the embedded payload.
    ///
    /// Panics on malformed HTML because tests expect a well-formed report output.
    fn extract_embedded_json(html: &str, element_id: &str) -> String {
        let needle = format!("id=\"{}\"", element_id);
        let id_pos = html
            .find(&needle)
            .expect("expected embedded json element id");

        let open_tag_end = html[id_pos..].find('>').expect("expected > after id") + id_pos;
        let close_tag = "</script>";
        let close_pos = html[open_tag_end + 1..]
            .find(close_tag)
            .expect("expected </script>")
            + (open_tag_end + 1);

        html[open_tag_end + 1..close_pos].trim().to_string()
    }

    #[test]
    #[should_panic(expected = "expected embedded json element id")]
    fn extract_embedded_json_panics_when_element_id_missing() {
        let html = r#"<html><body><script id="other">{"a":1}</script></body></html>"#;
        let _ = extract_embedded_json(html, "report-config");
    }

    #[test]
    #[should_panic(expected = "expected </script>")]
    fn extract_embedded_json_panics_when_closing_tag_missing() {
        let html = r#"<html><body><script id="report-config">{"a":1}"#;
        let _ = extract_embedded_json(html, "report-config");
    }

    #[test]
    #[should_panic(expected = "expected > after id")]
    fn extract_embedded_json_panics_when_open_tag_malformed() {
        // Contains id="report-config" but no '>' following it.
        let html = r#"<script id="report-config""#;
        let _ = extract_embedded_json(html, "report-config");
    }

    #[test]
    fn report_command_embeds_section_heights() {
        let td = tempdir().expect("tempdir");
        let base = td.path();

        let ws_pkg_a_path = base.join("src/a");
        create_dir_all(&ws_pkg_a_path).expect("mkdir a");

        let input_json = base.join("report.json");
        let out_dir = base.join("results");
        let cfg_path = base.join("report.toml");

        let report = AnalysisReport {
            summary: std::collections::HashMap::from([
                ("total_packages".to_string(), 1usize),
                ("total_findings".to_string(), 0usize),
            ]),
            packages: vec![crate::models::Package {
                name: "a".to_string(),
                version: None,
                path: ws_pkg_a_path.to_string_lossy().to_string(),
                build_type: None,
                dependencies: vec![],
                format: Some(2),
            }],
            findings: vec![],
        };

        std::fs::write(&input_json, serde_json::to_string_pretty(&report).unwrap())
            .expect("write report.json");

        std::fs::write(
            &cfg_path,
            r#"title = "Test"

[section_heights]
workspace_dependencies = 666
external_dependencies = "55vh"
rounded_float = 123.6
invalid_css = "nope"
empty_string = ""
whitespace_only = "   "
ignored_bool = true
ignored_array = ["a", "b"]
ignored_table = { a = 1 }
"#,
        )
        .expect("write report.toml");

        let args = commands::report::ReportArgs {
            input: input_json,
            output: Some(out_dir.clone()),
            config: Some(cfg_path),
        };
        commands::report::run(args).expect("report should succeed");

        let index_html =
            std::fs::read_to_string(out_dir.join("index.html")).expect("read index.html");
        let embedded = extract_embedded_json(&index_html, "report-config");
        let v: serde_json::Value =
            serde_json::from_str(&embedded).expect("parse embedded report-config JSON");

        let heights = v
            .get("section_heights")
            .and_then(|x| x.as_object())
            .expect("section_heights object");

        assert_eq!(
            heights
                .get("workspace_dependencies")
                .and_then(|x| x.as_str()),
            Some("666px")
        );
        assert_eq!(
            heights
                .get("external_dependencies")
                .and_then(|x| x.as_str()),
            Some("55vh")
        );
        assert_eq!(
            heights.get("rounded_float").and_then(|x| x.as_str()),
            Some("124px")
        );
        assert_eq!(
            heights.get("invalid_css").and_then(|x| x.as_str()),
            Some("nope")
        );

        assert!(heights.get("empty_string").is_none());
        assert!(heights.get("whitespace_only").is_none());
        assert!(heights.get("ignored_bool").is_none());
        assert!(heights.get("ignored_array").is_none());
        assert!(heights.get("ignored_table").is_none());
    }
}
