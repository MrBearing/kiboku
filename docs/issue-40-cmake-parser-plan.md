# Issue #40 CMake parser adapter plan

## Goal

Replace the current ad-hoc `src/parsers/cmake.rs` logic with an adapter over an external Rust CMake parser crate, while preserving valuable behavior-focused tests.

## Why

The PR history for issue #40 shows repeated review-driven fixes around:

- case-insensitive command handling
- comment stripping
- independent `find_package(...)` parsing
- standalone `REQUIRED` detection
- wrapper macro false positives
- keyword filtering in `ament_target_dependencies(...)`
- keyword filtering in `target_link_libraries(...)`
- variable-based target names

Those are useful behavioral requirements, but continuing to implement them through local regex/state logic is likely the wrong maintenance path.

## Proposed direction

- Use `cmake-parser` as the source parser for CMake syntax.
- Keep this repository responsible only for extracting analysis-relevant facts.
- Avoid re-expanding `CMakeInfo` into a clone of the parser crate's model.
- Prefer a thin adapter in `src/parsers/cmake.rs`.

## Data we likely still need in Kiboku

- `find_package(...)`
  - package name
  - optional version
  - whether `REQUIRED` is present as a standalone token
- whether `catkin_package()` exists
- whether `ament_package()` exists
- target declarations
  - executable names
  - library names
- target dependency/link relations
  - `ament_target_dependencies(target ...)`
  - `target_link_libraries(target ...)`

## Design shape

Possible split:

1. Parse full CMake doc with external crate.
2. Walk parser commands.
3. Extract only Kiboku-facing facts.
4. Store them in a minimal analysis struct.

If needed, `CMakeInfo` can remain a compact DTO for downstream code, but should not try to mirror the full parser AST.

## Test policy

Treat existing issue #40 tests as behavior/spec tests, not implementation tests.

Keep (or adapt) tests for:

- multiple `find_package(...)` calls
- uppercase / mixed-case command handling
- comment handling
- `REQUIRED` token semantics
- wrapper macro exclusion
- variable target names
- keyword filtering in dependency/link extraction

Potentially relax tests that overfit the old homemade parser structure rather than Kiboku's real analysis needs.

## Immediate next steps

1. Add the parser crate dependency.
2. Build a tiny spike that parses a synthetic CMakeLists snippet.
3. Inspect crate model / API shape in local build output.
4. Implement adapter extraction for the smallest useful subset.
5. Reconcile existing tests with the new adapter.
