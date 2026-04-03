use crate::models::cmake::{CMakeInfo, FindPackage, TargetDependencies, TargetLinks};
use anyhow::Result;
use cmake_parser::{parse_cmakelists, Command, Doc, Token};
use regex::Regex;
use std::fs;

pub fn parse_cmake_lists(path: &str) -> Result<CMakeInfo> {
    let text = fs::read_to_string(path)?;
    let mut info = CMakeInfo::default();

    parse_standard_commands(&text, &mut info)?;
    normalize_find_package_required_flags(&text, &mut info)?;
    parse_ros_commands(&text, &mut info)?;

    Ok(info)
}

fn parse_standard_commands(text: &str, info: &mut CMakeInfo) -> Result<()> {
    let tokens = parse_cmakelists(text.as_bytes())?;
    let doc = Doc::from(tokens);

    for cmd in doc.to_commands_iter() {
        let Ok(cmd) = cmd else {
            continue;
        };

        match cmd {
            Command::FindPackage(pkg) => {
                let (name, version, required) = match pkg.as_ref() {
                    cmake_parser::command::scripting::FindPackage::Basic(basic) => (
                        token_to_string(&basic.package_name),
                        basic.version.as_ref().map(token_to_string),
                        required_from_basic_components(basic.components.as_ref()),
                    ),
                    cmake_parser::command::scripting::FindPackage::Full(full) => (
                        token_to_string(&full.package_name),
                        full.version.as_ref().map(token_to_string),
                        required_from_full_components(full.components.as_ref()),
                    ),
                };

                info.find_packages.push(FindPackage {
                    name,
                    version,
                    required,
                });
            }
            Command::AddExecutable(exec) => {
                info.executables.push(token_to_string(&exec.name));
            }
            Command::AddLibrary(lib) => {
                info.libraries.push(token_to_string(&lib.name));
            }
            _ => {}
        }
    }

    Ok(())
}

fn normalize_find_package_required_flags(text: &str, info: &mut CMakeInfo) -> Result<()> {
    let re_find = Regex::new(r"(?is)(?m)(^|\n)\s*find_package\s*\((.*?)\)")?;
    let mut required_by_index = Vec::new();

    for cap in re_find.captures_iter(text) {
        let Some(args) = cap.get(2).map(|m| m.as_str()) else {
            continue;
        };
        let tokens = tokenize_unquoted_args(args);
        if tokens.is_empty() {
            continue;
        }
        required_by_index.push(tokens.iter().skip(1).any(|t| t.eq_ignore_ascii_case("REQUIRED")));
    }

    for (pkg, required) in info.find_packages.iter_mut().zip(required_by_index.into_iter()) {
        pkg.required = required;
    }

    Ok(())
}

fn parse_ros_commands(text: &str, info: &mut CMakeInfo) -> Result<()> {
    let re_pkg = Regex::new(r"(?is)(?m)(^|\n)\s*(catkin_package|ament_package)\s*\((.*?)\)")?;
    for cap in re_pkg.captures_iter(text) {
        let Some(name) = cap.get(2).map(|m| m.as_str().to_ascii_lowercase()) else {
            continue;
        };
        match name.as_str() {
            "catkin_package" => info.has_catkin_package = true,
            "ament_package" => info.has_ament_package = true,
            _ => {}
        }
    }

    let re_ament = Regex::new(r"(?is)(?m)(^|\n)\s*ament_target_dependencies\s*\((.*?)\)")?;
    for cap in re_ament.captures_iter(text) {
        let Some(args) = cap.get(2).map(|m| m.as_str()) else {
            continue;
        };
        let tokens = tokenize_unquoted_args(args);
        if let Some((target, rest)) = tokens.split_first() {
            let dependencies = rest
                .iter()
                .filter(|item| !is_ament_dependency_keyword(item))
                .cloned()
                .collect::<Vec<_>>();
            info.ament_target_dependencies.push(TargetDependencies {
                target: target.clone(),
                dependencies,
            });
        }
    }

    let re_link = Regex::new(r"(?is)(?m)(^|\n)\s*target_link_libraries\s*\((.*?)\)")?;
    for cap in re_link.captures_iter(text) {
        let Some(args) = cap.get(2).map(|m| m.as_str()) else {
            continue;
        };
        let tokens = tokenize_unquoted_args(args);
        if let Some((target, rest)) = tokens.split_first() {
            let libraries = rest
                .iter()
                .filter(|item| !is_target_link_keyword(item))
                .cloned()
                .collect::<Vec<_>>();
            info.target_link_libraries.push(TargetLinks {
                target: target.clone(),
                libraries,
            });
        }
    }

    Ok(())
}

fn required_from_basic_components(
    components: Option<&cmake_parser::command::scripting::find_package::PackageComponents<'_>>,
) -> bool {
    use cmake_parser::command::scripting::find_package::PackageComponents;
    matches!(components, Some(PackageComponents::Required(_)))
}

fn required_from_full_components(
    components: Option<&cmake_parser::command::scripting::find_package::PackageComponents<'_>>,
) -> bool {
    use cmake_parser::command::scripting::find_package::PackageComponents;
    matches!(components, Some(PackageComponents::Required(_)))
}

fn token_to_string(token: &Token<'_>) -> String {
    token.to_string()
}

fn tokenize_unquoted_args(s: &str) -> Vec<String> {
    let mut stripped = String::new();
    for line in s.lines() {
        let before_comment = line.split('#').next().unwrap_or_default();
        stripped.push_str(before_comment);
        stripped.push('\n');
    }
    stripped
        .split_whitespace()
        .map(|x| x.to_string())
        .collect()
}

fn is_ament_dependency_keyword(s: &str) -> bool {
    matches!(s.to_ascii_uppercase().as_str(), "SYSTEM" | "PUBLIC" | "INTERFACE")
}

fn is_target_link_keyword(s: &str) -> bool {
    matches!(
        s.to_ascii_uppercase().as_str(),
        "PUBLIC"
            | "PRIVATE"
            | "INTERFACE"
            | "DEBUG"
            | "OPTIMIZED"
            | "GENERAL"
            | "LINK_PUBLIC"
            | "LINK_PRIVATE"
            | "LINK_INTERFACE_LIBRARIES"
    )
}
