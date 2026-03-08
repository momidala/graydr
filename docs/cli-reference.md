# graydr CLI Reference

## Synopsis

```
graydr <SUBCOMMAND> [OPTIONS]
```

## Subcommands

| Subcommand       | Description                                         |
|------------------|-----------------------------------------------------|
| `compile`        | Compile a `.gtpl` template to IaC output            |
| `validate`       | Validate `.gmod` or `.gtpl` files without compiling |
| `init module`    | Scaffold a new `.gmod` module file                  |
| `init template`  | Scaffold a new `.gtpl` template file                |
| `version`        | Print the graydr version string                     |

## Exit Codes

| Code | Meaning                                                              |
|------|----------------------------------------------------------------------|
| 0    | Success                                                              |
| 1    | Error (parse error, validation error, compile error, missing file)   |

> **Note:** Exit code 1 is used for any error condition without exception.
> `graydr validate` calls `std::process::exit(1)` directly on error;
> `graydr compile` propagates `anyhow::Result<()>` through `main()`.
> There are no other exit codes.

---

## graydr compile

**Usage:** `graydr compile --template <PATH> [OPTIONS]`

Compiles a `.gtpl` template file by resolving all referenced modules (`.gmod`),
substituting variables, dispatching case arms, and writing the assembled IaC output.

### Flags

| Flag             | Value     | Required | Description                                                                                                                            |
|------------------|-----------|----------|----------------------------------------------------------------------------------------------------------------------------------------|
| `--template`     | PATH      | Yes      | Path to the `.gtpl` template file to compile.                                                                                          |
| `--include-path` | PATH      | No       | Directory to search for `.gmod` module files and `.gfrag` fragment files.                                                              |
| `-D`             | KEY=VALUE | No       | Override a variable value. Repeatable. For a given key, the **last** `-D` value wins. Takes precedence over all properties file values. |
| `--properties`   | FILE      | No       | YAML or JSON properties file. Repeatable. Files are deep-merged in declaration order; later files take precedence on key conflicts.     |
| `--output`       | FILE      | No       | Write compiled output to this file. If omitted, compiled output goes to stdout.                                                        |

### Variable Precedence

Variables are resolved from four sources. When the same key appears in more than one source,
the higher-priority source wins.

| Priority | Source                    | Notes                                                          |
|----------|---------------------------|----------------------------------------------------------------|
| 4 (lowest) | `gmod` default values   | `default = "..."` in `interface.inputs` declarations           |
| 3          | `.gtpl` input overrides | `inputs { key = "literal" }` in the resource block            |
| 2          | `--properties` files    | Deep-merged in declaration order; later file wins on conflict  |
| 1 (highest)| `-D` flags              | Repeatable; for a given key the last `-D` value wins           |

Any variable reference that cannot be resolved from these four sources is a **hard compile error**.
There is no fallback, no null, and no empty-string default. The variable name and file position
are included in the error message.

### Examples

**Multiple `-D` flags (last wins):**

```sh
graydr compile --template infra.gtpl -D provider=aws -D provider=azure
```

Result: `provider` resolves to `azure` (last `-D` wins).

**Multiple `--properties` files (later file wins on conflict):**

```sh
graydr compile --template infra.gtpl --properties base.yaml --properties prod.yaml
```

Result: values in `prod.yaml` take precedence over `base.yaml` on any conflicting key.

**Write output to file:**

```sh
graydr compile --template infra.gtpl --include-path ./modules --output main.tf
```

---

## graydr validate

**Usage:** `graydr validate <FILES...>`

Validates one or more `.gmod` or `.gtpl` files by parsing and checking structure.
Does not compile or resolve cross-file dependencies.

- Accepts one or more file paths.
- Reports all errors before stopping (not fail-fast on first error within a file).
- Exits 0 if all files are valid; exits 1 if any file has errors.

### Examples

```sh
graydr validate storage.gmod cdn.gmod infra.gtpl
```

---

## graydr init module

**Usage:** `graydr init module [--output FILE]`

Writes a `.gmod` scaffold to `FILE` (or stdout if `--output` is omitted).

The scaffold includes all required blocks (`metadata`, `interface`, `validation`, `generate`)
with placeholder `case` arms for at least two variable values and inline comments
explaining each block's purpose.

### Flags

| Flag       | Value | Required | Description                                    |
|------------|-------|----------|------------------------------------------------|
| `--output` | FILE  | No       | Write scaffold to file. Default: stdout.       |

### Example

```sh
graydr init module --output my_module.gmod
```

---

## graydr init template

**Usage:** `graydr init template [--output FILE]`

Writes a `.gtpl` scaffold to `FILE` (or stdout if `--output` is omitted).

The scaffold includes a `metadata` block, a `parameters` block, one placeholder
`resource` block, and an `outputs` block.

### Flags

| Flag       | Value | Required | Description                                    |
|------------|-------|----------|------------------------------------------------|
| `--output` | FILE  | No       | Write scaffold to file. Default: stdout.       |

### Example

```sh
graydr init template --output my_template.gtpl
```

---

## graydr version

**Usage:** `graydr version`

Prints the graydr version string and exits 0. No flags.

### Example

```sh
graydr version
```

---

*CLI reference for graydr v2. Derived from `src/cli/args.rs`. Run `graydr --help` for built-in help text.*
