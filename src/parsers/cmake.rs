use crate::models::cmake::{CMakeInfo, FindPackage, TargetDependencies, TargetLinks};
use anyhow::Result;
use std::fs;

#[derive(Debug, Clone)]
struct CMakeCommand {
    name: String,
    args_raw: String,
}

pub fn parse_cmake_lists(path: &str) -> Result<CMakeInfo> {
    let text = fs::read_to_string(path)?;
    let commands = extract_cmake_commands(&text);
    let mut info = CMakeInfo::default();

    for cmd in &commands {
        match cmd.name.as_str() {
            "find_package" => {
                let tokens = tokenize_cmake_args(&cmd.args_raw);
                if tokens.is_empty() {
                    continue;
                }
                let name = tokens[0].clone();
                let version = tokens
                    .get(1)
                    .filter(|t| t.chars().all(|c| c.is_ascii_digit() || c == '.'))
                    .cloned();
                let required = tokens.iter().any(|t| t.eq_ignore_ascii_case("REQUIRED"));
                info.find_packages.push(FindPackage {
                    name,
                    version,
                    required,
                });
            }
            "add_executable" => {
                let tokens = tokenize_cmake_args(&cmd.args_raw);
                if let Some(target) = tokens.first() {
                    info.executables.push(target.clone());
                }
            }
            "add_library" => {
                let tokens = tokenize_cmake_args(&cmd.args_raw);
                if let Some(target) = tokens.first() {
                    info.libraries.push(target.clone());
                }
            }
            "ament_target_dependencies" => {
                let tokens = tokenize_cmake_args(&cmd.args_raw);
                if let Some((target, rest)) = tokens.split_first() {
                    let dependencies = rest
                        .iter()
                        .filter(|item| {
                            !matches!(
                                item.to_ascii_uppercase().as_str(),
                                "SYSTEM" | "PUBLIC" | "INTERFACE"
                            )
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    info.ament_target_dependencies.push(TargetDependencies {
                        target: target.clone(),
                        dependencies,
                    });
                }
            }
            "target_link_libraries" => {
                let tokens = tokenize_cmake_args(&cmd.args_raw);
                if let Some((target, rest)) = tokens.split_first() {
                    let libraries = rest
                        .iter()
                        .filter(|item| {
                            !matches!(
                                item.to_ascii_uppercase().as_str(),
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
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    info.target_link_libraries.push(TargetLinks {
                        target: target.clone(),
                        libraries,
                    });
                }
            }
            "catkin_package" => {
                info.has_catkin_package = true;
            }
            "ament_package" => {
                info.has_ament_package = true;
            }
            _ => {}
        }
    }

    Ok(info)
}

fn extract_cmake_commands(s: &str) -> Vec<CMakeCommand> {
    let mut commands = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let name: String = chars[start..i].iter().collect();

            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j >= chars.len() || chars[j] != '(' {
                i = j;
                continue;
            }

            j += 1;
            let args_start = j;
            let mut depth = 1i32;
            let mut in_single = false;
            let mut in_double = false;
            let mut escaped = false;

            while j < chars.len() {
                let c = chars[j];
                if escaped {
                    escaped = false;
                    j += 1;
                    continue;
                }
                if c == '\\' && (in_single || in_double) {
                    escaped = true;
                    j += 1;
                    continue;
                }
                if c == '"' && !in_single {
                    in_double = !in_double;
                    j += 1;
                    continue;
                }
                if c == '\'' && !in_double {
                    in_single = !in_single;
                    j += 1;
                    continue;
                }
                if in_single || in_double {
                    j += 1;
                    continue;
                }
                if c == '#' {
                    while j < chars.len() && chars[j] != '\n' {
                        j += 1;
                    }
                    continue;
                }
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        let args_raw: String = chars[args_start..j].iter().collect();
                        commands.push(CMakeCommand {
                            name: name.to_ascii_lowercase(),
                            args_raw,
                        });
                        j += 1;
                        break;
                    }
                }
                j += 1;
            }
            i = j;
            continue;
        }

        i += 1;
    }

    commands
}

fn tokenize_cmake_args(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    while i < chars.len() {
        let c = chars[i];
        if escaped {
            current.push(c);
            escaped = false;
            i += 1;
            continue;
        }
        if c == '\\' && (in_single || in_double) {
            escaped = true;
            i += 1;
            continue;
        }
        if c == '"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }
        if c == '\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if (in_single || in_double) && c == '#' {
            current.push(c);
            i += 1;
            continue;
        }
        if !in_single && !in_double && c.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            i += 1;
            continue;
        }
        if !in_single && !in_double && c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        current.push(c);
        i += 1;
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}
