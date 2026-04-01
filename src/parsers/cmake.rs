use crate::models::cmake::{CMakeInfo, FindPackage, TargetDependencies, TargetLinks};
use anyhow::Result;
use regex::Regex;
use std::fs;

pub fn parse_cmake_lists(path: &str) -> Result<CMakeInfo> {
    let text = fs::read_to_string(path)?;
    let mut info = CMakeInfo::default();

    let re_find =
        Regex::new(r"(?i)find_package\s*\(\s*([A-Za-z0-9_:+-]+)(?:\s+([0-9\.]+))?(?:.*REQUIRED)?\)")
            .unwrap();
    for cap in re_find.captures_iter(&text) {
        let name = cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let version = cap.get(2).map(|m| m.as_str().to_string());
        let required = cap
            .get(0)
            .map(|m| m.as_str().contains("REQUIRED"))
            .unwrap_or(false);
        info.find_packages.push(FindPackage {
            name,
            version,
            required,
        });
    }

    let re_exe = Regex::new(r"(?i)add_executable\s*\(\s*([^\s\)]+)").unwrap();
    for cap in re_exe.captures_iter(&text) {
        if let Some(name) = cap.get(1) {
            info.executables.push(name.as_str().to_string());
        }
    }

    let re_lib = Regex::new(r"(?i)add_library\s*\(\s*([^\s\)]+)(?:\s+(?:STATIC|SHARED|MODULE|OBJECT|INTERFACE))?").unwrap();
    for cap in re_lib.captures_iter(&text) {
        if let Some(name) = cap.get(1) {
            info.libraries.push(name.as_str().to_string());
        }
    }

    let re_ament_deps = Regex::new(r"(?i)ament_target_dependencies\s*\(\s*([^\s\)]+)\s+([^\)]*)\)").unwrap();
    for cap in re_ament_deps.captures_iter(&text) {
        let target = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let dependencies = cap
            .get(2)
            .map(|m| split_ament_target_dependencies_args(m.as_str()))
            .unwrap_or_default();
        info.ament_target_dependencies.push(TargetDependencies { target, dependencies });
    }

    let re_target_links = Regex::new(r"(?i)target_link_libraries\s*\(\s*([^\s\)]+)\s+([^\)]*)\)").unwrap();
    for cap in re_target_links.captures_iter(&text) {
        let target = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let libraries = cap
            .get(2)
            .map(|m| split_link_libraries_args(m.as_str()))
            .unwrap_or_default();
        info.target_link_libraries.push(TargetLinks { target, libraries });
    }

    if text.contains("catkin_package") {
        info.has_catkin_package = true;
    }
    if text.contains("ament_package") {
        info.has_ament_package = true;
    }

    Ok(info)
}

fn strip_cmake_line_comments(s: &str) -> String {
    s.lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_ament_target_dependencies_args(s: &str) -> Vec<String> {
    let cleaned = strip_cmake_line_comments(s);
    cleaned
        .split_whitespace()
        .filter(|item| !item.is_empty())
        .filter(|item| !matches!(*item, "SYSTEM" | "PUBLIC" | "INTERFACE"))
        .map(|item| item.trim().to_string())
        .collect()
}

fn split_link_libraries_args(s: &str) -> Vec<String> {
    let cleaned = strip_cmake_line_comments(s);
    cleaned
        .split_whitespace()
        .filter(|item| !item.is_empty())
        .filter(|item| {
            !matches!(
                *item,
                "PUBLIC"
                    | "PRIVATE"
                    | "INTERFACE"
                    | "debug"
                    | "optimized"
                    | "general"
                    | "LINK_PUBLIC"
                    | "LINK_PRIVATE"
                    | "LINK_INTERFACE_LIBRARIES"
            )
        })
        .map(|item| item.trim().to_string())
        .collect()
}
