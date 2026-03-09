//! Case dispatch and output reference resolution.
//!
//! This module implements the core compile-time dispatch logic for graydr's `case` blocks.
//! Three public functions are provided:
//!
//! - [`dispatch_case`] — selects exactly one `CaseArm` for a given `CaseBlock` and variable context.
//! - [`resolve_output_mapping`] — substitutes `${resource_name}` in output template strings.
//! - [`check_case_completeness`] — warns when a resolved variable value has no matching arm.

use crate::ast::module::{CaseArm, CaseBlock, OutputMapping};
use crate::ast::span::Span;
use crate::resolver::context::ResolveContext;
use crate::resolver::error::ResolveError;

/// Configuration for dispatch behavior.
#[derive(Debug, Clone)]
pub struct DispatchConfig {
    /// When `true`, `check_case_completeness` emits a [`CompletenessWarning`] for each resolved
    /// variable value that has no matching arm. Set to `false` to suppress all warnings.
    pub completeness_warnings: bool,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        DispatchConfig { completeness_warnings: true }
    }
}

/// A warning emitted by [`check_case_completeness`] when a resolved variable value has no
/// matching arm in the case block.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletenessWarning {
    /// Source span of the case block for error reporting.
    pub span: Span,
    /// The variable name whose resolved value is not covered.
    pub variable_name: String,
    /// The resolved value that has no matching arm key.
    pub unhandled_value: String,
}

/// Selects the single [`CaseArm`] whose `keys` tuple matches the resolved values of
/// `case_block.variable_names` in `ctx`.
///
/// Matching is exact string equality. For multi-variable case blocks, all variable values
/// must match the corresponding arm key at the same position (zip comparison).
///
/// Returns `Err(ResolveError::NoMatchingArm)` if no arm's key tuple matches the resolved values.
/// Returns `Err(ResolveError::UnresolvedVariable)` if any variable name cannot be resolved.
///
/// Only the selected arm is returned — other arms are not accessible from the result.
pub fn dispatch_case<'a>(
    case_block: &'a CaseBlock,
    ctx: &ResolveContext,
) -> Result<&'a CaseArm, ResolveError> {
    // Resolve all variable values in declaration order.
    let resolved_values: Vec<&str> = case_block
        .variable_names
        .iter()
        .map(|sv| ctx.resolve(&sv.value, &sv.span))
        .collect::<Result<Vec<_>, _>>()?;

    // Find the first arm whose keys all match the resolved values (zip comparison — O(n) over arms).
    let selected = case_block.arms.iter().find(|arm_sw| {
        arm_sw.value.keys.len() == resolved_values.len()
            && arm_sw
                .value
                .keys
                .iter()
                .zip(resolved_values.iter())
                .all(|(key_sw, &resolved)| key_sw.value == resolved)
    });

    match selected {
        Some(arm_sw) => Ok(&arm_sw.value),
        None => {
            let variable_names: Vec<String> = case_block
                .variable_names
                .iter()
                .map(|sv| sv.value.clone())
                .collect();

            let resolved_values_owned: Vec<String> =
                resolved_values.iter().map(|s| s.to_string()).collect();

            let tried_keys: Vec<Vec<String>> = case_block
                .arms
                .iter()
                .map(|arm_sw| {
                    arm_sw.value.keys.iter().map(|k| k.value.clone()).collect()
                })
                .collect();

            Err(ResolveError::NoMatchingArm {
                span: case_block.span.clone(),
                variable_names,
                resolved_values: resolved_values_owned,
                tried_keys,
            })
        }
    }
}

/// Substitutes the `${resource_name}` token in `mapping.template.value` with
/// `resource_instance_name`.
///
/// Uses `str::replace` — not Tera, not HCL parsing. All other content in the template
/// passes through unchanged.
///
/// # Forward reference note
/// // Forward reference detection (ForwardOutputReference error) is deferred to Phase 4;
/// // topological sort will make producer ordering explicit. In Phase 3, a consumer declared
/// // before its producer simply won't find the output variable, producing UnresolvedVariable.
pub fn resolve_output_mapping(mapping: &OutputMapping, resource_instance_name: &str) -> String {
    mapping
        .template
        .value
        .replace("${resource_name}", resource_instance_name)
}

/// Checks whether the resolved value of the single variable in a single-variable `case_block`
/// is covered by at least one arm.
///
/// Returns a [`CompletenessWarning`] for each resolved value that has no matching arm key.
///
/// # Multi-variable case blocks
/// Completeness checking for multi-variable case blocks is deferred to Phase 3 scope limitation:
/// the combinatorial space of tuple values makes exhaustiveness checking non-trivial, and
/// Phase 4's topological sort context will enable more precise analysis. For `variable_names`
/// with more than one element, this function always returns an empty `Vec`.
///
/// # Suppression
/// When `config.completeness_warnings` is `false`, returns an empty `Vec` immediately.
///
/// # Unresolvable variables
/// If `ctx.resolve()` returns an error for the variable (variable not in context), no warning
/// is emitted — a variable that's not present is simply not applicable for completeness checking.
pub fn check_case_completeness(
    case_block: &CaseBlock,
    ctx: &ResolveContext,
    config: &DispatchConfig,
) -> Vec<CompletenessWarning> {
    if !config.completeness_warnings {
        return vec![];
    }

    // Multi-variable completeness checking is deferred to Phase 4.
    // Phase 4's topological sort makes producer ordering explicit, enabling combinatorial
    // exhaustiveness analysis. In Phase 3 we only check single-variable case blocks.
    if case_block.variable_names.len() != 1 {
        return vec![];
    }

    let var = &case_block.variable_names[0];

    // If the variable is not in the context, skip — not applicable.
    let resolved = match ctx.resolve(&var.value, &var.span) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    // Check whether any arm's first key matches the resolved value.
    let has_matching_arm = case_block.arms.iter().any(|arm_sw| {
        arm_sw
            .value
            .keys
            .first()
            .map(|k| k.value == resolved)
            .unwrap_or(false)
    });

    if has_matching_arm {
        vec![]
    } else {
        vec![CompletenessWarning {
            span: case_block.span.clone(),
            variable_name: var.value.clone(),
            unhandled_value: resolved.to_string(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use crate::ast::span::Span;
    use crate::ast::common::Spanned;
    use crate::ast::module::{CaseArm, CaseBlock, OutputMapping};
    use crate::resolver::context::ResolveContext;

    fn test_span() -> Span {
        Span { file: Arc::from("test.gmod"), start_line: 1, start_col: 1, end_line: 1, end_col: 1 }
    }

    fn spanned<T>(value: T) -> Spanned<T> {
        Spanned { value, span: test_span() }
    }

    fn make_arm(keys: Vec<&str>) -> Spanned<CaseArm> {
        Spanned { value: CaseArm {
            span: test_span(),
            keys: keys.iter().map(|k| spanned(k.to_string())).collect(),
            code: spanned(String::new()),
            variables: vec![],
            outputs: vec![],
        }, span: test_span() }
    }

    fn make_ctx(pairs: &[(&str, &str)]) -> ResolveContext {
        let map: HashMap<String, String> = pairs.iter().map(|(k,v)| (k.to_string(), v.to_string())).collect();
        ResolveContext::build(map, HashMap::new(), HashMap::new(), HashMap::new())
    }

    #[test]
    fn test_dispatch_single_variable() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![make_arm(vec!["aws"]), make_arm(vec!["azure"])],
        };
        let ctx = make_ctx(&[("provider", "aws")]);
        let arm = dispatch_case(&case_block, &ctx).unwrap();
        assert_eq!(arm.keys[0].value, "aws");
    }

    #[test]
    fn test_no_matching_arm_error() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![make_arm(vec!["aws"]), make_arm(vec!["azure"])],
        };
        let ctx = make_ctx(&[("provider", "gcp")]);
        let err = dispatch_case(&case_block, &ctx).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("gcp"), "error should mention 'gcp', got: {msg}");
    }

    #[test]
    fn test_dispatch_multi_variable() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![
                spanned("provider".to_string()),
                spanned("engine".to_string()),
            ],
            arms: vec![make_arm(vec!["aws", "aurora"])],
        };
        let ctx = make_ctx(&[("provider", "aws"), ("engine", "aurora")]);
        let arm = dispatch_case(&case_block, &ctx).unwrap();
        assert_eq!(arm.keys[0].value, "aws");
    }

    #[test]
    fn test_dispatch_multi_variable_no_match() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![
                spanned("provider".to_string()),
                spanned("engine".to_string()),
            ],
            arms: vec![make_arm(vec!["aws", "aurora"])],
        };
        let ctx = make_ctx(&[("provider", "aws"), ("engine", "postgres")]);
        let err = dispatch_case(&case_block, &ctx).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("postgres"), "error should mention 'postgres', got: {msg}");
    }

    #[test]
    fn test_only_selected_arm_returned() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![make_arm(vec!["aws"]), make_arm(vec!["azure"])],
        };
        let ctx = make_ctx(&[("provider", "aws")]);
        let arm = dispatch_case(&case_block, &ctx).unwrap();
        assert_eq!(arm.keys[0].value, "aws");
        assert_eq!(arm.keys.len(), 1);
    }

    #[test]
    fn test_resolve_output_mapping() {
        let mapping = OutputMapping {
            span: test_span(),
            name: spanned("vpc_id".to_string()),
            template: spanned("aws_vpc.${resource_name}.id".to_string()),
        };
        let result = resolve_output_mapping(&mapping, "my_network");
        assert_eq!(result, "aws_vpc.my_network.id");
    }

    #[test]
    fn test_resolve_output_mapping_no_token() {
        let mapping = OutputMapping {
            span: test_span(),
            name: spanned("static".to_string()),
            template: spanned("static_string".to_string()),
        };
        let result = resolve_output_mapping(&mapping, "my_network");
        assert_eq!(result, "static_string");
    }

    #[test]
    fn test_completeness_warning_emitted() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![make_arm(vec!["aws"])],
        };
        let ctx = make_ctx(&[("provider", "azure")]);
        let config = DispatchConfig { completeness_warnings: true };
        let warnings = check_case_completeness(&case_block, &ctx, &config);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].variable_name, "provider");
        assert_eq!(warnings[0].unhandled_value, "azure");
    }

    #[test]
    fn test_completeness_warning_suppressed() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![make_arm(vec!["aws"])],
        };
        let ctx = make_ctx(&[("provider", "azure")]);
        let config = DispatchConfig { completeness_warnings: false };
        let warnings = check_case_completeness(&case_block, &ctx, &config);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_completeness_warning_empty_when_arm_matches() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![spanned("provider".to_string())],
            arms: vec![make_arm(vec!["aws"])],
        };
        let ctx = make_ctx(&[("provider", "aws")]);
        let config = DispatchConfig { completeness_warnings: true };
        let warnings = check_case_completeness(&case_block, &ctx, &config);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn test_completeness_skipped_for_multi_variable() {
        let case_block = CaseBlock {
            span: test_span(),
            variable_names: vec![
                spanned("provider".to_string()),
                spanned("engine".to_string()),
            ],
            arms: vec![make_arm(vec!["aws", "aurora"])],
        };
        let ctx = make_ctx(&[("provider", "azure"), ("engine", "postgres")]);
        let config = DispatchConfig { completeness_warnings: true };
        // Multi-variable completeness checking is deferred to Phase 4 (topological sort context).
        // For now, we skip it and return an empty Vec.
        let warnings = check_case_completeness(&case_block, &ctx, &config);
        assert_eq!(warnings.len(), 0);
    }
}
