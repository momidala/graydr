use std::collections::HashMap;
use crate::ast::span::Span;
use crate::resolver::error::ResolveError;

pub struct ResolveContext {
    values: HashMap<String, String>,
}

impl ResolveContext {
    pub fn build(
        gmod_defaults: HashMap<String, String>,
        gtpl_overrides: HashMap<String, String>,
        properties_values: HashMap<String, String>,
        cli_flags: HashMap<String, String>,
    ) -> Self {
        let mut values = gmod_defaults;
        values.extend(gtpl_overrides);
        values.extend(properties_values);
        values.extend(cli_flags);
        ResolveContext { values }
    }

    pub fn resolve<'a>(&'a self, name: &str, span: &Span) -> Result<&'a str, ResolveError> {
        self.values
            .get(name)
            .map(|s| s.as_str())
            .ok_or_else(|| ResolveError::UnresolvedVariable {
                name: name.to_string(),
                span: span.clone(),
            })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    pub fn all_values(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_span() -> Span {
        Span {
            file: Arc::from("test.gmod"),
            start_line: 5,
            start_col: 3,
            end_line: 5,
            end_col: 10,
        }
    }

    fn make_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn test_priority_order() {
        let ctx = ResolveContext::build(
            make_map(&[("provider", "from_gmod")]),
            make_map(&[("provider", "from_gtpl")]),
            make_map(&[("provider", "from_props")]),
            make_map(&[("provider", "from_cli")]),
        );
        assert_eq!(ctx.resolve("provider", &test_span()).unwrap(), "from_cli");
    }

    #[test]
    fn test_gtpl_beats_gmod() {
        let ctx = ResolveContext::build(
            make_map(&[("size", "S")]),
            make_map(&[("size", "XL")]),
            make_map(&[]),
            make_map(&[]),
        );
        assert_eq!(ctx.resolve("size", &test_span()).unwrap(), "XL");
    }

    #[test]
    fn test_properties_beat_gtpl() {
        let ctx = ResolveContext::build(
            make_map(&[("region", "a")]),
            make_map(&[("region", "b")]),
            make_map(&[("region", "c")]),
            make_map(&[]),
        );
        assert_eq!(ctx.resolve("region", &test_span()).unwrap(), "c");
    }

    #[test]
    fn test_missing_variable_error() {
        let ctx = ResolveContext::build(
            make_map(&[]),
            make_map(&[]),
            make_map(&[]),
            make_map(&[]),
        );
        let span = test_span();
        let err = ctx.resolve("missing_var", &span).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("missing_var"), "error should contain variable name, got: {msg}");
        assert!(msg.contains("test.gmod:5:3"), "error should contain file:line:col, got: {msg}");
    }

    #[test]
    fn test_contains() {
        let ctx = ResolveContext::build(
            make_map(&[("key", "val")]),
            make_map(&[]),
            make_map(&[]),
            make_map(&[]),
        );
        assert!(ctx.contains("key"));
        assert!(!ctx.contains("absent"));
    }

    #[test]
    fn test_all_values() {
        let ctx = ResolveContext::build(
            make_map(&[("a", "1"), ("b", "2")]),
            make_map(&[]),
            make_map(&[]),
            make_map(&[]),
        );
        let collected: HashMap<&str, &str> = ctx.all_values().collect();
        assert_eq!(collected.get("a"), Some(&"1"));
        assert_eq!(collected.get("b"), Some(&"2"));
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn test_extract_region_mapping_basic() {
        let ctx = ResolveContext::build(
            make_map(&[]),
            make_map(&[]),
            make_map(&[
                ("region_mapping.us-east", "us-east-1"),
                ("region_mapping.eu-west", "eu-west-1"),
            ]),
            make_map(&[]),
        );
        let mapping = ctx.extract_region_mapping();
        assert_eq!(mapping.get("us-east"), Some(&"us-east-1".to_string()));
        assert_eq!(mapping.get("eu-west"), Some(&"eu-west-1".to_string()));
        assert_eq!(mapping.len(), 2);
    }

    #[test]
    fn test_extract_region_mapping_empty() {
        let ctx = ResolveContext::build(
            make_map(&[("provider", "aws")]),
            make_map(&[]),
            make_map(&[]),
            make_map(&[]),
        );
        let mapping = ctx.extract_region_mapping();
        assert!(mapping.is_empty());
    }

    #[test]
    fn test_extract_region_mapping_ignores_other_keys() {
        let ctx = ResolveContext::build(
            make_map(&[]),
            make_map(&[]),
            make_map(&[
                ("region_mapping.us-east", "us-east-1"),
                ("provider", "aws"),
            ]),
            make_map(&[]),
        );
        let mapping = ctx.extract_region_mapping();
        assert_eq!(mapping.get("us-east"), Some(&"us-east-1".to_string()));
        assert_eq!(mapping.len(), 1, "only region_mapping.* keys should be extracted");
    }
}
