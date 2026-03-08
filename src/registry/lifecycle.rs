/// Lifecycle state of a published module.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleState {
    Beta,
    Active,
    Deprecated,
    Retired,
}

impl LifecycleState {
    /// Returns true if this lifecycle state blocks new use of the module.
    pub fn blocks_new_use(&self) -> bool {
        matches!(self, LifecycleState::Retired)
    }

    /// Parse a lifecycle state from a string. Unknown values default to Active.
    pub fn from_str(s: &str) -> Self {
        match s {
            "beta" => Self::Beta,
            "deprecated" => Self::Deprecated,
            "retired" => Self::Retired,
            _ => Self::Active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_retired_blocks_new_use() {
        assert!(LifecycleState::Retired.blocks_new_use());
    }

    #[test]
    #[ignore]
    fn test_deprecated_does_not_block() {
        assert!(!LifecycleState::Deprecated.blocks_new_use());
    }

    #[test]
    #[ignore]
    fn test_from_str_retired() {
        assert_eq!(LifecycleState::from_str("retired"), LifecycleState::Retired);
    }

    #[test]
    #[ignore]
    fn test_from_str_unknown_defaults_to_active() {
        assert_eq!(LifecycleState::from_str("unknown"), LifecycleState::Active);
    }
}
