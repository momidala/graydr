# Changelog

All notable changes to graydr are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
graydr uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] — 2026-03-19

First public release.

### Compiler

- Full typed AST for `.gmod`, `.gtpl`, and `.gfrag` file types with
  span-preserving parser and per-node source positions
- Four-source variable resolution chain: module defaults → template
  parameters → properties files (YAML/JSON) → `-D` flags, with deep merge
  and missing=fail enforcement
- `evalexpr`-driven module validation rules (`.grule`) with configurable
  severity (error / warning / info)
- Compile-time case arm dispatch on one or more variables, with
  cross-resource output reference injection and case completeness warnings
- `petgraph`-based dependency graph with topological sort, cycle detection,
  and per-provider-per-region assembly grouping
- Tera rendering pipeline with IaC-native `${}` passthrough and
  collect-all validation
- `.gfrag` fragment inliner with cycle detection and byte-range source map
  for error attribution; supports diamond-shaped include graphs
- Governance metadata block emitted as structured comments in compiled output
- Six-command CLI: `compile`, `validate`, `init module`, `init template`,
  `version`, `publish`
- Multi-file `--properties` merge (later files take precedence)
- `--include-path` flag is repeatable — multiple module directories without
  symlink workarounds

### Formatter and Linter

- `graydr fmt`: opinionated formatter for `.gmod`, `.gtpl`, `.gfrag`, and
  `.grule` files with `--check` mode for CI
- `graydr lint`: standalone linter surfacing rule violations, unused
  variables, and cross-module type mismatches

### Registry

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

### Language Server

- LSP server (`graydr lsp`) with stdio transport for editor integration
- Parse and lint diagnostics published on file open and change
- Completion, hover, and go-to-definition for module properties and
  template parameters

### VSCode Extension

- Syntax highlighting for all four file types (`.gmod`, `.gtpl`, `.gfrag`,
  `.grule`) via TextMate grammars
- LSP client integration — diagnostics, completions, and hover in the editor

### Reference Modules

- 10 production-ready reference modules with AWS, Azure, and GCP arms:
  `network`, `relational_db`, `object_storage`, `cache`, `load_balancer`,
  `dns`, `container_registry`, `kubernetes`, `secret_manager`, `queue`
- Module style guide covering naming conventions, multi-cloud interface
  design, governance metadata standards, fragment patterns, and a
  new-provider-arm walkthrough

### Documentation

- Language specification, authoring guides (modules, templates, fragments),
  CLI reference, cross-resource reference guide, and governance metadata guide
- End-to-end example stack (`examples/web-app-stack.gtpl`) composing four
  reference modules across three clouds
