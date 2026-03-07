use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use serde_yaml_ng::Value;

use crate::ast::common::Literal;
use crate::cli::args::CompileArgs;
use crate::graph::{DependencyGraph, assemble_by_provider_region};
use crate::parser::module::parse_module_file;
use crate::parser::template::parse_template_file;
use crate::resolver::context::ResolveContext;
use crate::resolver::dispatch::dispatch_case;
use crate::resolver::merge::{deep_merge, flatten_to_dotted, parse_cli_flag};
use crate::codegen::assemble_output;

pub fn run_compile(args: CompileArgs) -> anyhow::Result<()> {
    // ── Step 1: Read template file ─────────────────────────────────────────
    let template_path = args.template.to_string_lossy().to_string();
    let template_source = fs::read_to_string(&args.template)
        .with_context(|| format!("reading template file {}", template_path))?;

    // ── Step 2: Parse template ─────────────────────────────────────────────
    let template = parse_template_file(&template_source, &template_path)
        .with_context(|| format!("parsing template {}", template_path))?;

    // ── Step 3: Load and parse each resource's .gmod file ─────────────────
    let mut module_map = HashMap::new();
    let mut resource_map = HashMap::new();

    for resource_sw in &template.resources {
        let resource = &resource_sw.value;
        let resource_name = resource.name.value.clone();
        let module_name = &resource.module_ref.value;

        // Resolve .gmod path: {include_path}/{module_name}.gmod
        let gmod_path = if let Some(ref inc) = args.include_path {
            inc.join(format!("{}.gmod", module_name))
        } else {
            Path::new(&format!("{}.gmod", module_name)).to_path_buf()
        };

        let gmod_path_str = gmod_path.to_string_lossy().to_string();
        let gmod_source = fs::read_to_string(&gmod_path)
            .with_context(|| format!("reading module file {}", gmod_path_str))?;

        let module_def = parse_module_file(&gmod_source, &gmod_path_str)
            .with_context(|| format!("parsing module {}", gmod_path_str))?;

        module_map.insert(resource_name.clone(), module_def);
        resource_map.insert(resource_name, resource.clone());
    }

    // ── Step 4: Multi-file properties merge (CLI-06) ──────────────────────
    let mut merged: Value = Value::Null;

    for props_path in &args.properties {
        let props_path_str = props_path.to_string_lossy().to_string();
        let content = fs::read_to_string(props_path)
            .with_context(|| format!("reading properties file {}", props_path_str))?;

        let v: Value = if props_path.extension().and_then(|e| e.to_str()) == Some("json") {
            let json_val: serde_json::Value = serde_json::from_str(&content)
                .with_context(|| format!("parsing JSON properties {}", props_path_str))?;
            let json_str = serde_json::to_string(&json_val)
                .with_context(|| format!("re-serializing JSON properties {}", props_path_str))?;
            serde_yaml_ng::from_str(&json_str)
                .with_context(|| format!("converting JSON properties to YAML {}", props_path_str))?
        } else {
            serde_yaml_ng::from_str(&content)
                .with_context(|| format!("parsing YAML properties {}", props_path_str))?
        };

        if matches!(merged, Value::Null) {
            merged = v;
        } else {
            deep_merge(&mut merged, v);
        }
    }

    let mut properties_map: HashMap<String, String> = HashMap::new();
    if !matches!(merged, Value::Null) {
        flatten_to_dotted(merged, "", &mut properties_map)
            .with_context(|| "flattening merged properties to dotted map")?;
    }

    // ── Step 5: Parse -D flags ─────────────────────────────────────────────
    let cli_flags: HashMap<String, String> = args.defines
        .iter()
        .filter_map(|d| parse_cli_flag(d))
        .collect();

    // ── Step 6: Build gmod_defaults from InputDecl.default fields ─────────
    let mut gmod_defaults: HashMap<String, String> = HashMap::new();
    for (resource_name, module) in &module_map {
        for input_sw in &module.interface.value.inputs {
            let input = &input_sw.value;
            if let Some(ref default_sw) = input.default {
                let str_val = match &default_sw.value {
                    Literal::String(s) => s.clone(),
                    Literal::Bool(b) => b.to_string(),
                    Literal::Number(n) => n.to_string(),
                };
                // Key uses resource-scoped dotted name: resource_name.input_name
                let key = format!("{}.{}", resource_name, input.name.value);
                gmod_defaults.insert(key, str_val);
            }
        }
    }

    // ── Step 7: Build gtpl_overrides from resource InputBinding values ─────
    let mut gtpl_overrides: HashMap<String, String> = HashMap::new();
    for (resource_name, resource) in &resource_map {
        for binding_sw in &resource.inputs {
            let binding = &binding_sw.value;
            // Only use literal string values (no variable references)
            if binding.variables.is_empty() {
                let key = format!("{}.{}", resource_name, binding.key.value);
                gtpl_overrides.insert(key, binding.value.value.clone());
            }
        }
    }

    // ── Step 8: Build ResolveContext ───────────────────────────────────────
    let ctx = ResolveContext::build(gmod_defaults, gtpl_overrides, properties_map, cli_flags);

    // ── Step 9: Dispatch case blocks, collect arm_map ─────────────────────
    let mut arm_map = HashMap::new();
    for (resource_name, module) in &module_map {
        for case_sw in &module.generate.value.cases {
            let arm = dispatch_case(&case_sw.value, &ctx)
                .with_context(|| format!("dispatching case for resource {}", resource_name))?;
            arm_map.insert(resource_name.clone(), arm.clone());
        }
    }

    // ── Step 10: Build DependencyGraph, add explicit edges ────────────────
    let resource_names: Vec<String> = resource_map.keys().cloned().collect();
    let mut graph = DependencyGraph::new(&resource_names);

    for (resource_name, resource) in &resource_map {
        for dep_sw in &resource.depends_on {
            graph.add_explicit_edge(&dep_sw.value, resource_name, &resource_sw_span(&dep_sw.span))
                .with_context(|| format!("adding dependency edge for resource {}", resource_name))?;
        }
    }

    // ── Step 11: Topological sort ──────────────────────────────────────────
    let topo = graph.topo_order()
        .with_context(|| "resolving topological order")?;

    // ── Step 12: Build provider_map and region_map ─────────────────────────
    // Resolve the "provider" and "region" input values for each resource
    // by looking up the resource's input bindings in the resolve context.
    let mut provider_map: HashMap<String, String> = HashMap::new();
    let mut region_map: HashMap<String, String> = HashMap::new();

    for (resource_name, resource) in &resource_map {
        // Look up provider binding value from resource inputs
        for binding_sw in &resource.inputs {
            let binding = &binding_sw.value;
            let key = binding.key.value.as_str();
            if key == "provider" {
                // Resolve the binding value — may be a literal or a variable reference
                let resolved = resolve_binding_value(&binding.value.value, &ctx);
                if let Some(val) = resolved {
                    provider_map.insert(resource_name.clone(), val);
                }
            } else if key == "region" {
                let resolved = resolve_binding_value(&binding.value.value, &ctx);
                if let Some(val) = resolved {
                    region_map.insert(resource_name.clone(), val);
                }
            }
        }
    }

    // ── Step 13: Build region_mapping from ResolveContext ──────────────────
    let region_mapping = ctx.extract_region_mapping();

    // ── Step 14: Group resources by provider+region ────────────────────────
    let groups = assemble_by_provider_region(&topo, &provider_map, &region_map, &region_mapping);

    // ── Step 15+16: Render each group and concatenate output ──────────────
    let include_path = args.include_path.as_deref();
    let mut all_output = String::new();

    for group in &groups {
        let result = assemble_output(group, &module_map, &arm_map, &ctx, &resource_map, include_path)
            .with_context(|| format!("assembling output for provider={} region={}", group.provider, group.region))?;
        if !all_output.is_empty() {
            all_output.push('\n');
        }
        all_output.push_str(&result.output);

        // Surface warnings to stderr
        for issue in &result.issues {
            eprintln!("warning: {}", issue.message);
        }
    }

    // ── Step 17: Write output ──────────────────────────────────────────────
    if let Some(ref output_path) = args.output {
        fs::write(output_path, &all_output)
            .with_context(|| format!("writing output to {}", output_path.display()))?;
    } else {
        std::io::stdout()
            .write_all(all_output.as_bytes())
            .with_context(|| "writing output to stdout")?;
    }

    Ok(())
}

/// Resolve a binding value string to its concrete string.
///
/// If the value contains `$variable` references, resolves against `ctx`.
/// For simple variables like `$provider`, strips the `$` and looks up.
/// For literal values (no `$`), returns as-is.
/// Returns `None` if resolution fails (variable not in context).
fn resolve_binding_value(value: &str, ctx: &ResolveContext) -> Option<String> {
    let trimmed = value.trim();

    // Simple variable reference: "$varname" or "$group.field"
    if trimmed.starts_with('$') && !trimmed.starts_with("${") {
        let var_name = trimmed.trim_start_matches('$');
        // Use a zero-span for resolution (CLI context — no source location)
        use std::sync::Arc;
        use crate::ast::span::Span;
        let span = Span {
            file: Arc::from(""),
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
        };
        ctx.resolve(var_name, &span).ok().map(|s| s.to_string())
    } else if !trimmed.contains('$') {
        // Literal value — return as-is
        Some(trimmed.to_string())
    } else {
        // Complex expression with mixed literals and vars — return as-is
        Some(trimmed.to_string())
    }
}

/// Helper: extract span from a Spanned<String> for error context.
fn resource_sw_span(span: &crate::ast::span::Span) -> crate::ast::span::Span {
    span.clone()
}
