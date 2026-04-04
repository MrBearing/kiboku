#[cfg(test)]
mod cli_tests {
    use std::fs::{create_dir_all, write};
    use std::path::PathBuf;
    use tempfile::tempdir;

    use crate::analyzer::analyze;
    use crate::commands;
    use crate::models::AnalysisReport;
    use crate::parsers::cmake::parse_cmake_lists;
    use crate::parsers::package::parse_package_xml;
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
    fn parse_package_xml_prefers_export_build_type() {
        let td = tempdir().expect("tempdir");
        let pkg_path = td.path().join("package.xml");
        write(
            &pkg_path,
            r#"<package format="3">
  <name>my_pkg</name>
  <version>0.1.0</version>
  <depend>ament_cmake</depend>
  <export>
    <build_type>catkin</build_type>
  </export>
</package>"#,
        )
        .expect("write package.xml");

        let pkg = parse_package_xml(pkg_path.to_str().unwrap()).expect("parse package.xml");
        assert_eq!(pkg.build_type.as_deref(), Some("catkin"));
    }

    #[test]
    fn parse_package_xml_falls_back_to_dependency_heuristic() {
        let td = tempdir().expect("tempdir");
        let pkg_path = td.path().join("package.xml");
        write(
            &pkg_path,
            r#"<package format="3">
  <name>my_pkg</name>
  <version>0.1.0</version>
  <depend>ament_cmake</depend>
</package>"#,
        )
        .expect("write package.xml");

        let pkg = parse_package_xml(pkg_path.to_str().unwrap()).expect("parse package.xml");
        assert_eq!(pkg.build_type.as_deref(), Some("ament_cmake"));
    }

    #[test]
    fn parse_cmake_lists_extracts_targets_and_dependencies() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"cmake_minimum_required(VERSION 3.8)
project(sample_pkg)
find_package(ament_cmake REQUIRED)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)

add_executable(my_node src/my_node.cpp)
add_library(my_lib SHARED src/my_lib.cpp)
ament_target_dependencies(my_node rclcpp std_msgs)
target_link_libraries(my_node my_lib foo::bar)
ament_package()
"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.executables.iter().any(|t| t == "my_node"));
        assert!(info.libraries.iter().any(|t| t == "my_lib"));
        assert!(info.ament_target_dependencies.iter().any(|entry| {
            entry.target == "my_node" && entry.dependencies == vec!["rclcpp", "std_msgs"]
        }));
        assert!(info.target_link_libraries.iter().any(|entry| {
            entry.target == "my_node" && entry.libraries == vec!["my_lib", "foo::bar"]
        }));
    }

    #[test]
    fn parse_cmake_lists_supports_variable_target_names() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"cmake_minimum_required(VERSION 3.8)
project(sample_pkg)

add_executable(${PROJECT_NAME}_node src/my_node.cpp)
ament_target_dependencies(${PROJECT_NAME}_node rclcpp std_msgs)
target_link_libraries(${PROJECT_NAME}_node ${PROJECT_NAME}_core foo::bar)
"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.executables.iter().any(|t| t == "${PROJECT_NAME}_node"));
        assert!(info.ament_target_dependencies.iter().any(|entry| {
            entry.target == "${PROJECT_NAME}_node"
                && entry.dependencies == vec!["rclcpp", "std_msgs"]
        }));
        assert!(info.target_link_libraries.iter().any(|entry| {
            entry.target == "${PROJECT_NAME}_node"
                && entry.libraries == vec!["${PROJECT_NAME}_core", "foo::bar"]
        }));
    }

    #[test]
    fn parse_cmake_lists_ignores_link_visibility_keywords() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"target_link_libraries(my_node PUBLIC my_lib PRIVATE foo::bar INTERFACE baz::qux optimized optlib debug dbgllib general genlib)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.target_link_libraries.iter().any(|entry| {
            entry.target == "my_node"
                && entry.libraries
                    == vec![
                        "my_lib", "foo::bar", "baz::qux", "optlib", "dbgllib", "genlib",
                    ]
        }));
    }

    #[test]
    fn parse_cmake_lists_ignores_ament_dependency_keywords() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"ament_target_dependencies(my_node SYSTEM PUBLIC INTERFACE rclcpp std_msgs)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.ament_target_dependencies.iter().any(|entry| {
            entry.target == "my_node" && entry.dependencies == vec!["rclcpp", "std_msgs"]
        }));
    }

    #[test]
    fn parse_cmake_lists_ignores_legacy_link_keywords() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"target_link_libraries(my_node LINK_PUBLIC my_lib LINK_PRIVATE foo::bar LINK_INTERFACE_LIBRARIES baz::qux)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.target_link_libraries.iter().any(|entry| {
            entry.target == "my_node" && entry.libraries == vec!["my_lib", "foo::bar", "baz::qux"]
        }));
    }

    #[test]
    fn parse_cmake_lists_supports_uppercase_commands() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"FIND_PACKAGE(rclcpp required)
FIND_PACKAGE(std_msgs REQUIRED)
FIND_PACKAGE(foo COMPONENTS required_tools)
ADD_EXECUTABLE(MY_NODE src/my_node.cpp)
ADD_LIBRARY(MY_LIB SHARED src/my_lib.cpp)
AMENT_TARGET_DEPENDENCIES(MY_NODE rclcpp std_msgs)
TARGET_LINK_LIBRARIES(MY_NODE public MY_LIB foo::bar)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info
            .find_packages
            .iter()
            .any(|pkg| pkg.name == "rclcpp" && pkg.required));
        assert!(info
            .find_packages
            .iter()
            .any(|pkg| pkg.name == "std_msgs" && pkg.required));
        assert!(info
            .find_packages
            .iter()
            .any(|pkg| pkg.name == "foo" && !pkg.required));
        assert_eq!(info.find_packages.len(), 3);
        assert!(info.executables.iter().any(|t| t == "MY_NODE"));
        assert!(info.libraries.iter().any(|t| t == "MY_LIB"));
        assert!(info.ament_target_dependencies.iter().any(|entry| {
            entry.target == "MY_NODE" && entry.dependencies == vec!["rclcpp", "std_msgs"]
        }));
        assert!(info.target_link_libraries.iter().any(|entry| {
            entry.target == "MY_NODE" && entry.libraries == vec!["MY_LIB", "foo::bar"]
        }));
    }

    #[test]
    fn parse_cmake_lists_strips_line_comments_from_args() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"ament_target_dependencies(my_node rclcpp # core ros client
  std_msgs)
target_link_libraries(my_node my_lib # internal lib
  foo::bar)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.ament_target_dependencies.iter().any(|entry| {
            entry.target == "my_node" && entry.dependencies == vec!["rclcpp", "std_msgs"]
        }));
        assert!(info.target_link_libraries.iter().any(|entry| {
            entry.target == "my_node" && entry.libraries == vec!["my_lib", "foo::bar"]
        }));
    }

    #[test]
    fn parse_cmake_lists_ignores_commented_out_commands() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"# add_executable(old_node src/old.cpp)
# ADD_LIBRARY(OLD_LIB SHARED src/old.cpp)
add_executable(real_node src/real.cpp)
# target_link_libraries(old_node old_lib)
target_link_libraries(real_node real_lib)
# catkin_package()
# ament_package()"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert_eq!(info.executables, vec!["real_node"]);
        assert!(info.libraries.is_empty());
        assert!(!info.has_catkin_package);
        assert!(!info.has_ament_package);
        assert!(info
            .target_link_libraries
            .iter()
            .any(|entry| { entry.target == "real_node" && entry.libraries == vec!["real_lib"] }));
        assert!(!info
            .target_link_libraries
            .iter()
            .any(|entry| entry.target == "old_node"));
    }

    #[test]
    fn parse_cmake_lists_detects_package_commands_case_insensitively() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"CATKIN_PACKAGE()
AMENT_PACKAGE()"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.has_catkin_package);
        assert!(info.has_ament_package);
    }

    #[test]
    fn parse_cmake_lists_ignores_bracket_commented_ros_commands() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"find_package(rclcpp REQUIRED)
#[[
find_package(fake_pkg REQUIRED)
ament_target_dependencies(fake_node fake_dep)
target_link_libraries(fake_node fake_lib)
ament_package()
]]
add_executable(real_node src/real.cpp)
ament_target_dependencies(real_node rclcpp)
target_link_libraries(real_node real_lib)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info
            .find_packages
            .iter()
            .any(|pkg| pkg.name == "rclcpp" && pkg.required));
        assert!(!info.find_packages.iter().any(|pkg| pkg.name == "fake_pkg"));
        assert!(!info.has_ament_package);
        assert!(info
            .ament_target_dependencies
            .iter()
            .any(|entry| { entry.target == "real_node" && entry.dependencies == vec!["rclcpp"] }));
        assert!(!info
            .ament_target_dependencies
            .iter()
            .any(|entry| entry.target == "fake_node"));
        assert!(info
            .target_link_libraries
            .iter()
            .any(|entry| { entry.target == "real_node" && entry.libraries == vec!["real_lib"] }));
        assert!(!info
            .target_link_libraries
            .iter()
            .any(|entry| entry.target == "fake_node"));
    }

    #[test]
    fn parse_cmake_lists_does_not_match_wrapper_macro_names() {
        let td = tempdir().expect("tempdir");
        let cmake_path = td.path().join("CMakeLists.txt");
        write(
            &cmake_path,
            r#"foo_add_executable(fake_node src/fake.cpp)
my_target_link_libraries(fake_node fake_lib)
real_add_library(fake_lib src/fake.cpp)"#,
        )
        .expect("write CMakeLists.txt");

        let info = parse_cmake_lists(cmake_path.to_str().unwrap()).expect("parse cmake");

        assert!(info.executables.is_empty());
        assert!(info.libraries.is_empty());
        assert!(info.target_link_libraries.is_empty());
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
