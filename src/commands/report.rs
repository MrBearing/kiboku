use std::collections::{BTreeMap, BTreeSet};

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::models::AnalysisReport;

#[derive(Args, Debug, Clone)]
pub struct ReportArgs {
    /// Input analysis JSON file (produced by `bok analyze --format json`)
    pub input: PathBuf,

    /// Output path.
    ///
    /// - If it ends with `.html`, writes a single HTML file.
    /// - Otherwise, writes a directory bundle: `index.html` + `assets/`.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Report configuration TOML.
    ///
    /// Currently supports customizing sidebar section ordering/visibility.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

pub fn run(args: ReportArgs) -> Result<()> {
    let bytes = fs::read(&args.input)
        .with_context(|| format!("failed to read input: {}", args.input.display()))?;

    let report: AnalysisReport = serde_json::from_slice(&bytes)
        .with_context(|| "failed to parse analysis JSON".to_string())?;

    match args.output {
        None => {
            // legacy: write a single HTML to stdout
            let html = render_html(&report);
            print!("{}", html);
        }
        Some(out) => {
            if looks_like_html_file(&out) {
                let html = render_html(&report);
                fs::write(&out, html)
                    .with_context(|| format!("failed to write output: {}", out.display()))?;
            } else {
                let cfg = load_report_config(args.config.as_deref())?;
                write_results_bundle(&out, &report, &cfg)?;
            }
        }
    }

    Ok(())
}

fn looks_like_html_file(p: &Path) -> bool {
    if p.exists() {
        return p.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("html"));
    }
    p.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html"))
}

fn write_results_bundle(
    out_dir: &Path,
    report: &AnalysisReport,
    config: &ReportConfig,
) -> Result<()> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output dir: {}", out_dir.display()))?;

    let assets_dir = out_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("failed to create assets dir: {}", assets_dir.display()))?;

    // Embedded templates / vendor
    const INDEX_HTML: &str = include_str!("../../assets/report/index.html");
    const STYLE_CSS: &str = include_str!("../../assets/report/style.css");
    const APP_JS: &str = include_str!("../../assets/report/app.js");
    const CYTO_JS: &[u8] = include_bytes!("../../assets/vendor/cytoscape.min.js");
    const CYTO_LICENSE: &str = include_str!("../../assets/vendor/cytoscape.LICENSE");

    const AG_GRID_JS: &[u8] = include_bytes!("../../assets/vendor/ag-grid-community.min.js");
    const AG_GRID_CSS: &str = include_str!("../../assets/vendor/ag-grid.css");
    const AG_GRID_THEME_CSS: &str = include_str!("../../assets/vendor/ag-theme-alpine.css");
    const AG_GRID_LICENSE: &str = include_str!("../../assets/vendor/ag-grid.LICENSE");

    let graph = build_graph_json(report);
    let graph_json = serde_json::to_string_pretty(&graph)?;
    let embedded_graph_json = escape_json_for_script_tag(&graph_json);

    let report_data = build_report_data(report, config)?;
    let report_data_json = serde_json::to_string_pretty(&report_data)?;
    let embedded_report_data_json = escape_json_for_script_tag(&report_data_json);

    let report_config_json = serde_json::to_string_pretty(config)?;
    let embedded_report_config_json = escape_json_for_script_tag(&report_config_json);

    let index_html = INDEX_HTML
        .replace("__GRAPH_JSON__", &embedded_graph_json)
        .replace("__REPORT_DATA_JSON__", &embedded_report_data_json)
        .replace("__REPORT_CONFIG_JSON__", &embedded_report_config_json);

    fs::write(out_dir.join("index.html"), index_html)
        .with_context(|| format!("failed to write {}", out_dir.join("index.html").display()))?;
    fs::write(assets_dir.join("style.css"), STYLE_CSS)
        .with_context(|| format!("failed to write {}", assets_dir.join("style.css").display()))?;
    fs::write(assets_dir.join("app.js"), APP_JS)
        .with_context(|| format!("failed to write {}", assets_dir.join("app.js").display()))?;
    fs::write(assets_dir.join("cytoscape.min.js"), CYTO_JS).with_context(|| {
        format!(
            "failed to write {}",
            assets_dir.join("cytoscape.min.js").display()
        )
    })?;

    fs::write(assets_dir.join("ag-grid-community.min.js"), AG_GRID_JS).with_context(|| {
        format!(
            "failed to write {}",
            assets_dir.join("ag-grid-community.min.js").display()
        )
    })?;
    fs::write(assets_dir.join("ag-grid.css"), AG_GRID_CSS).with_context(|| {
        format!(
            "failed to write {}",
            assets_dir.join("ag-grid.css").display()
        )
    })?;
    fs::write(assets_dir.join("ag-theme-alpine.css"), AG_GRID_THEME_CSS).with_context(|| {
        format!(
            "failed to write {}",
            assets_dir.join("ag-theme-alpine.css").display()
        )
    })?;

    fs::write(assets_dir.join("graph.json"), format!("{}\n", graph_json)).with_context(|| {
        format!(
            "failed to write {}",
            assets_dir.join("graph.json").display()
        )
    })?;

    let notices = format!(
        "This report bundle includes third-party software.\n\n- cytoscape.js v3.30.2\n\n{}\n\n- AG Grid Community v32.3.4\n\n{}\n",
        CYTO_LICENSE, AG_GRID_LICENSE
    );
    fs::write(out_dir.join("THIRD_PARTY_NOTICES.txt"), notices).with_context(|| {
        format!(
            "failed to write {}",
            out_dir.join("THIRD_PARTY_NOTICES.txt").display()
        )
    })?;

    Ok(())
}

fn escape_json_for_script_tag(json: &str) -> String {
    // Prevent prematurely closing the <script> tag.
    // This keeps the embedded JSON safe even if it contains "</script>".
    json.replace("</script>", "<\\/script>")
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ReportConfigToml {
    title: Option<String>,
    sections: Option<Vec<String>>,
    hidden: Option<Vec<String>>,
    external_repos: Option<BTreeMap<String, String>>,
    section_heights: Option<BTreeMap<String, toml::Value>>,
}

#[derive(Debug, Clone, Serialize)]
struct ReportConfig {
    title: String,
    sections: Vec<String>,
    hidden: Vec<String>,
    external_repos: BTreeMap<String, String>,
    section_heights: BTreeMap<String, String>,
}

fn is_probably_css_length(value: &str) -> bool {
    let s = value.trim();
    if s.is_empty() {
        return false;
    }

    // Accept common functional length expressions.
    // (We don't fully parse CSS; this is only best-effort validation.)
    let lower = s.to_ascii_lowercase();
    for p in ["calc(", "clamp(", "min(", "max(", "var("] {
        if lower.starts_with(p) {
            return true;
        }
    }

    // A unitless 0 is valid for lengths in CSS.
    if let Ok(n) = s.parse::<f64>() {
        if n == 0.0 {
            return true;
        }
    }

    // A small allowlist of commonly-used CSS length units.
    // Note: this is intentionally conservative; we warn (but still accept) unknown values.
    const UNITS: [&str; 18] = [
        "px", "vh", "vw", "vmin", "vmax", "dvh", "lvh", "svh", "%", "em", "rem", "ch", "ex", "cm",
        "mm", "in", "pt", "pc",
    ];

    // Percent is special because it's a single-character unit.
    if let Some(num) = s.strip_suffix('%') {
        return num.trim().parse::<f64>().is_ok();
    }

    for unit in UNITS {
        if unit == "%" {
            continue;
        }
        if let Some(num) = s.strip_suffix(unit) {
            return num.trim().parse::<f64>().is_ok();
        }
    }

    false
}

fn load_report_config(config_path: Option<&Path>) -> Result<ReportConfig> {
    let defaults = ReportConfig {
        title: "Kiboku Report".to_string(),
        sections: vec![
            "package_summary".to_string(),
            "workspace_dependencies".to_string(),
            "external_dependencies".to_string(),
            "findings".to_string(),
            "findings_matrix".to_string(),
            "external_libraries".to_string(),
        ],
        hidden: Vec::new(),
        external_repos: BTreeMap::new(),
        section_heights: BTreeMap::new(),
    };

    let Some(path) = config_path else {
        return Ok(defaults);
    };

    let txt = fs::read_to_string(path)
        .with_context(|| format!("failed to read config: {}", path.display()))?;

    let cfg: ReportConfigToml = toml::from_str(&txt)
        .with_context(|| format!("failed to parse config TOML: {}", path.display()))?;

    let mut section_heights: BTreeMap<String, String> = BTreeMap::new();
    if let Some(raw) = cfg.section_heights {
        for (k, v) in raw {
            let s = match v {
                toml::Value::String(x) => x,
                toml::Value::Integer(n) => format!("{}px", n),
                // Treat numeric values as pixels. For floats, round to an integer pixel value
                // to avoid scientific notation (and keep output predictable in CSS).
                toml::Value::Float(f) => {
                    if !f.is_finite() {
                        eprintln!(
                            "warning: section_heights entry for '{}' in config '{}' is not finite and will be ignored",
                            k,
                            path.display()
                        );
                        continue;
                    }

                    let rounded = f.round();
                    if rounded < (i64::MIN as f64) || rounded > (i64::MAX as f64) {
                        eprintln!(
                            "warning: section_heights entry for '{}' in config '{}' is out of range and will be ignored",
                            k,
                            path.display()
                        );
                        continue;
                    }

                    let px = rounded as i64;
                    format!("{}px", px)
                }
                // Intentionally ignore unsupported TOML types (bool/array/table/datetime/...)
                // to keep the config forgiving and forward-compatible.
                other => {
                    eprintln!(
                        "warning: section_heights entry for '{}' in config '{}' has unsupported type '{}' and will be ignored",
                        k,
                        path.display(),
                        other.type_str()
                    );
                    continue;
                }
            };

            let trimmed = s.trim();
            if trimmed.is_empty() {
                eprintln!(
                    "warning: section_heights entry for '{}' in config '{}' is empty after trimming and will be ignored",
                    k,
                    path.display()
                );
                continue;
            }

            if !is_probably_css_length(trimmed) {
                eprintln!(
                    "warning: section_heights entry for '{}' in config '{}' looks like an invalid CSS length: '{}' (examples: '700px', '60vh')",
                    k,
                    path.display(),
                    trimmed
                );
            }

            section_heights.insert(k, trimmed.to_string());
        }
    }

    Ok(ReportConfig {
        title: cfg.title.unwrap_or(defaults.title),
        sections: cfg.sections.unwrap_or(defaults.sections),
        hidden: cfg.hidden.unwrap_or_default(),
        external_repos: cfg.external_repos.unwrap_or_default(),
        section_heights,
    })
}

#[derive(Debug, Clone, Serialize)]
struct ReportData {
    meta: MetaData,
    package_summary: PackageSummaryData,
    external_libraries: ExternalLibrariesData,
    findings: FindingsData,
}

#[derive(Debug, Clone, Serialize)]
struct MetaData {
    title: String,
    generated_at_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
struct PackageSummaryData {
    package_count: usize,
    cpp_files: usize,
    python_files: usize,
    launch_files: usize,
    urdf_files: usize,
    xacro_files: usize,
    mesh_files: usize,
}

#[derive(Debug, Clone, Serialize)]
struct ExternalLibrariesData {
    total_unique: usize,
    items: Vec<ExternalLibraryItem>,
}

#[derive(Debug, Clone, Serialize)]
struct ExternalLibraryItem {
    name: String,
    usage_count: usize,
    repository: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FindingsData {
    total: usize,
    rules: Vec<String>,
    packages: Vec<String>,
    items: Vec<FindingItem>,
    counts: Vec<FindingCount>,
}

#[derive(Debug, Clone, Serialize)]
struct FindingItem {
    package: String,
    rule_id: String,
    severity: String,
    file: String,
    line: Option<usize>,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct FindingCount {
    package: String,
    rule_id: String,
    count: usize,
}

fn build_report_data(report: &AnalysisReport, config: &ReportConfig) -> Result<ReportData> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let generated_at_ms: u128 = (now.as_secs() as u128) * 1000 + (now.subsec_millis() as u128);

    let workspace_names: BTreeSet<String> =
        report.packages.iter().map(|p| p.name.clone()).collect();

    let mut cpp_files = 0usize;
    let mut python_files = 0usize;
    let mut launch_files = 0usize;
    let mut urdf_files = 0usize;
    let mut xacro_files = 0usize;
    let mut mesh_files = 0usize;

    for p in &report.packages {
        for e in WalkDir::new(&p.path).into_iter().filter_map(|r| r.ok()) {
            if !e.file_type().is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_ascii_lowercase();
            let ext = e
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            match ext.as_str() {
                "c" | "cc" | "cpp" | "cxx" | "h" | "hh" | "hpp" | "hxx" | "ipp" => cpp_files += 1,
                "py" => python_files += 1,
                "urdf" => urdf_files += 1,
                "xacro" => xacro_files += 1,
                "stl" | "dae" | "obj" | "ply" => mesh_files += 1,
                _ => {}
            }

            if name.ends_with(".launch")
                || name.ends_with(".launch.xml")
                || name.ends_with(".launch.py")
            {
                launch_files += 1;
            }
        }
    }

    // External library usage counts: number of workspace packages that depend on it (deduped per package).
    let mut ext_counts: BTreeMap<String, usize> = BTreeMap::new();
    for p in &report.packages {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for d in &p.dependencies {
            if workspace_names.contains(&d.name) {
                continue;
            }
            if seen.insert(d.name.clone()) {
                *ext_counts.entry(d.name.clone()).or_insert(0) += 1;
            }
        }
    }
    let mut external_items: Vec<ExternalLibraryItem> = ext_counts
        .into_iter()
        .map(|(name, usage_count)| ExternalLibraryItem {
            repository: config.external_repos.get(&name).cloned(),
            name,
            usage_count,
        })
        .collect();
    external_items.sort_by(|a, b| {
        b.usage_count
            .cmp(&a.usage_count)
            .then_with(|| a.name.cmp(&b.name))
    });

    // Findings: attach package name by path prefix.
    // Sort packages by path length (longest first) to match most specific package.
    let mut sorted_packages = report.packages.clone();
    sorted_packages.sort_by(|a, b| b.path.len().cmp(&a.path.len()));

    let mut findings_items: Vec<FindingItem> = Vec::new();
    let mut rule_set: BTreeSet<String> = BTreeSet::new();
    for f in &report.findings {
        let mut pkg = "(unknown)".to_string();
        let file_path = Path::new(&f.file);

        // Find the longest matching package path using proper path prefix matching
        for p in &sorted_packages {
            let pkg_path = Path::new(&p.path);
            // Use strip_prefix which properly handles path separators
            if file_path.strip_prefix(pkg_path).is_ok() {
                pkg = p.name.clone();
                break;
            }
        }

        rule_set.insert(f.rule_id.clone());
        findings_items.push(FindingItem {
            package: pkg,
            rule_id: f.rule_id.clone(),
            severity: f.severity.clone(),
            file: f.file.clone(),
            line: f.line,
            message: f.message.clone(),
        });
    }

    let mut rules: Vec<String> = rule_set.into_iter().collect();
    rules.sort();

    let mut packages: Vec<String> = report.packages.iter().map(|p| p.name.clone()).collect();
    packages.sort();

    let mut counts_map: BTreeMap<(String, String), usize> = BTreeMap::new();
    for it in &findings_items {
        *counts_map
            .entry((it.package.clone(), it.rule_id.clone()))
            .or_insert(0) += 1;
    }
    let counts: Vec<FindingCount> = counts_map
        .into_iter()
        .map(|((package, rule_id), count)| FindingCount {
            package,
            rule_id,
            count,
        })
        .collect();

    Ok(ReportData {
        meta: MetaData {
            title: config.title.clone(),
            generated_at_ms,
        },
        package_summary: PackageSummaryData {
            package_count: report.packages.len(),
            cpp_files,
            python_files,
            launch_files,
            urdf_files,
            xacro_files,
            mesh_files,
        },
        external_libraries: ExternalLibrariesData {
            total_unique: external_items.len(),
            items: external_items,
        },
        findings: FindingsData {
            total: report.findings.len(),
            rules,
            packages,
            items: findings_items,
            counts,
        },
    })
}

#[derive(Debug, Clone, Serialize)]
struct GraphJson {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
struct GraphNode {
    id: String,
    label: String,
    kind: String,
    has_findings: bool,
}

#[derive(Debug, Clone, Serialize)]
struct GraphEdge {
    source: String,
    target: String,
    dep_type: String,
}

fn build_graph_json(report: &AnalysisReport) -> GraphJson {
    // Use stable ordering for deterministic output.
    let mut nodes: BTreeMap<String, GraphNode> = BTreeMap::new();

    let workspace_names: BTreeSet<String> =
        report.packages.iter().map(|p| p.name.clone()).collect();

    // Precompute which packages have findings by path prefix.
    let mut has_findings: BTreeMap<String, bool> = BTreeMap::new();
    for p in &report.packages {
        let pfx = p.path.clone();
        let found = report.findings.iter().any(|f| f.file.starts_with(&pfx));
        has_findings.insert(p.name.clone(), found);
    }

    for p in &report.packages {
        nodes.insert(
            p.name.clone(),
            GraphNode {
                id: p.name.clone(),
                label: p.name.clone(),
                kind: "workspace".to_string(),
                has_findings: has_findings.get(&p.name).copied().unwrap_or(false),
            },
        );
    }

    let mut edges: Vec<GraphEdge> = Vec::new();
    for p in &report.packages {
        for d in &p.dependencies {
            let target = d.name.clone();
            let dep_type = d.kind.as_deref().unwrap_or("build").to_string();

            if !workspace_names.contains(&target) {
                nodes.entry(target.clone()).or_insert(GraphNode {
                    id: target.clone(),
                    label: target.clone(),
                    kind: "external".to_string(),
                    has_findings: false,
                });
            }

            edges.push(GraphEdge {
                source: p.name.clone(),
                target,
                dep_type,
            });
        }
    }

    edges.sort_by(|a, b| {
        (a.source.as_str(), a.target.as_str(), a.dep_type.as_str()).cmp(&(
            b.source.as_str(),
            b.target.as_str(),
            b.dep_type.as_str(),
        ))
    });

    GraphJson {
        nodes: nodes.into_values().collect(),
        edges,
    }
}

fn render_html(report: &AnalysisReport) -> String {
    let total_packages = report.summary.get("total_packages").copied().unwrap_or(0);
    let total_findings = report
        .summary
        .get("total_findings")
        .copied()
        .unwrap_or(report.findings.len());

    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>Kiboku Report</title>\n");
    out.push_str("</head>\n<body>\n");

    out.push_str("<h1>Kiboku Report</h1>\n");
    out.push_str("<ul>\n");
    out.push_str(&format!("<li>Total packages: {}</li>\n", total_packages));
    out.push_str(&format!("<li>Total findings: {}</li>\n", total_findings));
    out.push_str("</ul>\n");

    out.push_str("<h2>Findings</h2>\n");
    out.push_str("<table border=\"1\" cellspacing=\"0\" cellpadding=\"6\">\n");
    out.push_str("<caption>Findings list</caption>\n");
    out.push_str("<thead><tr><th>Severity</th><th>Rule</th><th>File</th><th>Line</th><th>Message</th><th>Suggestion</th></tr></thead>\n");
    out.push_str("<tbody>\n");

    for f in &report.findings {
        out.push_str("<tr>");
        out.push_str(&format!("<td>{}</td>", escape_html(&f.severity)));
        out.push_str(&format!("<td>{}</td>", escape_html(&f.rule_id)));
        out.push_str(&format!("<td>{}</td>", escape_html(&f.file)));
        out.push_str(&format!(
            "<td>{}</td>",
            f.line.map(|v| v.to_string()).unwrap_or_default()
        ));
        out.push_str(&format!("<td>{}</td>", escape_html(&f.message)));
        out.push_str(&format!(
            "<td>{}</td>",
            f.suggestion.as_deref().map(escape_html).unwrap_or_default()
        ));
        out.push_str("</tr>\n");
    }

    out.push_str("</tbody></table>\n");
    out.push_str("</body>\n</html>\n");
    out
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
