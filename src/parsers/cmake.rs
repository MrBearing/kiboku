use crate::models::cmake::{CMakeInfo, FindPackage, TargetDependencies, TargetLinks};
use anyhow::Result;
use cmake_parser::{parse_cmakelists, Command, Doc, RosCommand, Token};
use std::fs;

pub fn parse_cmake_lists(path: &str) -> Result<CMakeInfo> {
    let text = fs::read_to_string(path)?;
    let mut info = CMakeInfo::default();

    let parse_text = if text.ends_with("\n") { text.clone() } else { format!("{text}\n") };
    let tokens = parse_cmakelists(parse_text.as_bytes())?;
    let doc = Doc::from(tokens);

    parse_standard_commands(&doc, &mut info)?;
    normalize_find_package_required_flags(&doc, &mut info);
    parse_ros_and_raw_commands(&doc, &mut info);

    Ok(info)
}

fn parse_standard_commands(doc: &Doc<'_>, info: &mut CMakeInfo) -> Result<()> {
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

fn normalize_find_package_required_flags(doc: &Doc<'_>, info: &mut CMakeInfo) {
    let required_by_index = doc
        .raw_commands()
        .filter(|raw| eq_ignore_ascii_case(raw.identifier.as_ref(), b"find_package"))
        .filter(|raw| !raw.tokens.is_empty())
        .map(|raw| {
            raw.tokens
                .iter()
                .skip(1)
                .any(|t| eq_ignore_ascii_case(t.as_ref(), b"REQUIRED"))
        });

    for (pkg, required) in info.find_packages.iter_mut().zip(required_by_index) {
        pkg.required = required;
    }
}

fn parse_ros_and_raw_commands(doc: &Doc<'_>, info: &mut CMakeInfo) {
    for cmd in doc.ros_commands() {
        match cmd {
            RosCommand::AmentPackage => info.has_ament_package = true,
            RosCommand::CatkinPackage => info.has_catkin_package = true,
            RosCommand::AmentTargetDependencies(dep) => {
                info.ament_target_dependencies.push(TargetDependencies {
                    target: dep.target.to_string(),
                    dependencies: dep.dependencies.iter().map(ToString::to_string).collect(),
                });
            }
        }
    }

    for raw in doc.raw_commands() {
        if eq_ignore_ascii_case(raw.identifier.as_ref(), b"target_link_libraries") {
            if let Some((target, rest)) = raw.tokens.split_first() {
                let libraries = rest
                    .iter()
                    .filter(|token| !is_target_link_keyword(token.as_ref()))
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                info.target_link_libraries.push(TargetLinks {
                    target: target.to_string(),
                    libraries,
                });
            }
        }
    }
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

fn is_target_link_keyword(token: &[u8]) -> bool {
    matches!(
        ascii_upper(token).as_str(),
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

fn eq_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn ascii_upper(token: &[u8]) -> String {
    String::from_utf8_lossy(token).to_ascii_uppercase()
}
