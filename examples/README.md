# web-app-stack Example

## What This Example Shows

This template composes four reference modules — `network`, `relational_db`, `object_storage`, and `load_balancer` — into a deployable multi-cloud web application stack. graydr compiles the template into provider-native IaC (CloudFormation, Bicep, or Terraform/google) from a single source template, with no cloud credentials needed to run this example. The cross-resource wiring (`${net.vpc_id}` and `${net.subnet_ids}` passed into `relational_db` inputs) demonstrates the core graydr composition pattern: one module's outputs become another module's inputs.

## Prerequisites

- Rust toolchain installed (`cargo` available in your PATH)
- Git (to clone the repo)

That's it — no cloud accounts, no provider CLIs required.

## Build the graydr Binary

From the repository root:

```bash
cargo build
```

The binary will be at `./target/debug/graydr`.

## Compile Against Each Cloud

Run any or all of the following commands from the repository root. Each command produces provider-native IaC printed to stdout.

### AWS — CloudFormation YAML

```bash
./target/debug/graydr compile \
  --template examples/web-app-stack.gtpl \
  --include-path examples/modules \
  --properties examples/aws.yaml
```

### Azure — Bicep

```bash
./target/debug/graydr compile \
  --template examples/web-app-stack.gtpl \
  --include-path examples/modules \
  --properties examples/azure.yaml
```

### GCP — Terraform (google provider)

```bash
./target/debug/graydr compile \
  --template examples/web-app-stack.gtpl \
  --include-path examples/modules \
  --properties examples/gcp.yaml
```

## What the Template Wires Together

| Resource | Module | Inputs | Outputs |
|----------|--------|--------|---------|
| `net` | `network` | `name`, `region`, `cidr_block` | `vpc_id`, `subnet_ids` |
| `db` | `relational_db` | `name`, `region`, `db_username`, `db_password`, `instance_class`, **`vpc_id` from `net`**, **`subnet_ids` from `net`** | `endpoint` |
| `storage` | `object_storage` | `bucket_name`, `region` | `bucket_url` |
| `lb` | `load_balancer` | `name`, `region`, `resource_group_name` | `dns_name` |

The `db` resource demonstrates cross-resource wiring: `vpc_id = "${net.vpc_id}"` and `subnet_ids = "${net.subnet_ids}"` bind the `network` module's outputs to `relational_db`'s VPC placement inputs. The compiler passes these through as native IaC references in the compiled output.

Template-level outputs expose: `vpc_id`, `db_endpoint`, `bucket_url`, `lb_dns_name`.

## Customizing the Properties Files

The properties files `aws.yaml`, `azure.yaml`, and `gcp.yaml` contain the concrete input values for each cloud. You can change `name`, `cidr_block`, `region`, `db_username`, and `instance_class` to match your environment. Note that `db_password` is a placeholder value — in production, retrieve secrets from a secret manager rather than storing them in a properties file.

## Next Steps

- See `docs/module-style-guide.md` for how to author new reference modules following the same conventions used here.
- Any of the other reference modules (`cache`, `dns`, `kubernetes`, `container_registry`, `secret_manager`, `queue`) can be added to this template by adding a new `resource` block and a matching parameter group in `web-app-stack.gtpl`.
