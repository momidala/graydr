# Fragment Authoring Guide

This guide explains how to write a `.gfrag` file and how to include fragments in module case arms.
Read it top-to-bottom to write a working fragment include without consulting source code.

---

## Overview

A **fragment** is a reusable snippet of IaC code that can be included in any case arm's `code` block.
Fragments solve the repetition problem: if the same IAM role definition, tagging policy, or provider
configuration appears in multiple modules, extract it into a `.gfrag` file and include it once.

The include mechanism is a **text pre-pass** — fragment content is inserted into the case arm's code
string before HCL parsing begins. Variables in the fragment's code are resolved the same way as
variables in the enclosing case arm code. There is no macro system, no runtime indirection — just
string inclusion.

When to use fragments:
- Repeated boilerplate that appears in multiple module arms (tagging policies, IAM role stubs,
  provider-required configuration blocks).
- Provider-specific configuration that is shared across several modules but does not vary per module.

When NOT to use fragments:
- Module-specific logic that is unique to a single module. Put it directly in the module's case arm.
- Logic that varies per resource instance. Module inputs handle that; fragments do not receive per-call
  parameters.

---

## Fragment File Structure

A fragment file contains a single `fragment` block with a `code` heredoc:

```hcl
fragment "name" {
  code = <<-EOT
    # IaC code here
  EOT
}
```

The following is `tests/fixtures/sample.gfrag` — the canonical reference fragment:

```hcl
fragment "sample" {
  code = <<-EOT
    resource "aws_s3_bucket" "sample_bucket" {
      bucket = "my-bucket"
    }
  EOT
}
```

The `fragment` keyword is followed by a string label (the fragment name). The `code` attribute is a
heredoc string using the `<<-EOT` form — the `<<-` prefix strips leading whitespace, which allows
indented heredocs.

---

## Including a Fragment

The `include` directive goes inside a case arm's `code` heredoc. It is a text pre-pass directive, not
HCL syntax — it must appear on its own line and is processed before HCL parsing.

```hcl
generate {
  case "provider" {
    aws {
      code = <<-EOT
        include "shared/tags.gfrag"
        resource "aws_s3_bucket" "$bucket_name" {
          bucket = "$bucket_name"
        }
      EOT
      outputs {
        bucket_url = "${aws_s3_bucket.storage.bucket_regional_domain_name}"
      }
    }
  }
}
```

The `include "shared/tags.gfrag"` line is replaced with the full content of `tags.gfrag` before HCL
parsing. The resulting string is then parsed and compiled as normal.

---

## How Includes Work

1. **Pre-pass:** Before HCL parsing, graydr scans each case arm's code block for `include "path"`
   directives (one per line, any leading whitespace is allowed).
2. **File resolution:** The path in the `include` directive is resolved relative to the
   `--include-path` directory supplied at compile time. The `.gfrag` extension must be included in
   the path.
3. **Text substitution:** The `include` line is replaced with the fragment file's `code` heredoc
   content verbatim. No HCL is parsed yet.
4. **Variable resolution:** Variables in the substituted fragment code (e.g., `$bucket_name`) are
   resolved the same way as variables in the case arm code — from the same four-source resolution
   chain (module defaults, template overrides, properties files, -D flags).
5. **HCL parsing:** The expanded code string is parsed as HCL.

The include path must be provided with the `.gfrag` extension:

```
include "shared/tags.gfrag"       # correct
include "shared/tags"              # not resolved — .gfrag extension required
```

---

## Cycle Detection

Fragment A includes Fragment B, Fragment B includes Fragment A = hard compile error. The error names
all fragments in the cycle:

```
error: circular fragment include: A.gfrag -> B.gfrag -> A.gfrag
```

**Diamond includes are safe.** If Fragment A includes both Fragment B and Fragment C, and both B and
C include Fragment D, then D is inlined twice — once in B's expansion and once in C's expansion. This
is expected behavior. Diamond includes produce duplicate code; restructure to avoid the diamond if
duplicate output is undesirable.

The cycle detection algorithm tracks the **active recursion path** (call stack), not all visited nodes.
This means it correctly distinguishes cycles from diamonds.

---

## Source Map and Error Reporting

Errors in fragment code report the **fragment file path and line number**, not the include site. This
enables precise error messages even in deeply nested include graphs.

Example: if `tags.gfrag` has a syntax error on line 3, the error message will read:

```
error in shared/tags.gfrag:3: unexpected token
```

Not:

```
error in my_module.gmod:42: unexpected token
```

The source map is built during the pre-pass and carried through compilation. All graydr error types
that report a `Span` position use the source-mapped position, not the post-expansion position.

---

## Registry Coordinates (Deferred)

graydr supports registry coordinate syntax in `include` directives:

```
include "org/name@1.2.0"
```

This syntax is **parsed but not resolved** in the community tier. When graydr encounters a registry
coordinate include, it:

1. Emits a compile-time warning: `warning: registry coordinate includes are not resolved in community tier`
2. Leaves the include directive as a comment in the output (the include line is not expanded).
3. Continues compilation.

This behavior allows module authors to annotate where a community fragment would go without blocking
compilation. Registry resolution is a Phase 9 feature and is not available in the current release.

**Path-based includes** (`include "path/to/file.gfrag"`) resolve normally at any tier.

---

## When to Use Fragments

**Good use cases:**

- **Tagging policy blocks:** A tagging requirement that applies to every S3 bucket across all modules.
  Extract the `tags {}` block to a fragment; include it in each aws arm.
- **IAM role stubs:** A standard IAM execution role that is identical across Lambda function modules.
- **Provider configuration:** A provider block that must appear in every Terraform output but is the
  same in all modules.

**Cases where fragments are not the right tool:**

- Logic that differs per module (use the module's case arm directly).
- Logic that differs per resource instance (use module inputs and variable substitution).
- Large sections of IaC that belong in their own module (create a module, not a fragment).

---

*See also: [Module Authoring Guide](module-authoring-guide.md), [Template Authoring Guide](template-authoring-guide.md)*
