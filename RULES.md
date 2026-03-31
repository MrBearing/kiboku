# Kiboku Rules Specification

This document describes the TOML rule format used by Kiboku and the meaning of the `severity` field.

## Overview

Rules are declared in TOML files. Each file may contain multiple `[[rules]]` entries. The loader reads rules from:

- The path provided with `bok analyze --rules` (file or directory),
- `~/.config/kiboku/rules/*.toml`,
- Bundled built-in rules.

Rules are intended to be flexible: some rules only *discover* occurrences (investigation), while others may indicate actionable migration concerns.

## Rule file structure (example)

```toml
[meta]
name = "ros1_to_ros2"
version = "1.0.0"

[[rules]]
id = "ros1-header-ros"
name = "ROS1 core header (discovery)"
description = "Detect use of ROS1 core header includes for investigation purposes"
severity = "info"
category = "discovery"
target = "cpp"

[rules.match]
type = "include"
pattern = "ros/ros.h"

[rules.output]
message = "found include: ros/ros.h (investigation)"
```

## Fields

- `id` (string): unique identifier for the rule.
- `name` (string): short human-friendly name.
- `description` (string): longer explanation of intent and scope.
- `severity` (string): how findings are classified; see below.
- `category` (string): optional grouping (e.g., `migration`, `discovery`, `style`).
- `target` (string): target language / domain (e.g., `cpp`, `cmake`, `package`).
- `match`: matching criteria (e.g., `type = "include"`, `pattern = "ros/ros.h"`, `type = "regex"`).
- `output`: message, suggestion, effort estimation (optional).

## `severity` semantics

`severity` determines how findings from a rule are presented and acted upon. Use these meanings:

- `info`
  - Purpose: Discovery or informational findings only.
  - Behavior: Record the finding but do not mark it as a migration warning.
  - Use for: Inventorying usage (includes, dependencies), telemetry, or items for human review.

- `warning`
  - Purpose: Indicates something that likely requires developer attention.
  - Behavior: Present as a non-fatal warning; may include suggested remediation.
  - Use for: API usages or patterns that commonly need migration, but where automatic remediation is non-trivial.

- `error`
  - Purpose: Strongly actionable or high-risk items.
  - Behavior: Present as high-severity findings that should be prioritized.
  - Use for: Deprecated APIs with known breakage, unsafe patterns, or hard blockers.

Notes:
- The tool's default reporting may choose to surface `warning` and `error` prominently while keeping `info` as an investigatory list.
- Rules that enumerate findings without prescribing a migration path should generally use `info` (discovery).

## Best practices

- Keep discovery rules (`info`) separate from migration rules (`warning`/`error`) so users can first get an inventory, then enable migration guidance.
- If a rule includes a `suggestion` or `effort_hours` in `output`, consider using `warning` to indicate actionability.
- Avoid emitting `warning` when the rule merely lists occurrences; use `info` instead.

## Examples

- Inventory rule: detection of `ros/ros.h` includes — `severity = "info"`.
- Migration suggestion: recommend `rclcpp` for a known pattern with code examples — `severity = "warning"`.
- Critical error: use of an API guaranteed to fail in ROS 2 builds — `severity = "error"`.

## Compatibility

Existing rules that previously used a single warning level should be reviewed and migrated to the appropriate `severity` value.

The built-in rule sets in `builtin-rules/ros1/` and `builtin-rules/ros2/` are intended to be discovery-first (often `info`). Select them with `-p/--platform`.

If you have questions about a rule conversion, open an issue or PR with the proposed severity mapping.
