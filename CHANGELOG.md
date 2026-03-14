# Changelog

All notable changes to graydr are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
graydr uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.2.0] — 2026-03-14

### Added
- 10 production-ready reference modules with AWS, Azure, and GCP arms:
  `network`, `relational_db`, `object_storage`, `cache`, `load_balancer`,
  `dns`, `container_registry`, `kubernetes`, `secret_manager`, `queue`
- Module style guide (`docs/module-style-guide.md`) covering naming
  conventions, multi-cloud interface design, governance metadata standards,
  fragment patterns, and a new-provider-arm walkthrough
- End-to-end example stack (`examples/web-app-stack.gtpl`) composing four
  reference modules with cross-resource wiring, three cloud properties files,
  and a 5-minute README walkthrough

### Fixed
- `--include-path` flag is now repeatable — multiple module directories can
  be specified without symlink workarounds
- Literal input bindings (including cross-resource `${net.vpc_id}` wiring)
  now correctly create bare-name aliases in the resolve context, fixing
  variable resolution failures in composed stacks
- CloudFormation YAML (AWS arms) and Bicep (Azure arms) in all reference
  modules updated to production-correct, deployable-quality syntax

---

## [1.1.0] — 2026-03-10

### Added
- `graydr-registry`: self-hostable module registry server (`axum` 0.8,
  filesystem-backed store, atomic writes)
- `graydr publish`: publish `.gmod` files to a registry by
  `org/name@version` coordinate
- Registry-aware compilation: `include "org/name@version"` coordinates in
  `.gfrag` files are resolved via HTTP at compile time
- Lifecycle management API: `PATCH .../lifecycle` with enforced
  `active → deprecated → retired` state machine (retired is terminal)
- `GET .../content` returns `410 Gone` for retired modules; `graydr`
  client maps 410 to a hard `RetiredModule` error blocking compilation
- `GET .../versions` returns SemVer-sorted version list for operator
  inspection

### Changed
- `graydr` and `graydr-registry` are now separate crates in a Cargo
  workspace — the registry server has no dependency on the compiler

---

## [1.0.0] — 2026-03-08

### Added
- Full typed AST for `.gmod`, `.gtpl`, and `.gfrag` file types with
  span-preserving parser and per-node source positions
- Four-source variable resolution chain: module defaults → template
  parameters → properties files (YAML/JSON) → `-D` flags, with deep merge
  and missing=fail enforcement
- `evalexpr`-driven module validation rules with configurable severity
  (error / warning / info)
- Compile-time case arm dispatch on one or more variables, with
  cross-resource output reference injection and case completeness warnings
- `petgraph`-based dependency graph with topological sort, cycle detection,
  and per-provider-per-region assembly grouping
- Tera rendering pipeline with IaC-native `${}` passthrough and
  collect-all validation
- Governance metadata block emitted as structured comments in compiled
  output
- `.gfrag` fragment inliner with cycle detection and byte-range source map
  for error attribution; supports diamond-shaped include graphs
- Five-command CLI: `compile`, `validate`, `init module`, `init template`,
  `version`
- Multi-file `--properties` merge (later files take precedence)
- Full documentation suite: language spec, three authoring guides, CLI
  reference, cross-resource reference guide, and v1→v2 migration guide
- Community registry client: `org/name@version` SemVer coordinates,
  `graydr publish`, lifecycle state enforcement
