use std::collections::HashMap;
use serde_yaml_ng::Value;
use crate::resolver::error::ResolveError;

pub fn deep_merge(_dst: &mut Value, _src: Value) {
    todo!("implemented in plan 02-02")
}

pub fn flatten_to_dotted(_value: Value, _prefix: &str, _out: &mut HashMap<String, String>) {
    todo!("implemented in plan 02-02")
}

pub fn load_properties_file(_path: &str) -> Result<HashMap<String, String>, ResolveError> {
    todo!("implemented in plan 02-02")
}

pub fn parse_cli_flag(_flag: &str) -> Option<(String, String)> {
    todo!("implemented in plan 02-02")
}
