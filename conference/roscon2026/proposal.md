# Talk summary
Kiboku is a Rust-based CLI tool that statically analyzes ROS 1 / ROS 2 workspaces without requiring a ROS environment, extracting and visualizing package dependencies, external dependencies, and rule-based findings. This talk presents the design of the analysis pipeline and rule engine, the static HTML report generation mechanism, and practical usage for understanding workspace structure.

# Talk outline
Total: 15 minutes + 5 minutes Q&A

## 1. Motivation and Problem Statement (3 min)

How ROS workspaces grow in complexity over time
Limitations of relying on grep / colcon list / rospack for dependency understanding
The gap: no ROS-environment-independent, workspace-wide static analysis tool exists

## 2. Kiboku Overview and Architecture (4 min)

Tool overview: CLI binary bok, Rust implementation
Core components:
Workspace scanner
package.xml parser
TOML-based rule engine (built-in + custom rules)
Report generator (JSON → HTML)
Platform flag: catkin vs. ament workspace support
Key design decision: zero dependency on ROS runtime

## 3. Report Walkthrough (5 min)

Running bok against a real workspace (MoveIt)
CLI output: package list, dependency summary, findings
HTML report sections:
Package summary table
Intra-workspace dependency graph
External dependency graph
Findings list and findings matrix
Customizing report layout via TOML config

## 4. Limitations and Future Directions (3 min)

CMakeLists.txt analysis (not yet implemented)
Node/topic extraction from C++/Python source
Semantic analysis possibilities
Community contributions welcome

# Purpose
The goal of this talk is to introduce Kiboku as a practical, immediately usable tool that helps ROS developers gain a clearer understanding of their workspace structure — without requiring a ROS environment to be installed.

## By presenting at ROSCon, I hope to:

Raise awareness that static, file-based analysis is a viable and underexplored approach for ROS workspace comprehension.
Provide the community with a ready-to-use tool (cargo install kiboku) that addresses a real, recurring pain point in medium-to-large ROS projects.
Gather feedback from experienced ROS developers on missing features, rule sets, and integration patterns that would make Kiboku more broadly useful.

## Intended audience

This talk is aimed at:

ROS/ROS 2 developers working on projects with multiple packages who want better visibility into dependency structure and potential design issues.
DevOps and CI engineers maintaining ROS build pipelines who want to enforce workspace-level rules without runtime dependencies.
Tech leads and architects who need to onboard new team members or audit workspace structure during code reviews.
Attendees will leave with:

A concrete understanding of how Kiboku extracts and visualizes dependency information from package.xml without a ROS runtime.
Practical knowledge of how to integrate bok into local workflows and CI pipelines.
Familiarity with the rule engine and how to write custom rules for their own workspace conventions.
The ability to immediately apply the tool to their own workspaces via cargo install kiboku.

# Relevance
As ROS-based systems scale — particularly in production, research, and multi-team settings — understanding the internal structure of a workspace becomes increasingly difficult. Developers frequently ask: "What does this package actually depend on?", "If I change this package, what else breaks?", or "Are there any packages that violate our dependency conventions?" Today, answering these questions typically requires either running ROS tools (which assumes a configured environment) or manual inspection.

This problem is directly relevant to open-source robotics: many open-source ROS projects are contributed to and reviewed by developers who do not have a local ROS environment configured, especially in CI/CD contexts. A static, environment-independent analysis tool lowers the barrier to contribution and review.

Kiboku addresses this gap by treating the workspace as a collection of files to be parsed — not a live ROS environment to be queried. This makes it suitable for CI, code review, documentation, and onboarding contexts where runtime availability cannot be assumed.

The combination of dependency visualization and rule-based findings detection is, to the author's knowledge, not currently available in any existing ROS-ecosystem tool, making this a novel and practical contribution to the community.

# Comparison with existing work
Several existing tools touch on related functionality, but none fully address the same problem space as Kiboku:

- colcon list: enumerates packages and dependencies, but requires a configured colcon environment and produces no visual reports or rule-based analysis.
- rospack depends / rospack depends-on: queries dependency relationships at runtime, but requires a sourced ROS environment and operates on individual packages rather than the whole workspace.
- ament_lint family: performs per-file linting within the ROS build system, but does not analyze workspace-level structural properties.
- ros2 pkg CLI: provides package metadata queries but requires a ROS 2 installation and does not aggregate workspace-wide views.
- Ad-hoc graphviz/Python scripts: exist in various ROS projects as one-off visualizations, but are not generalized, rule-aware, or easily reusable across workspaces.

Kiboku is a new, standalone open-source project. Its distinguishing characteristics are:

Zero ROS runtime dependency — works on any machine with the binary installed.
Integrated rule engine with TOML-based configuration, enabling both built-in and project-specific checks.
Self-contained static HTML report output, shareable without any server or viewer dependency.
Rust implementation enabling fast execution and easy distribution via cargo install.

# Software/hardware information
1. Name: Kiboku (CLI binary: bok)

2. Repository and License:

GitHub: https://github.com/MrBearing/kiboku
License: MIT
The tool is fully open-source and publicly available. Live examples of generated HTML reports targeting the MoveIt workspace are published at:

ROS 2: https://mrbearing.github.io/kiboku/ros2/
ROS 1: https://mrbearing.github.io/kiboku/ros1/
Installable today via: cargo install kiboku

# Desired talk length: Short (default)

Links
https://github.com/MrBearing/kiboku

Profile link
https://github.com/MrBearing