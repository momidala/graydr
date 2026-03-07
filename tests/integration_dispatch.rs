use graydr::resolver::dispatch::{dispatch_case, resolve_output_mapping, check_case_completeness, DispatchConfig};
use graydr::resolver::{ResolveContext, ResolveError};
use graydr::ast::span::Span;
use graydr::ast::common::Spanned;
use graydr::ast::module::{CaseArm, CaseBlock, OutputMapping};
use std::collections::HashMap;
use std::sync::Arc;

fn test_span() -> Span {
    Span {
        file: Arc::from("test.gmod"),
        start_line: 1,
        start_col: 1,
        end_line: 1,
        end_col: 1,
    }
}

fn spanned<T>(value: T) -> Spanned<T> {
    Spanned { value, span: test_span() }
}

fn make_arm_with_code(keys: Vec<&str>, code: &str) -> Spanned<CaseArm> {
    Spanned {
        value: CaseArm {
            span: test_span(),
            keys: keys.iter().map(|k| spanned(k.to_string())).collect(),
            code: spanned(code.to_string()),
            variables: vec![],
            outputs: vec![],
        },
        span: test_span(),
    }
}

fn make_arm_with_output(keys: Vec<&str>, output_name: &str, output_template: &str) -> Spanned<CaseArm> {
    Spanned {
        value: CaseArm {
            span: test_span(),
            keys: keys.iter().map(|k| spanned(k.to_string())).collect(),
            code: spanned(String::new()),
            variables: vec![],
            outputs: vec![spanned(OutputMapping {
                span: test_span(),
                name: spanned(output_name.to_string()),
                template: spanned(output_template.to_string()),
            })],
        },
        span: test_span(),
    }
}

fn make_arm(keys: Vec<&str>) -> Spanned<CaseArm> {
    make_arm_with_code(keys, "")
}

fn make_ctx(pairs: &[(&str, &str)]) -> ResolveContext {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    ResolveContext::build(map, HashMap::new(), HashMap::new(), HashMap::new())
}

/// test_output_injection_end_to_end (COMP-02):
/// Dispatches "network" resource, resolves its output vpc_id via resolve_output_mapping,
/// injects the resolved value into "db"'s ResolveContext via the gtpl_overrides layer,
/// then dispatches "db" successfully and asserts vpc_id resolves to the injected value.
#[test]
fn test_output_injection_end_to_end() {
    // --- Step 1: Dispatch "network" resource ---
    let network_case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![spanned("provider".to_string())],
        arms: vec![make_arm_with_output(
            vec!["aws"],
            "vpc_id",
            "aws_vpc.${resource_name}.id",
        )],
    };

    let network_ctx = make_ctx(&[("provider", "aws")]);
    let network_arm = dispatch_case(&network_case_block, &network_ctx)
        .expect("network dispatch should succeed");

    // --- Step 2: Resolve outputs from selected arm ---
    let vpc_id_mapping = &network_arm.outputs[0].value;
    let resolved_vpc_id = resolve_output_mapping(vpc_id_mapping, "network");
    assert_eq!(resolved_vpc_id, "aws_vpc.network.id");

    // --- Step 3: Collect resolved outputs into injection map ---
    let mut injected_outputs: HashMap<String, String> = HashMap::new();
    injected_outputs.insert("vpc_id".to_string(), resolved_vpc_id);

    // --- Step 4: Build "db" context with injected outputs in gtpl_overrides layer ---
    let gmod_defaults: HashMap<String, String> = HashMap::new();
    let mut gtpl_overrides: HashMap<String, String> = HashMap::new();
    gtpl_overrides.extend(injected_outputs);
    let properties_values: HashMap<String, String> =
        [("provider".to_string(), "aws".to_string())].iter().cloned().collect();
    let cli_flags: HashMap<String, String> = HashMap::new();

    let db_ctx = ResolveContext::build(gmod_defaults, gtpl_overrides, properties_values, cli_flags);

    // --- Step 5: Dispatch "db" resource ---
    let db_case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![spanned("provider".to_string())],
        arms: vec![make_arm(vec!["aws"])],
    };

    let _db_arm = dispatch_case(&db_case_block, &db_ctx)
        .expect("db dispatch should succeed with augmented context");

    // --- Step 6: Assert vpc_id resolves correctly in db's context ---
    let span = test_span();
    let resolved = db_ctx.resolve("vpc_id", &span).expect("vpc_id should be resolvable");
    assert_eq!(resolved, "aws_vpc.network.id");
}

/// test_correct_declaration_order_output_injection (COMP-02):
/// Producer resource "network" is declared first, consumer "db" is declared second
/// — the correct order for Phase 3.
///
/// // Phase 3 requires correct declaration order (producer before consumer). When a consumer
/// // is declared before its producer, ctx.resolve() on the output variable returns UnresolvedVariable
/// // (the output has not yet been injected). Phase 4's topological sort will make ordering explicit
/// // and produce a precise ForwardOutputReference error for this case.
#[test]
fn test_correct_declaration_order_output_injection() {
    // --- Step 1: Dispatch "network" producer first ---
    let network_case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![spanned("provider".to_string())],
        arms: vec![make_arm_with_output(
            vec!["aws"],
            "subnet_id",
            "aws_subnet.${resource_name}.id",
        )],
    };

    let network_ctx = make_ctx(&[("provider", "aws")]);
    let network_arm = dispatch_case(&network_case_block, &network_ctx)
        .expect("network dispatch should succeed");

    // --- Step 2: Resolve output ---
    let subnet_id_mapping = &network_arm.outputs[0].value;
    let resolved_subnet_id = resolve_output_mapping(subnet_id_mapping, "network");
    assert_eq!(resolved_subnet_id, "aws_subnet.network.id");

    // --- Step 3: Inject into db's context (producer-before-consumer order is correct) ---
    let gmod_defaults: HashMap<String, String> = HashMap::new();
    let mut gtpl_overrides: HashMap<String, String> = HashMap::new();
    gtpl_overrides.insert("subnet_id".to_string(), resolved_subnet_id.clone());
    let properties_values: HashMap<String, String> =
        [("provider".to_string(), "aws".to_string())].iter().cloned().collect();
    let cli_flags: HashMap<String, String> = HashMap::new();

    let db_ctx = ResolveContext::build(gmod_defaults, gtpl_overrides, properties_values, cli_flags);

    // --- Step 4: Dispatch "db" consumer ---
    let db_case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![spanned("provider".to_string())],
        arms: vec![make_arm(vec!["aws"])],
    };

    let _db_arm = dispatch_case(&db_case_block, &db_ctx)
        .expect("db dispatch should succeed");

    // --- Step 5: Assert subnet_id resolves in db's context ---
    let span = test_span();
    let resolved = db_ctx.resolve("subnet_id", &span).expect("subnet_id should be resolvable");
    assert_eq!(resolved, "aws_subnet.network.id");
}

/// test_only_selected_arm_code_in_result (COMP-03):
/// Dispatching provider=aws on a module with aws and azure arms yields only the aws arm's code.
/// The azure arm's code is absent from the result (compile-time selection guarantee).
#[test]
fn test_only_selected_arm_code_in_result() {
    let case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![spanned("provider".to_string())],
        arms: vec![
            make_arm_with_code(
                vec!["aws"],
                "resource \"aws_s3_bucket\" \"main\" { bucket = \"my-bucket\" }",
            ),
            make_arm_with_code(
                vec!["azure"],
                "resource \"azurerm_storage_account\" \"main\" { name = \"mystorage\" }",
            ),
        ],
    };

    let ctx = make_ctx(&[("provider", "aws")]);
    let selected_arm = dispatch_case(&case_block, &ctx)
        .expect("dispatch with provider=aws should succeed");

    // The selected arm's code contains the aws resource
    assert!(
        selected_arm.code.value.contains("aws_s3_bucket"),
        "selected arm code should contain 'aws_s3_bucket', got: {}",
        selected_arm.code.value
    );

    // The selected arm's code does NOT contain the azure resource
    assert!(
        !selected_arm.code.value.contains("azurerm_storage_account"),
        "selected arm code must NOT contain 'azurerm_storage_account' (compile-time arm selection)"
    );
}

/// test_completeness_warning_end_to_end (COMP-08):
/// A case block with one arm (keys=["aws"]) and context provider="gcp" emits a
/// CompletenessWarning for the unhandled value AND dispatch_case still hard-errors
/// with NoMatchingArm.
#[test]
fn test_completeness_warning_end_to_end() {
    let case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![spanned("provider".to_string())],
        arms: vec![make_arm(vec!["aws"])],
    };

    let ctx = make_ctx(&[("provider", "gcp")]);

    // --- Step 1: Check completeness — warning should be emitted ---
    let config = DispatchConfig::default(); // completeness_warnings = true
    let warnings = check_case_completeness(&case_block, &ctx, &config);

    assert_eq!(warnings.len(), 1, "expected exactly one completeness warning");
    assert_eq!(warnings[0].variable_name, "provider");
    assert_eq!(warnings[0].unhandled_value, "gcp");

    // --- Step 2: Dispatch still hard-errors (warning does not suppress the error) ---
    let result = dispatch_case(&case_block, &ctx);
    assert!(
        result.is_err(),
        "dispatch_case should return Err(NoMatchingArm) even when warning was emitted"
    );

    match result.unwrap_err() {
        ResolveError::NoMatchingArm { variable_names, resolved_values, tried_keys, .. } => {
            assert!(variable_names.contains(&"provider".to_string()));
            assert!(resolved_values.contains(&"gcp".to_string()));
            assert!(
                tried_keys.iter().any(|keys| keys.contains(&"aws".to_string())),
                "tried_keys should contain the aws arm keys"
            );
        }
        other => panic!("expected NoMatchingArm, got: {:?}", other),
    }
}

/// test_multi_variable_dispatch_integration (LANG-07):
/// Multi-variable case block with (provider, engine) tuple dispatch:
/// - aws+aurora selects the aurora arm
/// - aws+postgres selects the postgres arm
/// - aws+mysql yields NoMatchingArm with all tried keys listed
#[test]
fn test_multi_variable_dispatch_integration() {
    let case_block = CaseBlock {
        span: test_span(),
        variable_names: vec![
            spanned("provider".to_string()),
            spanned("engine".to_string()),
        ],
        arms: vec![
            make_arm(vec!["aws", "aurora"]),
            make_arm(vec!["aws", "postgres"]),
        ],
    };

    // --- Scenario 1: provider=aws, engine=aurora → selects aws+aurora arm ---
    let ctx_aurora = make_ctx(&[("provider", "aws"), ("engine", "aurora")]);
    let arm_aurora = dispatch_case(&case_block, &ctx_aurora)
        .expect("dispatch aws+aurora should succeed");
    assert_eq!(arm_aurora.keys[0].value, "aws");
    assert_eq!(arm_aurora.keys[1].value, "aurora");

    // --- Scenario 2: provider=aws, engine=postgres → selects aws+postgres arm ---
    let ctx_postgres = make_ctx(&[("provider", "aws"), ("engine", "postgres")]);
    let arm_postgres = dispatch_case(&case_block, &ctx_postgres)
        .expect("dispatch aws+postgres should succeed");
    assert_eq!(arm_postgres.keys[0].value, "aws");
    assert_eq!(arm_postgres.keys[1].value, "postgres");

    // --- Scenario 3: provider=aws, engine=mysql → NoMatchingArm with all tried keys ---
    let ctx_mysql = make_ctx(&[("provider", "aws"), ("engine", "mysql")]);
    let err = dispatch_case(&case_block, &ctx_mysql)
        .expect_err("dispatch aws+mysql should fail with NoMatchingArm");

    match err {
        ResolveError::NoMatchingArm { tried_keys, resolved_values, .. } => {
            // resolved values should include aws and mysql
            assert!(resolved_values.contains(&"aws".to_string()));
            assert!(resolved_values.contains(&"mysql".to_string()));

            // tried_keys should contain both arms
            assert!(
                tried_keys.iter().any(|keys| keys == &vec!["aws".to_string(), "aurora".to_string()]),
                "tried_keys should contain [aws, aurora], got: {:?}", tried_keys
            );
            assert!(
                tried_keys.iter().any(|keys| keys == &vec!["aws".to_string(), "postgres".to_string()]),
                "tried_keys should contain [aws, postgres], got: {:?}", tried_keys
            );
        }
        other => panic!("expected NoMatchingArm, got: {:?}", other),
    }
}
