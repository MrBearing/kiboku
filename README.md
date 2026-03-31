# Kiboku

Kiboku is a CLI static analysis tool for ROS workspaces.

Usage (for users)
------------------

Install (from crates.io):

```bash
# install the binary to your cargo bin directory
cargo install kiboku
```

Install (from source):

```bash
# install the binary to your cargo bin directory
cargo install --path . --locked
```

After installation use `bok` from your PATH.

Analyze a workspace (text output):

```bash
bok analyze /path/to/ros/workspace
```

Select a platform (load platform-specific builtin rules):

```bash
# analyze with ROS1 builtin rules
bok analyze --platform ros1 /path/to/ros/workspace

# analyze with ROS2 builtin rules
bok analyze --platform ros2 /path/to/ros/workspace
```

JSON output:

```bash
bok analyze -f json /path/to/ros/workspace
```

Custom rules:

```bash
# add custom rules on top of builtin rules
bok analyze --platform ros1 --rules /path/to/custom/rules /path/to/ros/workspace

# load only custom rules (disable builtin)
bok analyze --rules /path/to/custom/rules --no-builtin /path/to/ros/workspace
```

List available rules:

```bash
bok analyze --list-rules

# include builtin rules for a specific platform
bok analyze -p ros1 --list-rules
bok analyze -p ros2 --list-rules
```

Generate a static HTML report from JSON:

```bash
# 1) analyze and write JSON to a file
bok analyze -p ros1 -f json -o report.json /path/to/ros/workspace

# 2) convert JSON to a standalone HTML file
bok report report.json -o report.html

# or: write a directory bundle (index.html + assets/) for richer UI
bok report report.json -o results/
```

Analyze and generate a report in one step:

```bash
# official form
bok run /path/to/ros/workspace -p ros1 -o results/ --config report.toml

# shorthand (equivalent to `bok run`)
bok /path/to/ros/workspace -p ros1 -o results/ --config report.toml
```

`bok run` uses the same `--output/-o` and `--config` option names as `bok report`.

Customize report sections (optional)
-------------------------------

When writing a directory bundle (e.g. `-o results/`), you can customize the sidebar section ordering and visibility via a TOML config:

```bash
bok report report.json -o results/ --config report.toml
```

Example `report.toml`:

```toml
# Report title (optional, default: "Kiboku Report")
title = "My ROS Workspace Analysis"

# Sidebar section order
sections = [
	"package_summary",
	"workspace_dependencies",
	"external_dependencies",
	"findings",
	"findings_matrix",
	"external_libraries",
]

# Hide sections by id
hidden = []

# Optional: per-section panel height override (graph/matrix panels)
# Values are CSS lengths like "700px" or "60vh".
# - String / integer / float are supported (floats are rounded to integer px).
# - Other TOML value types (bool/array/table/datetime/...) are ignored.
# - Strings are best-effort validated; suspicious values may emit a warning.
[section_heights]
workspace_dependencies = "700px"
external_dependencies = "700px"

# Optional: map external library name -> repository URL
[external_repos]
roscpp = "https://github.com/ros/ros_comm"
```

Available section ids:

- `package_summary` — package count + file counts (C++/Python/launch/urdf/xacro/mesh)
- `workspace_dependencies` — workspace package dependencies (matrix/graph toggle)
- `external_dependencies` — external library dependencies (matrix/graph toggle)
- `findings` — findings views (by package / by finding)
- `findings_matrix` — packages × findings count table (sortable)
- `external_libraries` — external library list + repository links (from `external_repos`)

Sample Report on GitHub Pages
------------------------------

This repository automatically generates and publishes a sample analysis report to GitHub Pages on every push to the main branch. The report analyzes the [ros_tutorials](https://github.com/ros/ros_tutorials) repository as a demonstration.

**View the live report:** [https://mrbearing.github.io/kiboku/](https://mrbearing.github.io/kiboku/)

The workflow configuration can be found in `.github/workflows/publish-report.yml`, and the report configurations are in `docs/github_pages_report.toml` and `docs/github_pages_report_ros2.toml`.

Rules
-----

Rules are provided as TOML files. The tool loads rules in this order:
1. **Builtin rules** (from `builtin-rules/<platform>/`) — selected via `--platform ros1|ros2`
2. **Custom rules** (via `--rules <path>`) — added on top of builtin rules
3. **Config directory** (from `~/.config/kiboku/rules/*.toml`) — added on top

See `RULES.md` for the rule file format and the meaning of the `severity` field (info/warning/error).

Development (for contributors)
------------------------------

Build the project:

```bash
cargo build --release
```

Run tests:

```bash
cargo test
```

Project layout
--------------

- `src/` — main source files (parsers, scanner, analyzer, plugins, output)
- `builtin-rules/` — platform-specific builtin rule sets (ros1/, ros2/)
