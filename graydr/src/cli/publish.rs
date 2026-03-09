use crate::registry::{RegistryClient, RegistryConfig, ModuleCoord};
use super::args::PublishArgs;
use anyhow::Context;

/// Run the `publish` subcommand. Publishes a .gmod file to the community registry.
pub fn run_publish(args: PublishArgs) -> anyhow::Result<()> {
    // Override registry URL from --registry flag if provided.
    let mut config = RegistryConfig::from_env();
    if let Some(registry_url) = args.registry {
        config.base_url = registry_url;
    }
    if config.base_url.is_empty() {
        anyhow::bail!("registry URL required: set GRAYDR_REGISTRY_URL or use --registry <url>");
    }

    // Org comes from GRAYDR_REGISTRY_ORG env var.
    let org = std::env::var("GRAYDR_REGISTRY_ORG")
        .context("GRAYDR_REGISTRY_ORG env var required for publish")?;

    // Read module source to extract name and version.
    let module_src = std::fs::read_to_string(&args.module)
        .with_context(|| format!("cannot read module file: {}", args.module.display()))?;

    let (name, version) = extract_name_version_from_gmod(&module_src)
        .context("cannot extract module name/version from .gmod file")?;

    let coord_str = format!("{}/{}@{}", org, name, version);
    let coord = ModuleCoord::parse(&coord_str)
        .map_err(|e| anyhow::anyhow!("invalid module coordinate '{}': {}", coord_str, e))?;

    let client = RegistryClient::new(config);
    client
        .publish_module(&coord, &args.module)
        .map_err(|e| anyhow::anyhow!("publish failed: {}", e))?;

    println!("Published {} to registry", coord_str);
    Ok(())
}

/// Extract the module name and version from a raw .gmod source string.
///
/// Parses the HCL to find the `module "name" { metadata { version = "x.y.z" } }` structure.
/// The module block label is the name; the `version` attribute inside the `metadata` block
/// provides the version string.
fn extract_name_version_from_gmod(src: &str) -> anyhow::Result<(String, String)> {
    let body = hcl_edit::parser::parse_body(src)
        .map_err(|e| anyhow::anyhow!("HCL parse error: {}", e))?;

    let module_block = body
        .blocks()
        .find(|b| b.ident.as_str() == "module")
        .ok_or_else(|| anyhow::anyhow!("no 'module' block found in .gmod file"))?;

    let name = module_block
        .labels
        .first()
        .map(|l| l.as_str().to_owned())
        .ok_or_else(|| anyhow::anyhow!("module block has no name label"))?;

    // Find metadata block and extract version attribute.
    let metadata_block = module_block
        .body
        .blocks()
        .find(|b| b.ident.as_str() == "metadata")
        .ok_or_else(|| anyhow::anyhow!("module '{}' has no 'metadata' block", name))?;

    let version_attr = metadata_block
        .body
        .attributes()
        .find(|a| a.key.as_str() == "version")
        .ok_or_else(|| anyhow::anyhow!("metadata block has no 'version' field"))?;

    let version = match &version_attr.value {
        hcl_edit::expr::Expression::String(s) => s.value().to_owned(),
        other => other.to_string().trim().trim_matches('"').to_owned(),
    };

    Ok((name, version))
}
