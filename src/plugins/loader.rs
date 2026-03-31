use crate::plugins::rule::{Rule, RulesFile};
use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::fs;
use std::path::{Path, PathBuf};

static BUILTIN_RULES_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/builtin-rules");

fn load_file(p: &Path) -> Result<Vec<Rule>> {
    let txt =
        fs::read_to_string(p).with_context(|| format!("reading rule file {}", p.display()))?;
    let rf: RulesFile =
        toml::from_str(&txt).with_context(|| format!("parsing toml {}", p.display()))?;
    Ok(rf.rules.unwrap_or_default())
}

fn load_rules_from_toml_str(toml_str: &str, source_name: &str) -> Result<Vec<Rule>> {
    let rf: RulesFile = toml::from_str(toml_str)
        .with_context(|| format!("parsing builtin rules {}", source_name))?;
    Ok(rf.rules.unwrap_or_default())
}

pub fn load_rules_from_path(
    path_opt: Option<PathBuf>,
    platform_opt: Option<String>,
    include_builtin: bool,
) -> Result<Vec<Rule>> {
    let mut rules: Vec<Rule> = Vec::new();

    // 1. built-in (loaded first, user rules add on top)
    if include_builtin {
        rules.append(&mut built_in_rules(platform_opt.as_deref())?);
    }

    // 2. user-specified
    if let Some(p) = path_opt {
        let p = p.as_path();
        if p.is_file() {
            rules.append(&mut load_file(p)?);
        } else if p.is_dir() {
            for entry in fs::read_dir(p)? {
                let e = entry?;
                let path = e.path();
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    rules.append(&mut load_file(&path)?);
                }
            }
        }
    }

    // 3. config dir
    if let Some(mut dir) = dirs_next::config_dir() {
        dir.push("kiboku");
        dir.push("rules");
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let e = entry?;
                let path = e.path();
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    rules.append(&mut load_file(&path)?);
                }
            }
        }
    }

    Ok(rules)
}

fn built_in_rules(platform: Option<&str>) -> Result<Vec<Rule>> {
    let mut rules: Vec<Rule> = Vec::new();

    // Load platform-specific builtin rules.
    // IMPORTANT: must work regardless of the current working directory (e.g. CI, GitHub Actions).
    // Also: do not hardcode filenames; load all *.toml under builtin-rules/<platform>/.
    if let Some(p) = platform {
        if let Some(platform_dir) = BUILTIN_RULES_DIR.get_dir(p) {
            let mut toml_files = platform_dir
                .files()
                .filter(|f| f.path().extension().and_then(|s| s.to_str()) == Some("toml"))
                .collect::<Vec<_>>();

            // Keep deterministic ordering.
            toml_files.sort_by(|a, b| a.path().cmp(b.path()));

            for f in toml_files {
                let source_name = format!(
                    "builtin-rules/{}/{}",
                    p,
                    f.path()
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("<unknown>")
                );
                let txt = f
                    .contents_utf8()
                    .with_context(|| format!("reading builtin rules {}", source_name))?;
                rules.append(&mut load_rules_from_toml_str(txt, &source_name)?);
            }
        }
    }

    // Backwards-compatibility: some tests / consumers expect legacy ids.
    // Ensure a legacy id `ros1-dep-roscpp` exists if a roscpp dependency rule is present.
    let mut extra: Vec<Rule> = Vec::new();
    for r in &rules {
        if r.id == "roscpp-dep" {
            let mut nr = r.clone();
            nr.id = "ros1-dep-roscpp".to_string();
            extra.push(nr);
        }
    }
    rules.append(&mut extra);
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_rules_from_toml_str_parses_valid_toml() {
        let toml_str = r#"
[meta]
name = "test"
version = "0.1"

[[rules]]
id = "test-rule"
target = "cpp"

[rules.match]
type = "regex"
pattern = "foo"

[rules.output]
message = "found foo"
"#;

        let rules = load_rules_from_toml_str(toml_str, "inline").expect("should parse");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "test-rule");
        assert_eq!(rules[0].match_rule.kind, "regex");
        assert_eq!(rules[0].match_rule.pattern, "foo");
        assert_eq!(rules[0].output.message, "found foo");
    }

    #[test]
    fn load_rules_from_toml_str_rejects_invalid_toml() {
        let err = load_rules_from_toml_str("this is not toml", "inline").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.to_lowercase().contains("parsing"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn built_in_rules_ros1_are_loaded_from_embedded_dir() {
        let rules = built_in_rules(Some("ros1")).expect("builtin ros1 rules should load");
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.id == "ros1-header-ros"));
        // legacy id should be injected for compatibility
        assert!(rules.iter().any(|r| r.id == "ros1-dep-roscpp"));
    }

    #[test]
    fn built_in_rules_none_is_empty() {
        let rules = built_in_rules(None).expect("builtin rules should load");
        assert!(rules.is_empty());
    }
}
