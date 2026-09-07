//! Stable public entry point for `PassoFlow`'s local automation contracts.
//!
//! The umbrella crate deliberately exposes the published, platform-independent
//! scenario contract first. Platform adapters and the execution engine remain
//! separate crates until their public interfaces are stable.

#![forbid(unsafe_code)]

pub use passoflow_core::*;

/// Return the version of the shared `PassoFlow` action/event contract.
#[must_use]
pub const fn contract_version() -> &'static str {
    CONTRACT_VERSION
}

#[cfg(test)]
mod tests {
    use super::{Scenario, contract_version};

    #[test]
    fn exposes_the_stable_contract_through_the_umbrella_crate() {
        let scenario = Scenario::from_yaml("steps: []\n").expect("valid scenario");
        assert!(
            scenario
                .validate()
                .iter()
                .all(|diagnostic| { diagnostic.severity == passoflow_core::Severity::Warning })
        );
        assert_eq!(contract_version(), "0.1");
    }
}
