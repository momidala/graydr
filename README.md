# graydr

**Write infrastructure once. Compile to any cloud.**

graydr is a compile-time preprocessor for Infrastructure-as-Code. You write
modules in a provider-neutral format, then compile them to native
CloudFormation, Bicep, or Terraform — with no runtime, no abstraction layer,
and no dependency on graydr in the compiled output.

```
modules/network/network.gmod  ─┐
modules/relational_db/...     ─┤  graydr compile  ─►  infra-aws.yaml   (CloudFormation)
examples/web-app-stack.gtpl   ─┤                  ─►  infra-azure.bicep (Bicep)
examples/aws.yaml             ─┘                  ─►  infra-gcp.tf      (Terraform)
```

The compiled output is ordinary IaC — readable, auditable, and deployable
with standard tooling. graydr is only involved at build time.

---

## Install

**From source** (requires [Rust](https://rustup.rs)):

```bash
git clone https://github.com/momidala/graydr.git
cd graydr
cargo build --release
# binary at ./target/release/graydr
```

**From crates.io** (coming soon):

```bash
cargo install graydr
```

---

## Quick Start

1. **Build a module** — a reusable infrastructure component with per-provider IaC arms:

```hcl
module "network" {
  metadata { description = "VPC / VNet / VPC Network" version = "1.0.0" }

  interface {
    inputs  { name = { required = true } cidr_block = { required = true } }
    outputs { vpc_id = {} subnet_ids = {} }
  }

  generate {
    case "provider" {
      aws   { code = "resource \"aws_vpc\" \"$name\" { cidr_block = \"$cidr_block\" }" }
      azure { code = "resource \"azurerm_virtual_network\" \"$name\" { ... }" }
      gcp   { code = "resource \"google_compute_network\" \"$name\" { ... }" }
    }
  }
}
```

2. **Compose a template** — wire modules together into a stack:

```hcl
template "web-app-stack" {
  parameters "network_params" {
    name       = "web-app"
    cidr_block = "10.0.0.0/16"
  }

  resource "net" {
    module = "network"
    inputs { name = "$network_params.name"  cidr_block = "$network_params.cidr_block" }
  }

  resource "db" {
    module = "relational_db"
    inputs {
      vpc_id     = "${net.vpc_id}"      # cross-resource wiring
      subnet_ids = "${net.subnet_ids}"
    }
  }
}
```

3. **Compile**:

```bash
graydr compile \
  --template web-app-stack.gtpl \
  --include-path modules/network \
  --include-path modules/relational_db \
  --properties aws.yaml \
  --output infra-aws.yaml
```

---

## Reference Modules

graydr ships with 10 production-ready reference modules, each with AWS,
Azure, and GCP arms:

| Module | What it provisions |
|--------|--------------------|
| `network` | VPC / VNet / VPC Network |
| `relational_db` | Aurora PostgreSQL / Azure DB for PostgreSQL / Cloud SQL |
| `object_storage` | S3 / Blob Storage / GCS |
| `cache` | ElastiCache Redis / Azure Cache for Redis / Memorystore |
| `load_balancer` | ALB / Application Gateway / Cloud Load Balancing |
| `dns` | Route 53 / Azure DNS / Cloud DNS |
| `container_registry` | ECR / ACR / Artifact Registry |
| `kubernetes` | EKS / AKS / GKE |
| `secret_manager` | Secrets Manager / Key Vault / Secret Manager |
| `queue` | SQS / Service Bus / Pub/Sub |

See [`modules/`](modules/) for source and [`examples/`](examples/) for a
working multi-module stack.

---

## CLI Reference

```
graydr compile    --template <FILE> --include-path <DIR> [--include-path <DIR>...]
                  --properties <FILE> [--properties <FILE>...]
                  [--output <FILE>] [-D KEY=VALUE...]

graydr validate   <FILE> [<FILE>...]
graydr init       module [--output <FILE>]
graydr init       template [--output <FILE>]
graydr version
```

Full reference: [`docs/cli-reference.md`](docs/cli-reference.md)

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/language-spec.md`](docs/language-spec.md) | Full language grammar and semantics |
| [`docs/module-authoring-guide.md`](docs/module-authoring-guide.md) | How to write `.gmod` modules |
| [`docs/template-authoring-guide.md`](docs/template-authoring-guide.md) | How to write `.gtpl` templates |
| [`docs/module-style-guide.md`](docs/module-style-guide.md) | Conventions for reference-quality modules |
| [`docs/cli-reference.md`](docs/cli-reference.md) | CLI flags and exit codes |
| [`docs/fragment-authoring-guide.md`](docs/fragment-authoring-guide.md) | Reusable `.gfrag` fragments |
| [`docs/cross-resource-reference.md`](docs/cross-resource-reference.md) | Cross-resource output wiring |

---

## Community Registry

graydr includes a self-hostable module registry (`graydr-registry`) and a
`graydr publish` command for distributing modules via `org/name@version`
coordinates.

See [`graydr-registry/`](graydr-registry/) for the server source.

---

## Contributing

Contributions are welcome. Please read [`CONTRIBUTING.md`](CONTRIBUTING.md)
before opening a pull request — in particular the CLA requirement.

---

## License

- **Compiler** (`graydr/`): [AGPL 3.0](LICENSE) with output exception
- **Registry** (`graydr-registry/`): [AGPL 3.0](graydr-registry/LICENSE)
- **Reference modules** (`modules/`): [MIT](modules/LICENSE)
- **Examples** (`examples/`): [MIT](examples/LICENSE)

Commercial licenses available for proprietary use — contact
legal@momidala.com.

---

*Built by [Momidala Consulting, LLC](https://momidala.com)*
