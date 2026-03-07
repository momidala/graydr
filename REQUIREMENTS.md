# graydr Requirements
**Version:** 2.0
**Date:** 2026-03-06
**Status:** Approved — supersedes DESIGN_REVIEW_SUMMARY.md, PROJECT_STATUS.md, and scattered requirements across spec files

---

## Table of Contents

1. [Vision](#1-vision)
2. [Actors](#2-actors)
3. [Core Philosophy](#3-core-philosophy)
4. [Language Design](#4-language-design)
5. [Functional Requirements — Community](#5-functional-requirements--community)
6. [Functional Requirements — Enterprise](#6-functional-requirements--enterprise)
7. [Key Design Decisions](#7-key-design-decisions)
8. [Roadmap](#8-roadmap)
9. [Out of Scope](#9-out-of-scope)

---

## 1. Vision

Teams deploying infrastructure to multiple cloud providers write the same logic repeatedly in provider-specific dialects. Abstraction layers that promise to solve this introduce their own DSLs, lag behind provider features, and create new lock-in.

graydr is a **text preprocessor for infrastructure-as-code**. Module authors write real provider code (Terraform, Bicep, CloudFormation, or anything else). graydr assembles, parameterizes, and renders that code from reusable modules and declarative templates. The output is standard IaC that deploys with existing tools and works without graydr.

graydr is **not** a runtime system, an abstraction layer, or a replacement for IaC tools. It runs once at build time and gets out of the way.

---

## 2. Actors

| Actor | Description | Community | Enterprise |
|-------|-------------|-----------|------------|
| Module Author | Writes `.gmod` files — reusable infrastructure components | Yes | Must satisfy org ruleset to publish |
| Template Author | Writes `.gtpl` files — wires modules into deployments | Yes | Same |
| Operator | Runs `graydr compile` with variables | Yes | Same + `--no-local-modules` |
| Platform/Org Team | Sets standards, manages registry, defines governance | Implicit | First-class — defines rulesets, manages lifecycle |
| Security Team | Monitors module vulnerabilities, triggers notifications | None | Manages Security events in registry |
| Approver | Reviews and approves module publications | None | Role in management portal |

---

## 3. Core Philosophy

These principles are non-negotiable and apply to both tiers:

1. **Everything is a variable.** Provider, region, size, environment — all variables. No special-cased concepts in the language runtime.

2. **Explicit over implicit.** Modules receive only what templates explicitly pass. Nothing propagates automatically. Cross-resource wiring is visible in the template.

3. **Missing = fail.** If a variable has no value, fail with a clear error. If a case arm has no match, fail. No fallback, no silent substitution, no "closest match."

4. **Graydr knows nothing about IaC languages.** It renders text. The module author writes real provider code; graydr substitutes variables. It does not parse, validate, or understand Terraform, Bicep, CloudFormation, or any other language.

5. **Combined output.** All resources in a template compile to a single deployable unit per provider per region. The `.gtpl` is the assembly point.

6. **Zero lock-in.** Generated output is standard IaC. It can be committed, edited, and deployed without graydr.

7. **Community is the foundation.** Enterprise adds governance, registry, and contracts on top of the community tool without forking it.

---

## 4. Language Design

### 4.1 File Types

| Extension | Purpose |
|-----------|---------|
| `.gmod` | Module definition — one reusable infrastructure component |
| `.gtpl` | Template definition — assembles modules into a deployable unit |
| `.gfrag` | Module fragment — reusable code snippet included within case arms |
| `.grule` | Ruleset definition — org-level module publishing contract (Enterprise) |
| `.yaml` / `.json` | Properties files — environment-specific variable values |

### 4.2 Variable System

Variables are the only mechanism for parameterization. There are no special-cased primitives (size, region, provider are all just variables).

**Reference syntax:** `$variable_name` to reference a variable's value.

**Structured parameters:** dotted notation for grouped variables.
```
-Dprimary_db.size=XL
-Dprimary_db.name=AppP01
-Danalytics_db.size=M
-Danalytics_db.name=AnDb01
```

**Resolution chain (lowest to highest precedence):**
```
gmod default → gtpl override → yaml/json properties → -D flags
```

A variable with no value at compile time is a hard error.

### 4.3 Case Statement

The primary dispatch mechanism. Any variable can drive a case expression. No reserved variable names.

```hcl
generate {
  case ($provider) {
    "aws": {
      code    = <<- ... ->>
      outputs { endpoint = "aws_rds_cluster.${resource_name}.endpoint" }
    }
    "azure": {
      code    = <<- ... ->>
      outputs { endpoint = "sqlServer.properties.fullyQualifiedDomainName" }
    }
    "ibm": {
      code    = <<- ... ->>
      outputs { endpoint = "..." }
    }
  }
}
```

- Case arms match string literals against variable values.
- No matching arm → fail. No default fallback.
- Cases can be nested or multi-variable: `case ($provider, $engine) { ("aws", "aurora"): { ... } }`
- Any variable can be the switch axis — `$provider`, `$size`, `$engine`, `$tier`, anything.
- `language` is not an attribute. Graydr does not know or care what language the code block contains.

### 4.4 Module Fragments

Reusable code snippets that module authors include within case arms to avoid repeating boilerplate (standard tagging blocks, monitoring config, backup policies, etc.).

```hcl
// In a case arm:
include "momidala/aws-standard-tags"
include "momidala/aws-monitoring"
```

Fragments are versioned and publishable to the registry like modules.

### 4.5 Output Reference Template Strings

The `outputs {}` block inside a case arm maps output names to **reference template strings** — the syntax used by the target IaC language to reference that resource's output at deploy time. `${resource_name}` is substituted with the resource's instance name from the template.

```hcl
outputs {
  vpc_id   = "aws_vpc.${resource_name}.id"               // Terraform
  vpc_id   = "!GetAtt ${resource_name}VPC.VpcId"         // CloudFormation
  vpc_id   = "network.outputs.vpcId"                     // Bicep
}
```

When a template wires `network.vpc_id` to `primary-db.vpc_id`, graydr:
1. Resolves `network.vpc_id` using the network module's matching case arm outputs block
2. Renders the reference template string (substituting `${resource_name}`)
3. Injects the rendered string as `$vpc_id` in the db module's render context
4. The db module uses `{{$vpc_id}}` in its code block

The template author writes `inputs = { vpc_id = network.vpc_id }` — no IaC language knowledge required. Language-specific reference syntax lives in the module where it belongs.

### 4.6 Structural Keywords

These are grammar constructs, not reserved variable names. Variables may be named anything, including words that appear in this list as block types:

`module` `template` `fragment` `ruleset` `parameters` `interface` `inputs` `outputs` `generate` `case` `validation` `metadata` `include` `depends_on`

### 4.7 Template Structure

```hcl
template "app" {
  metadata { ... }

  parameters {
    // Flat variables
    provider { type = string }
    region   { type = string }

    // Structured groups (dotted CLI: -Dprimary_db.size=XL)
    primary_db {
      size     { type = string }
      name     { type = string }
      password { type = string, sensitive = true }
    }
    analytics_db {
      size { type = string }
      name { type = string }
    }
  }

  resource "app-network" {
    module = "network"
    inputs = {
      cidr = "10.0.0.0/16"
    }
  }

  resource "primary-db" {
    module  = "relational_db"
    inputs  = {
      size   = $primary_db.size
      name   = $primary_db.name
      vpc_id = app-network.vpc_id      // cross-resource reference
    }
    depends_on = ["app-network"]       // explicit when no output reference exists
  }

  resource "analytics-db" {
    module = "relational_db"
    inputs = {
      size   = $analytics_db.size
      name   = $analytics_db.name
      vpc_id = app-network.vpc_id
    }
    depends_on = ["app-network"]
  }

  outputs {
    db_endpoint = primary-db.endpoint
  }
}
```

### 4.8 Module Structure

```hcl
module "relational_db" {
  metadata {
    name        = "relational_db"
    version     = "1.0.0"
    description = "..."
    authors     = ["Platform Team"]
    tags        = ["database", "relational"]

    lifecycle {
      maturity = "stable"
    }

    governance {
      security_tier          = "high"
      compliance_frameworks  = ["soc2", "pci-dss"]
      data_classification    = "sensitive"
    }
  }

  interface {
    inputs {
      size     { type = string, required = true }
      name     { type = string, required = true }
      vpc_id   { type = string, required = true }
      password { type = string, required = true, sensitive = true }
    }

    outputs {
      endpoint { type = string }
      port     { type = number }
    }
  }

  validation {
    rule "name_max_length" {
      condition     = "len($name) <= 63"
      error_message = "Database name must be 63 characters or fewer"
      severity      = "error"
    }
  }

  generate {
    case ($provider) {
      "aws": {
        code = <<-CFN
          // CloudFormation — module author writes real CFN YAML
          // Uses $size, $name, $vpc_id, $password, $resource_name
        CFN
        outputs {
          endpoint = "!GetAtt ${resource_name}Cluster.Endpoint.Address"
          port     = "5432"
        }
      }
      "azure": {
        code = <<-BICEP
          // Bicep — module author writes real Bicep
        BICEP
        outputs {
          endpoint = "sqlServer.properties.fullyQualifiedDomainName"
          port     = "1433"
        }
      }
    }
  }
}
```

---

## 5. Functional Requirements — Community

**R1 — Multi-cloud compilation**
Compile a template and its referenced modules to a single combined output per provider per region. The compiler has no knowledge of any cloud provider or IaC language. Module authors provide all cloud-specific content.

**R2 — Module system**
Modules are versioned, self-describing units. Each module declares a typed interface (inputs and outputs), validation rules, governance metadata, and one or more case arms containing provider-specific code. Multiple instances of the same module may be used in a single template; each is independently named and parameterized.

**R3 — Template composition**
Templates assemble modules into deployments. They declare typed parameters (including structured groups), instantiate named module resources, wire outputs to inputs across resources, and declare dependency ordering. The template is the sole integration point — modules do not communicate directly.

**R4 — Variable system**
All parameterization flows through variables. Variables are plain scalars or structured groups accessed with dotted notation. The resolution chain is: gmod default → gtpl override → yaml/json properties → `-D` flags. Missing value at compile time is a hard error. No fallback at any level.

**R5 — Region as variable**
Logical region names (`EAST`, `EU`, `APAC`) are variables. Provider-specific region strings are resolved via mapping tables defined in properties files or gmod, with gtpl override permitted. Module code references the resolved region value.

**R6 — Properties / configuration merging**
YAML or JSON files supply variable values. Multiple files merge with defined precedence (base → environment → team → local). Deep merge by default. `-D` flags override all file-based values.

**R7 — Dependency resolution**
The compiler builds a dependency graph from cross-resource output references and explicit `depends_on` declarations. It performs topological sort, detects circular dependencies, and generates code in correct order. Independent resources may generate in parallel.

**R8 — Output mapping**
Module outputs are typed and declared in `interface.outputs`. Cross-resource references in templates (`resource-name.output-name`) resolve using the producing module's reference template strings in the matching case arm's `outputs {}` block. The resolved string is injected into the consuming module's render context as a variable.

**R9 — Validation**
- Lexical: valid identifiers, string literals, number literals
- Syntactic: correct block structure per grammar
- Semantic: required fields present, types match, cross-references resolve, version constraints valid
- Custom: module-defined rules with `condition`, `error_message`, `severity` (`error` | `warning` | `info`)
- Case completeness: warn if a case expression has no arm for a commonly used variable value (configurable)

**R10 — Governance metadata**
Modules carry governance fields: `security_tier`, `compliance_frameworks`, `cost_tier`, `data_classification`, `disaster_recovery_tier`, `approval_required`. In the community tier, this data is informational — it is carried through to generated output as comments or metadata files but not enforced by the compiler.

**R11 — CLI**

| Command | Purpose |
|---------|---------|
| `graydr compile` | Compile template to combined IaC output |
| `graydr validate` | Validate `.gmod` and `.gtpl` files without compiling |
| `graydr init module` | Scaffold a new `.gmod` with correct structure and inline guidance |
| `graydr init template` | Scaffold a new `.gtpl` with parameter groups and resource stubs |
| `graydr version` | Show version information |

Required compile flags: `--template`, `--include-path`.
All other values supplied via `-D` flags or properties files.
No hard-coded enumeration of providers, regions, or sizes in the CLI.

**R12 — Module fragments**
Reusable code snippets (`.gfrag`) that module authors include within case arms. Fragments reduce boilerplate for common patterns (standard tagging, monitoring configuration, backup policies). Fragments are versioned and follow the same `include "source/name@version"` syntax as modules.

**R13 — Scaffolding**
`graydr init module` and `graydr init template` generate correctly structured skeletons with:
- All required blocks stubbed out
- Placeholder case arms for common providers (aws, azure, gcp)
- Inline comments referencing the authoring guide
- Validation that the scaffolded file passes basic structural checks

**R14 — Documentation (ships with the tool)**
- Language specification (grammar, types, expressions, case statement)
- Module authoring guide
- Template authoring guide
- Fragment authoring guide
- CLI reference
- Cross-resource reference guide (output mapping patterns per provider)
- Migration guide from v1

---

## 6. Functional Requirements — Enterprise

Enterprise extends community without forking it. The community CLI and language are the foundation. Enterprise adds the registry, portal, and governance engine.

**ER1 — Module Registry**
Maven-style registry. Module coordinates: `org/name@version`.

Lifecycle states:
- `beta` — published, approved for non-production use
- `active` — approved for all environments
- `deprecated` — use discouraged, replacement documented
- `retired` — no new use permitted, existing use generates warnings
- `*security*` — vulnerability found; blocks new deployments, triggers notification workflow

Workflow: `graydr submit` → automated lint gate → human approval chain → `beta` or `active`.

Version policy: SemVer enforced. Breaking changes (removed outputs, added required inputs) require major version bump, detected automatically by registry on publish.

**ER2 — Rule Lint**
Org-defined rulesets (`.grule`) specify what any published module must provide. Evaluated by `graydr lint` before `graydr publish`. Publish is blocked on lint failure.

Ruleset can require:
- Specific input variable names and types
- Specific output names and types
- Minimum number of providers supported
- Specific providers (e.g., must support `aws` and `azure`)
- Governance metadata fields (security_tier, compliance_frameworks, etc.)
- Naming conventions
- Minimum/maximum size tier count
- Org-defined variable name standards (e.g., all modules must have a `size` input)

The ruleset is the mechanism by which platform teams standardize module interfaces across the library. Enterprise teams may define org-level canonical variable names (e.g., `XL`, `L`, `M`, `S`, `XS` as the approved size vocabulary) and enforce them via ruleset — modules using non-conforming names fail lint.

**ER3 — Management Portal**
Web portal (Artifactory-style) providing:
- RBAC: roles include admin, approver, author, consumer, auditor
- SSO/SAML, LDAP/AD, and local authentication
- Approval chains: configurable per lifecycle transition, quorum rules, escalation paths
- Module browsing: search and filter by lifecycle, provider support, governance tags, compliance frameworks
- Org configuration: rulesets, region maps, approved provider list, cost thresholds, secret backends
- Use report dashboard: modules deployed, instance counts, cost estimates
- Security event management: flag modules, compose notifications, track project acknowledgments
- Audit log: all publishes, approvals, deprecations, security events, policy changes

**ER4 — Use Reporting**
On each `graydr compile`, graydr optionally emits a use report: modules used, versions, instance counts, variable values (excluding sensitive), governance metadata. Report submitted to registry. Enables:
- Impact analysis before deprecating a module
- Security event notifications to affected projects
- Inventory of deployed infrastructure

**ER5 — Security Event Workflow**
When a module is flagged `*security*`:
- New deployments using that module version are blocked
- All projects with recorded use (via use reports) are notified
- Acknowledgment tracking in portal
- Upgrade path documented by module author

**ER6 — Cost Governance**
Governance metadata may include pricing data attached to variable values (e.g., size `XL` = `$5000/day base + $30/GB`). Compiler can emit a cost estimate alongside IaC output. Rulesets may define cost thresholds; `--cost-limit` flag fails compile if estimate exceeds threshold.

**ER7 — Compliance Template Generation**
Alongside IaC output, graydr can generate a compliance document mapping the selected modules and variables to compliance framework controls (SOC2, PCI-DSS, HIPAA, FedRAMP). Based on governance metadata carried in modules.

**ER8 — Secret Injection**
Sensitive variables may be sourced from secret backends rather than passed as plaintext `-D` values:
```
-Ddb.password=@vault:secret/myapp/db
-Ddb.password=@aws-secrets:arn:aws:secretsmanager:...
-Ddb.password=@azure-keyvault:https://myvault.vault.azure.net/secrets/dbpass
```
Secret is resolved at compile time. Never written to disk or shell history.

**ER9 — Module Signing and Provenance**
Modules published to the enterprise registry are signed. `--require-signed` flag at compile time rejects unsigned modules. Chain of custody tracked in registry for audit.

**ER10 — Breaking Change Detection**
Registry compares module interface to previous version on publish. Automatically classifies SemVer impact:
- Removed output → major
- Added required input (no default) → major
- Changed output type → major
- Added optional input → minor
- Everything else → patch

Blocks publish if declared version is lower than detected impact.

**ER11 — Impact Analysis**
`graydr impact --module org/relational-db@1.2.0` queries use reports in registry and returns the list of templates currently using that version. Answers "who breaks if I deprecate this?" before action is taken.

**ER12 — Deployment Inventory**
Track which compiled templates are deployed to which environments, accounts, and regions. Requires integration with deploy pipeline via `graydr record-deploy` or CI/CD plugin. Enables "what's running this module" queries independent of compile-time use reports.

**ER13 — Policy-as-Code Integration**
OPA/Rego policy rules evaluated against compiled output before it reaches the deployment tool. Enforces runtime constraints ("no storage buckets without encryption") at compile time. `graydr compile --policy org-policy.rego`.

**ER14 — Approval Workflows**
Certain variable combinations trigger approval requests before compile succeeds. Configurable in ruleset or portal. Integration with ticketing systems (Jira, ServiceNow) and webhook endpoints.

**ER15 — `--no-local-modules`**
Compile flag that rejects any module not sourced from the enterprise registry. Prevents use of local or unvetted modules in controlled environments.

---

## 7. Key Design Decisions

These decisions are final. They should not be relitigated without updating this document.

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Size tiers | No primitive — module-managed conditionals or case arms | Size is a variable like any other; arbitrary naming required |
| Fallback resolution | Removed — missing = fail | Fallback hides errors; explicit failure is correct for production |
| `provider "name" {}` blocks | Replaced by `case ($variable) {}` | Any variable can drive dispatch; no reserved names; IBM/Oracle cost zero language changes |
| `language` attribute | Removed | Graydr does not know what it emits; build platform handles deployment tool selection |
| `size_configs` primitive | Removed | Size is a variable; config lookup is handled by case statements in module code |
| Reserved variable names | None | `provider`, `region`, `size` are user-defined variable names, not language keywords |
| Cross-resource references | Outputs block reference template strings | Language-specific reference syntax belongs in the module, not the template |
| Combined output | One unit per provider per region | The gtpl is the assembly point; deployment tools expect unified input |
| Variable scope | Explicit passing only | Modules see only what templates explicitly pass in `inputs`; no implicit propagation |
| Globals/locals | No distinction — all variables are global within their scope layer | Simplicity; dotted structured parameters provide namespacing where needed |
| gtpl override of gmod defaults | Permitted | Enterprise concern (region map locking) deferred; easy to restrict later |
| Community vs Enterprise split | Community = language + compiler; Enterprise = registry + portal + governance | Core tool is open; enterprise adds without forking |

---

## 8. Roadmap

### Phase 1 — Language and Compiler Rewrite
Implement the v2 language design. This is a significant rewrite of the v1 implementation.

- New variable system (`$name`, `group.field`, `-D` flags)
- Case statement (`case ($var) { "value": { ... } }`)
- Remove `size_configs` primitive and `SizeTier` enum
- Remove `provider "name" {}` block structure
- Remove `language` attribute
- Remove fallback resolution — missing = fail
- Output reference template strings (`${resource_name}` substitution)
- Module fragment support (`.gfrag`, `include`)
- Structured parameter groups in templates
- Combined output assembly per provider per region
- Updated validation for new grammar
- Updated dependency resolver for new structure

### Phase 2 — CLI and Scaffolding
- `graydr compile` with `-D` flag support
- `graydr validate`
- `graydr init module` scaffolding
- `graydr init template` scaffolding
- `graydr version`
- Properties file loading (yaml/json)
- Region map resolution

### Phase 3 — Documentation
- Language specification (v2)
- Module authoring guide
- Template authoring guide
- Fragment authoring guide
- CLI reference
- Cross-resource reference guide (output mapping patterns per IaC language)
- Migration guide from v1

### Phase 4 — Reference Module Library
Ship 10 Momidala-authored modules covering core infrastructure patterns. All three providers (AWS, Azure, GCP) for each. All modules pass the community standard ruleset.

| Module | Patterns covered |
|--------|-----------------|
| `network` | VPC/VNet, subnets, private endpoints |
| `relational_db` | Aurora / Azure SQL / Cloud SQL |
| `object_storage` | S3 / Blob Storage / GCS |
| `cache` | ElastiCache / Azure Cache / Memorystore |
| `load_balancer` | ALB / Application Gateway / Cloud Load Balancing |
| `dns` | Route 53 / Azure DNS / Cloud DNS |
| `container_registry` | ECR / ACR / Artifact Registry |
| `kubernetes` | EKS / AKS / GKE |
| `secret_manager` | Secrets Manager / Key Vault / Secret Manager |
| `queue` | SQS / Service Bus / Pub/Sub |

### Phase 5 — Enterprise: Registry and Rule Lint
- Module registry (ER1)
- Rule lint and `.grule` format (ER2)
- `graydr submit`, `graydr publish`, `graydr lint`
- Use reporting (ER4)
- `--no-local-modules` flag (ER15)
- Breaking change detection (ER10)
- Impact analysis (ER11)

### Phase 6 — Enterprise: Management Portal
- Web portal (ER3)
- RBAC, authentication, approval chains
- Security event workflow (ER5)
- Audit logging

### Phase 7 — Enterprise: Advanced Governance
- Cost governance and estimation (ER6)
- Compliance template generation (ER7)
- Secret injection (ER8)
- Module signing and provenance (ER9)
- Policy-as-code integration (ER13)
- Approval workflows (ER14)
- Deployment inventory (ER12)

---

## 9. Out of Scope

These are explicitly not requirements and should not influence design:

- **Runtime system** — graydr does not deploy infrastructure, manage state, or run after compile time
- **IaC language parsing** — graydr does not parse or validate Terraform, Bicep, CloudFormation, or any output language
- **Cloud provider SDKs** — graydr has no cloud provider dependencies
- **Drift detection** — detecting deviation between desired and deployed state is out of scope for the core tool (may appear as an enterprise integration, not a core feature)
- **Abstraction of IaC semantics** — graydr does not abstract resources, providers, or APIs; module authors write real code

---

*graydr v2 Requirements — Momidala Consulting, LLC — 2026-03-06*
