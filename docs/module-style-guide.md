# graydr Module Style Guide

> Prescriptive conventions for writing well-structured, multi-cloud graydr modules.
> This guide explains *when and how* to apply graydr language features well.
> For how the features work mechanically, see the [authoring guides](./module-authoring-guide.md).

## Contents

1. [Naming Conventions](#1-naming-conventions)
2. [Multi-Cloud Interface Design](#2-multi-cloud-interface-design)
3. [Governance Metadata Standards](#3-governance-metadata-standards)
4. [Fragment Usage Patterns](#4-fragment-usage-patterns)
5. [Adding a New Provider Arm](#5-adding-a-new-provider-arm)

---

## 1. Naming Conventions

**Rules at a glance:** snake_case everywhere, module identifier matches the filename stem, inputs and outputs are cloud-agnostic.

### 1.1 Module identifier must match the filename stem

The compiler resolves `module = "network"` in a template by looking for `network.gmod` in the include path. A mismatch causes `UnresolvedModule` at compile time — the error is hard, not a warning.

```hcl
# File: modules/object_storage/object_storage.gmod
module "object_storage" {
  # module name matches filename stem exactly
```

Rule: the string label after `module "..."` must equal the stem of the `.gmod` filename (filename without the `.gmod` extension).

### 1.2 Directory layout: one directory per module

The reference library uses `modules/{module_name}/{module_name}.gmod`. Prescribe this layout for all reference modules:

```
modules/
  object_storage/
    object_storage.gmod
  relational_db/
    relational_db.gmod
  network/
    network.gmod
```

Rule: each module lives in its own subdirectory of `modules/`. The directory name and the `.gmod` filename stem both equal the module identifier. Do not place multiple modules in the same directory.

### 1.3 Use snake_case for all identifiers

Inputs, outputs, module names, fragment names, and parameter group names all use snake_case. The grammar permits hyphens in identifiers (`[a-zA-Z_][a-zA-Z0-9_-]*`), but underscores are the standard — consistent with Terraform and CloudFormation naming conventions.

Do not use camelCase, PascalCase, or kebab-case.

| Identifier type | snake_case (correct) | Do not use |
|-----------------|---------------------|------------|
| Module name | `object_storage` | `objectStorage`, `ObjectStorage`, `object-storage` |
| Input name | `bucket_name` | `bucketName`, `BucketName`, `bucket-name` |
| Output name | `bucket_url` | `bucketUrl`, `BucketURL`, `bucket-url` |
| Fragment name | `aws_standard_tags` | `awsStandardTags`, `aws-standard-tags` |
| Parameter group | `primary_db` | `primaryDb`, `primary-db` |

### 1.4 Input names must be cloud-agnostic

An input named `aws_s3_bucket_name` is cloud-specific — it can only be supplied correctly by someone thinking in AWS terms. Use `bucket_name`. The case arm translates the abstract input into provider-specific resource attributes internally.

```hcl
interface {
  inputs {
    # GOOD: cloud-agnostic names
    bucket_name = { required = true, sensitive = false }
    region      = { required = true, sensitive = false }

    # BAD: do not do this — cloud-specific prefix violates the interface contract
    # aws_s3_bucket_name = { required = true, sensitive = false }
    # azure_storage_account_name = { required = true, sensitive = false }
  }
}
```

Warning signs: any input name that starts with `aws_`, `azure_`, `gcp_`, `ibm_`, or `oci_`.

### 1.5 Output names describe the value, not the resource

Outputs are consumed by templates and must be cloud-agnostic. Name outputs for the value they carry — `endpoint`, `port`, `bucket_url`, `dns_name`, `registry_url` — not for the resource that produces them.

| Output value | Correct name | Do not use |
|--------------|-------------|------------|
| URL to a storage bucket | `bucket_url` | `s3_output`, `storage_account_url` |
| DNS hostname | `dns_name` | `elb_dns_name`, `lb_fqdn` |
| Container registry URL | `registry_url` | `ecr_url`, `acr_login_server` |
| Database connection endpoint | `endpoint` | `rds_endpoint`, `sql_server_fqdn` |

### 1.6 Dotted-path input groups use snake_case throughout

When a module declares dotted-path inputs (populated from a template's parameter group), use snake_case for both the group prefix and the field name.

```hcl
interface {
  inputs {
    primary_db.size   = { required = true, sensitive = false }
    primary_db.region = { required = true, sensitive = false }
    # NOT: primaryDb.size, primary-db.size
  }
}
```

### Summary naming table

| Element | Convention | Example |
|---------|------------|---------|
| Module identifier | snake_case, matches filename stem | `object_storage` |
| Module filename | `{module_name}.gmod` | `object_storage.gmod` |
| Module directory | `modules/{module_name}/` | `modules/object_storage/` |
| Input name | snake_case, cloud-agnostic | `bucket_name`, `region` |
| Output name | snake_case, describes the value | `bucket_url`, `dns_name` |
| Input group | snake_case prefix + field | `primary_db.size` |
| Fragment name | snake_case, describes the concern | `aws_standard_tags` |

---

## 2. Multi-Cloud Interface Design

**Core principle:** *A module's interface is cloud-agnostic. Its implementation is cloud-specific. The template wires them together.*

The graydr compile model divides responsibility cleanly: modules encapsulate provider-specific IaC code behind stable, abstract interfaces; templates assemble module instances into deployable configurations. Getting this boundary right is the most important design decision a module author makes.

### Decision rules

**Rule 1: Provider-specific IaC code always goes in the module.**

If a block of code looks different for AWS, Azure, and GCP, it belongs in a case arm inside the module's `generate` block — never inline in a template. Templates do not contain IaC code. A template that embeds Terraform HCL directly is a design error.

**Rule 2: Resource composition and cross-resource wiring go in the template.**

When one resource consumes another resource's output (for example, an app server that needs a storage bucket's URL), that wiring belongs in the template's `inputs {}` block using `${resource_instance.output_name}` references. The module does not know about other module instances — that topology is the template's concern.

```hcl
resource "app_server" {
  module = "appserver"
  inputs {
    storage_endpoint = "${main_storage.bucket_url}"
    # wiring lives here in the template, not in the module
  }
}
```

**Rule 3: Operator inputs go in the template's `parameters {}` block.**

Values like region, size, and environment name come from the operator (via a properties file or `-D` flag). The module declares them as interface inputs; the template supplies them via parameter group references. Neither the module nor the template hard-codes these values.

**Rule 4: Business logic validation goes in the module's `validation {}` block.**

If you need to enforce a constraint on an input (for example, "bucket_name must not be empty" or "region must be us-east-1 or eu-west-1"), put that rule in the module's `validation {}` block. Templates have no validation block — they cannot enforce input constraints.

**Rule 5: A module's interface must be cloud-agnostic end-to-end.**

An input named `aws_account_id` violates this rule because it can only be supplied meaningfully when deploying to AWS. An input named `account_id` is acceptable. The template wires the cloud-specific value in from the properties file; the module interface stays abstract and works identically across all providers.

### Module/template boundary table

| Concept | Template (.gtpl) | Module (.gmod) |
|---------|-----------------|----------------|
| Purpose | Wire module instances together | Encapsulate provider-specific IaC code |
| Contains | Parameter groups, resource blocks, output wiring | Interface, validation, generate/case blocks |
| Authored by | Platform engineer / infrastructure team | Module author / library maintainer |
| Executed | Once per deployment target | Once per resource instance |
| Has validation block | No | Yes |
| Contains IaC code | No | Yes (inside case arms) |

### Decision heuristic

**Ask: "Would this code look different for AWS vs. Azure vs. GCP?"**

- If yes — it belongs in a module case arm.
- If no — it likely belongs in the template.

A second heuristic for inputs: **Ask: "Would a caller using a different cloud provider need to change this input name?"**

- If yes — the input name is cloud-specific. Rename it.
- If no — the name is cloud-agnostic. Keep it.

For the mechanical syntax of `generate`, `case`, and `interface` blocks, see the [Module Authoring Guide](./module-authoring-guide.md).

---

## 3. Governance Metadata Standards

All reference modules must populate the following metadata fields before publishing. The compiler does not enforce these fields (they are optional at the language level), but they are required for reference library quality and are consumed by enterprise-tier tooling.

**Minimum required fields for reference modules:** `description`, `security_tier`, `cost_tier`, and `data_classification`.

### Field: description

A human-readable sentence that completes: "This module provisions a ..."

- Use a complete sentence ending with a period.
- State what cloud resource or concept the module encapsulates.
- Do not describe implementation details ("uses Terraform aws_s3_bucket resource") — describe the logical resource.

```hcl
# GOOD
description = "Cross-cloud object storage for application data."
description = "Multi-provider relational database with configurable engine and size."

# BAD — too vague
description = "Storage module."
description = "TODO"
```

### Field: security_tier

Canonical values (lowest to highest): `"low"`, `"medium"`, `"high"`, `"critical"`

| Value | When to assign |
|-------|---------------|
| `"low"` | DNS, parameter store, tagging resources, monitoring targets — no direct data access |
| `"medium"` | Object storage, databases, message queues — stores or processes data |
| `"high"` | Network access rules, load balancers, API gateways — controls network-level access |
| `"critical"` | Key management, IAM, secrets management, identity providers — controls access to everything else |

### Field: compliance_frameworks

A comma-separated string of compliance tag names. Set when the module's compiled output directly affects a compliance control.

Canonical tag names: `"SOC2"`, `"PCI-DSS"`, `"HIPAA"`, `"FedRAMP"`, `"ISO27001"`

```hcl
compliance_frameworks = "SOC2,PCI-DSS"   # storage module handling cardholder data
compliance_frameworks = "HIPAA"           # database module handling PHI
# omit the field if the module has no direct compliance implication
```

Do not set this field speculatively. Assign only the frameworks whose controls are directly affected by what the module provisions.

### Field: cost_tier

Canonical values: `"low"`, `"standard"`, `"premium"`, `"variable"`

| Value | When to assign |
|-------|---------------|
| `"low"` | DNS, parameter store, tags, monitoring — negligible cost |
| `"standard"` | Object storage, load balancers, static compute — predictable moderate cost |
| `"premium"` | Managed databases, Kubernetes clusters, NAT gateways — significant fixed cost |
| `"variable"` | Message queues, serverless functions, data transfer — usage-based billing, cost scales with load |

### Field: data_classification

Canonical values (lowest to highest sensitivity): `"public"`, `"internal"`, `"confidential"`, `"restricted"`

| Value | When to assign |
|-------|---------------|
| `"public"` | DNS, static asset storage, public CDN — data is publicly accessible by design |
| `"internal"` | General infrastructure — data is internal but not specially sensitive |
| `"confidential"` | Databases containing PII, session storage, audit logs — regulated or sensitive data |
| `"restricted"` | Key management, secrets management, credential storage — the highest sensitivity tier |

### Field: disaster_recovery_tier

Canonical values: `"tier1"`, `"tier2"`, `"tier3"`, `"tier4"`

| Value | RTO target | When to assign |
|-------|-----------|---------------|
| `"tier1"` | < 1 hour | Databases and networks underpinning production services |
| `"tier2"` | < 4 hours | Important services that can tolerate brief outages |
| `"tier3"` | < 24 hours | Non-critical workloads with daily recovery tolerance |
| `"tier4"` | Best effort | Development, staging, experimental environments |

### Field: approval_required

Set to `true` when any of these apply:

- `security_tier = "critical"`
- The module provisions access control or identity resources (IAM, key management)
- An applicable compliance framework requires change-management approval for this resource type

Set to `false` for all other reference modules in the community tier. In the community tier, this field is informational — graydr does not enforce approval gates. Enterprise-tier tooling reads this field to decide whether to require a human-approval step in the CI pipeline.

### Complete example: relational database module metadata

```hcl
metadata {
  description            = "Cross-cloud relational database for application data."
  version                = "1.0.0"
  security_tier          = "medium"
  compliance_frameworks  = "SOC2,HIPAA"
  cost_tier              = "premium"
  data_classification    = "confidential"
  disaster_recovery_tier = "tier1"
  approval_required      = false
}
```

### Complete example: object storage module metadata

```hcl
metadata {
  description            = "Cross-cloud object storage for application assets."
  version                = "1.0.0"
  security_tier          = "medium"
  compliance_frameworks  = "SOC2"
  cost_tier              = "standard"
  data_classification    = "internal"
  disaster_recovery_tier = "tier2"
  approval_required      = false
}
```

> **Do not ship with TODO governance fields.** The compiler does not enforce governance fields, so it is tempting to leave them as empty strings or `"TODO"`. Reference modules with unset governance fields cannot be consumed reliably by enterprise tooling. Set all required fields before publishing.

For the mechanical syntax of the `metadata` block, see the [Module Authoring Guide](./module-authoring-guide.md).

---

## 4. Fragment Usage Patterns

Fragments (`.gfrag` files) let you share boilerplate IaC blocks across multiple module case arms without duplication. Use them deliberately — over-extracting to fragments adds indirection without benefit.

For the mechanical syntax of fragments and the `include` directive, see the [Fragment Authoring Guide](./fragment-authoring-guide.md).

### When to extract to a fragment

Extract to a `.gfrag` file when **all three** of the following are true:

1. **The block appears in more than one module's case arm** — verbatim or near-verbatim. If only one module uses it, keep it inline.
2. **The block is provider-specific boilerplate** — tagging policies, standard IAM role bindings, provider configuration blocks. Generic boilerplate used the same way everywhere is the primary use case.
3. **The block has no per-module variation** — it references only variables that are universally available in every context where the fragment will be included (such as `$environment` or `$provider`). If the block needs `$db_engine` and only database modules declare that input, the fragment will fail with `UnresolvedVariable` when included from a non-database module.

### When to keep inline

Keep IaC code inline in the case arm when:

- The code is unique to a single module (no reuse benefit from extracting)
- The code uses inputs specific to this module (`$bucket_name`, `$db_engine`) — those inputs may not be declared in other modules
- The code is fewer than 5–10 lines — extraction adds an `include` hop without meaningful reuse value

### Fragment naming and organization

- Fragment files live in a `fragments/` subdirectory of the include path: `fragments/aws_standard_tags.gfrag`
- Fragment names use snake_case, same as module names
- Fragment names describe the cross-cutting concern, not the modules that use them: `aws_standard_tags`, `azure_provider_config`, `gcp_labels`
- Do not create a fragment for single-use code

### Key technical constraints

- Fragments do not receive per-call parameters. They share the enclosing arm's variable context — variables resolve from the module that includes the fragment.
- Diamond includes are safe: a fragment included by two paths is included twice, producing duplicate IaC code. Avoid diamond includes unless the IaC is idempotent.
- Circular includes (`A` includes `B`, `B` includes `A`) are a hard compile error.

### Decision example

```hcl
# EXTRACT to fragment: AWS tagging policy appears in every AWS arm across
# multiple modules — pure boilerplate with universally available variables.
#
# File: fragments/aws_standard_tags.gfrag
fragment "aws_standard_tags" {
  code = <<-EOT
    tags = {
      managed_by  = "graydr"
      environment = "$environment"
    }
  EOT
}

# KEEP INLINE: resource-specific block unique to the object_storage module.
# It uses $bucket_name, which only this module declares as an input.
aws {
  code = <<-EOT
    include "fragments/aws_standard_tags.gfrag"
    resource "aws_s3_bucket" "$bucket_name" {
      bucket = "$bucket_name"
    }
  EOT
  outputs {
    bucket_url = "${aws_s3_bucket.object_storage.bucket_regional_domain_name}"
  }
}
```

---

## 5. Adding a New Provider Arm

graydr modules are designed for extension. Adding a new cloud provider (IBM Cloud, Oracle Cloud, or any future provider) follows the same three-step pattern regardless of provider.

**Prerequisite:** The provider must use a Terraform-compatible output format. graydr emits IaC code as text — the `code` heredoc contains whatever the target IaC toolchain expects. AWS, Azure, GCP, IBM Cloud, and Oracle Cloud all use Terraform providers, so their arms contain Terraform HCL.

### The three-step pattern

1. **Add the arm identifier** to the existing `case "provider"` block in the module's `generate` block.
2. **Write the provider-specific IaC code** in the arm's `code` heredoc.
3. **Map each declared output** to the provider-specific resource reference in the arm's `outputs {}` block.

### Worked example: adding IBM Cloud to object_storage

Starting from a module with `aws`, `azure`, and `gcp` arms, add IBM Cloud:

```hcl
module "object_storage" {
  metadata {
    description            = "Cross-cloud object storage for application data."
    version                = "1.0.0"
    security_tier          = "medium"
    compliance_frameworks  = "SOC2"
    cost_tier              = "standard"
    data_classification    = "internal"
    disaster_recovery_tier = "tier2"
    approval_required      = false
  }

  interface {
    inputs {
      bucket_name       = { required = true, sensitive = false }
      region            = { required = true, sensitive = false }
      resource_group_id = { required = false, sensitive = false, default = "" }
    }
    outputs {
      bucket_name = {}
      bucket_url  = {}
    }
  }

  validation {
    rule "bucket_name_not_empty" {
      condition     = "$bucket_name != \"\""
      error_message = "bucket_name must not be empty"
      severity      = "error"
    }
  }

  generate {
    case "provider" {
      aws {
        code = <<-EOT
          resource "aws_s3_bucket" "$bucket_name" {
            bucket = "$bucket_name"
            region = "$region"
          }
        EOT
        outputs {
          bucket_name = "$bucket_name"
          bucket_url  = "${aws_s3_bucket.object_storage.bucket_regional_domain_name}"
        }
      }
      azure {
        code = <<-EOT
          resource "azurerm_storage_account" "$bucket_name" {
            name     = "$bucket_name"
            location = "$region"
          }
        EOT
        outputs {
          bucket_name = "$bucket_name"
          bucket_url  = "${azurerm_storage_account.object_storage.primary_blob_endpoint}"
        }
      }
      gcp {
        code = <<-EOT
          resource "google_storage_bucket" "$bucket_name" {
            name     = "$bucket_name"
            location = "$region"
          }
        EOT
        outputs {
          bucket_name = "$bucket_name"
          bucket_url  = "${google_storage_bucket.object_storage.url}"
        }
      }
      ibm {
        code = <<-EOT
          resource "ibm_resource_instance" "$bucket_name" {
            name              = "$bucket_name"
            resource_group_id = "$resource_group_id"
            service           = "cloud-object-storage"
            plan              = "standard"
            location          = "global"
          }
        EOT
        outputs {
          bucket_name = "$bucket_name"
          bucket_url  = "${ibm_resource_instance.object_storage.crn}"
        }
      }
    }
  }
}
```

**Key points:**

- The arm identifier (`ibm`) must exactly match the value that `provider` resolves to at compile time. If the properties file contains `provider: ibm`, the compiler selects the `ibm` arm.
- The IBM Cloud Object Storage Terraform resource is `ibm_resource_instance`. The `service`, `plan`, and `location` attributes are IBM-specific — they are inside the case arm, invisible to the caller.
- Output names (`bucket_name`, `bucket_url`) match the other arms exactly. The caller uses the same output names regardless of provider.

### Oracle Cloud variant (secondary example)

For Oracle Cloud Infrastructure, the arm identifier is `oci` and the Terraform resource is `oci_objectstorage_bucket`:

```
oci arm identifier: "oci"
Terraform resource: oci_objectstorage_bucket
Required inputs: bucket_name, region, namespace (OCI-specific, add to interface)
```

The pattern is identical to the IBM example. Only the IaC code inside `code = <<-EOT ... EOT` changes.

### Verification steps after adding an arm

After adding any new arm:

1. Validate the module structure:
   ```
   graydr validate modules/object_storage/object_storage.gmod
   ```
   Exit code 0 confirms the module is structurally valid — all blocks present, interface well-formed.

2. Test-compile against all existing arms to confirm you did not break them:
   ```
   graydr compile --template examples/web-app-stack.gtpl \
     --include-path modules \
     --properties props/aws.yaml
   graydr compile --template examples/web-app-stack.gtpl \
     --include-path modules \
     --properties props/azure.yaml
   graydr compile --template examples/web-app-stack.gtpl \
     --include-path modules \
     --properties props/gcp.yaml
   ```

3. Test-compile against the new provider:
   ```
   graydr compile --template examples/web-app-stack.gtpl \
     --include-path modules \
     --properties props/ibm.yaml
   ```

**Standard:** All reference modules in the library must have arms for at least `aws`, `azure`, and `gcp`. A module with arms for only aws and azure will pass `graydr validate` but fail at compile time when `provider = "gcp"` is supplied. Always test all three canonical providers before publishing.

---

*For mechanical syntax details, see:*
- *[Module Authoring Guide](./module-authoring-guide.md) — interface, validation, generate/case blocks*
- *[Template Authoring Guide](./template-authoring-guide.md) — parameter groups, resource wiring, output references*
- *[Fragment Authoring Guide](./fragment-authoring-guide.md) — fragment file structure, include directive, cycle detection*
