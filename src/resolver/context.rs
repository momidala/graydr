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
