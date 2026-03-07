use std::collections::HashMap;
use serde_yaml_ng::Value;
use crate::resolver::error::ResolveError;

pub fn deep_merge(dst: &mut Value, src: Value) {
    match (dst, src) {
        (Value::Mapping(dst_map), Value::Mapping(src_map)) => {
            for (key, src_val) in src_map {
                match dst_map.get_mut(&key) {
                    Some(dst_val) => deep_merge(dst_val, src_val),
                    None => { dst_map.insert(key, src_val); }
                }
            }
        }
        (dst, src) => *dst = src,
    }
}

pub fn flatten_to_dotted(value: Value, prefix: &str, out: &mut HashMap<String, String>) -> Result<(), ResolveError> {
    match value {
        Value::Mapping(map) => {
            for (k, v) in map {
                let key_str = match &k {
                    Value::String(s) => s.clone(),
                    _ => continue, // non-string map key: skip silently
                };
                let new_prefix = if prefix.is_empty() {
                    key_str
                } else {
                    format!("{}.{}", prefix, key_str)
                };
                flatten_to_dotted(v, &new_prefix, out)?;
            }
        }
        Value::String(s) => {
            out.insert(prefix.to_string(), s);
        }
        Value::Bool(b) => {
            out.insert(prefix.to_string(), b.to_string());
        }
        Value::Number(n) => {
            out.insert(prefix.to_string(), n.to_string());
        }
        Value::Null => {
            // skip — null values produce no map entry
        }
        Value::Sequence(_) => {
            return Err(ResolveError::PropertiesLoadError {
                path: String::new(),
                reason: format!(
                    "sequence values are not supported for variable bindings (key: '{}')",
                    prefix
                ),
            });
        }
        Value::Tagged(tagged) => {
            // Treat tagged values by recursing into the value
            flatten_to_dotted(tagged.value, prefix, out)?;
        }
    }
    Ok(())
}

pub fn load_properties_file(path: &str) -> Result<HashMap<String, String>, ResolveError> {
    let content = std::fs::read_to_string(path).map_err(|e| ResolveError::PropertiesLoadError {
        path: path.to_string(),
        reason: e.to_string(),
    })?;

    let value: Value = if path.ends_with(".json") {
        let json_val: serde_json::Value = serde_json::from_str(&content).map_err(|e| ResolveError::PropertiesLoadError {
            path: path.to_string(),
            reason: e.to_string(),
        })?;
        let json_str = serde_json::to_string(&json_val).map_err(|e| ResolveError::PropertiesLoadError {
            path: path.to_string(),
            reason: e.to_string(),
        })?;
        serde_yaml_ng::from_str(&json_str).map_err(|e| ResolveError::PropertiesLoadError {
            path: path.to_string(),
            reason: e.to_string(),
        })?
    } else {
        serde_yaml_ng::from_str(&content).map_err(|e| ResolveError::PropertiesLoadError {
            path: path.to_string(),
            reason: e.to_string(),
        })?
    };

    let mut out = HashMap::new();
    flatten_to_dotted(value, "", &mut out).map_err(|e| match e {
        ResolveError::PropertiesLoadError { reason, .. } => ResolveError::PropertiesLoadError {
            path: path.to_string(),
            reason,
        },
        other => other,
    })?;
    Ok(out)
}

pub fn parse_cli_flag(flag: &str) -> Option<(String, String)> {
    let eq_pos = flag.find('=')?;
    let key = flag[..eq_pos].to_string();
    let value = flag[eq_pos + 1..].to_string();
    Some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_ng::Value;

    fn yaml(s: &str) -> Value {
        serde_yaml_ng::from_str(s).unwrap()
    }

    // ---- deep_merge tests ----

    #[test]
    fn test_deep_merge_simple() {
        let mut dst = yaml("a: 0\nb: 2");
        let src = yaml("a: 1");
        deep_merge(&mut dst, src);
        let m = dst.as_mapping().unwrap();
        assert_eq!(m[&Value::String("a".into())], Value::Number(1.into()));
        assert_eq!(m[&Value::String("b".into())], Value::Number(2.into()));
    }

    #[test]
    fn test_deep_merge_nested() {
        let mut dst = yaml("db:\n  size: S\n  region: us");
        let src = yaml("db:\n  size: XL");
        deep_merge(&mut dst, src);
        let db = dst.as_mapping().unwrap()[&Value::String("db".into())].as_mapping().unwrap().clone();
        assert_eq!(db[&Value::String("size".into())], Value::String("XL".into()));
        assert_eq!(db[&Value::String("region".into())], Value::String("us".into()));
    }

    #[test]
    fn test_deep_merge_sequence_replaces() {
        let mut dst = yaml("list:\n  - a\n  - b");
        let src = yaml("list:\n  - c\n  - d");
        deep_merge(&mut dst, src);
        let list = dst.as_mapping().unwrap()[&Value::String("list".into())].as_sequence().unwrap().clone();
        assert_eq!(list, vec![Value::String("c".into()), Value::String("d".into())]);
    }

    // ---- flatten_to_dotted tests ----

    #[test]
    fn test_flatten_simple() {
        let v = yaml("provider: aws");
        let mut out = HashMap::new();
        flatten_to_dotted(v, "", &mut out).unwrap();
        assert_eq!(out.get("provider"), Some(&"aws".to_string()));
    }

    #[test]
    fn test_flatten_nested() {
        let v = yaml("primary_db:\n  size: XL\n  region: us");
        let mut out = HashMap::new();
        flatten_to_dotted(v, "", &mut out).unwrap();
        assert_eq!(out.get("primary_db.size"), Some(&"XL".to_string()));
        assert_eq!(out.get("primary_db.region"), Some(&"us".to_string()));
    }

    #[test]
    fn test_flatten_null_skipped() {
        let v = yaml("key: ~");
        let mut out = HashMap::new();
        flatten_to_dotted(v, "", &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn test_flatten_sequence_errors() {
        let v = yaml("items:\n  - a\n  - b");
        let mut out = HashMap::new();
        let result = flatten_to_dotted(v, "", &mut out);
        assert!(result.is_err());
        match result.unwrap_err() {
            ResolveError::PropertiesLoadError { .. } => {}
            other => panic!("expected PropertiesLoadError, got {:?}", other),
        }
    }

    #[test]
    fn test_flatten_bool_and_number() {
        let v = yaml("flag: true\ncount: 42");
        let mut out = HashMap::new();
        flatten_to_dotted(v, "", &mut out).unwrap();
        assert_eq!(out.get("flag"), Some(&"true".to_string()));
        assert_eq!(out.get("count"), Some(&"42".to_string()));
    }

    // ---- parse_cli_flag tests ----

    #[test]
    fn test_parse_cli_flag_dotted() {
        assert_eq!(
            parse_cli_flag("primary_db.size=XL"),
            Some(("primary_db.size".to_string(), "XL".to_string()))
        );
    }

    #[test]
    fn test_parse_cli_flag_simple() {
        assert_eq!(
            parse_cli_flag("provider=aws"),
            Some(("provider".to_string(), "aws".to_string()))
        );
    }

    #[test]
    fn test_parse_cli_flag_no_equals() {
        assert_eq!(parse_cli_flag("noequalssign"), None);
    }

    #[test]
    fn test_parse_cli_flag_value_with_equals() {
        assert_eq!(
            parse_cli_flag("key=val=ue"),
            Some(("key".to_string(), "val=ue".to_string()))
        );
    }

    // ---- load_properties_file tests ----

    #[test]
    fn test_load_yaml_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        writeln!(f, "primary_db:").unwrap();
        writeln!(f, "  size: XL").unwrap();
        writeln!(f, "  region: us-east-1").unwrap();
        writeln!(f, "  engine: postgres").unwrap();
        writeln!(f, "provider: aws").unwrap();
        writeln!(f, "environment: production").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let map = load_properties_file(&path).unwrap();
        assert_eq!(map.get("primary_db.size"), Some(&"XL".to_string()));
        assert_eq!(map.get("primary_db.region"), Some(&"us-east-1".to_string()));
        assert_eq!(map.get("primary_db.engine"), Some(&"postgres".to_string()));
        assert_eq!(map.get("provider"), Some(&"aws".to_string()));
        assert_eq!(map.get("environment"), Some(&"production".to_string()));
    }

    #[test]
    fn test_load_json_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        writeln!(f, r#"{{"primary_db": {{"size": "XL", "region": "us-east-1", "engine": "postgres"}}, "provider": "aws", "environment": "production"}}"#).unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let map = load_properties_file(&path).unwrap();
        assert_eq!(map.get("primary_db.size"), Some(&"XL".to_string()));
        assert_eq!(map.get("primary_db.region"), Some(&"us-east-1".to_string()));
        assert_eq!(map.get("primary_db.engine"), Some(&"postgres".to_string()));
        assert_eq!(map.get("provider"), Some(&"aws".to_string()));
        assert_eq!(map.get("environment"), Some(&"production".to_string()));
    }

    #[test]
    fn test_load_missing_file() {
        let result = load_properties_file("/nonexistent/path/file.yaml");
        assert!(result.is_err());
        match result.unwrap_err() {
            ResolveError::PropertiesLoadError { path, .. } => {
                assert_eq!(path, "/nonexistent/path/file.yaml");
            }
            other => panic!("expected PropertiesLoadError, got {:?}", other),
        }
    }

    #[test]
    fn test_two_yaml_files_merge() {
        use std::io::Write;
        let mut f1 = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        writeln!(f1, "provider: aws").unwrap();
        writeln!(f1, "db:").unwrap();
        writeln!(f1, "  size: S").unwrap();

        let mut f2 = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        writeln!(f2, "db:").unwrap();
        writeln!(f2, "  size: XL").unwrap();
        writeln!(f2, "  region: us").unwrap();

        let path1 = f1.path().to_str().unwrap().to_string();
        let path2 = f2.path().to_str().unwrap().to_string();

        let mut merged = Value::Null;
        for path in &[path1, path2] {
            let content = std::fs::read_to_string(path).unwrap();
            let v: Value = serde_yaml_ng::from_str(&content).unwrap();
            if matches!(merged, Value::Null) {
                merged = v;
            } else {
                deep_merge(&mut merged, v);
            }
        }
        let mut out = HashMap::new();
        flatten_to_dotted(merged, "", &mut out).unwrap();

        assert_eq!(out.get("provider"), Some(&"aws".to_string()));
        assert_eq!(out.get("db.size"), Some(&"XL".to_string()));
        assert_eq!(out.get("db.region"), Some(&"us".to_string()));
    }
}
