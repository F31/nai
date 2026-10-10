//! Cost estimate: per component, or explicitly unknown.
//!
//! A component that cannot be estimated is [`Component::Unknown`], never
//! `Known(0)`. Zero is a claim — "this part is free" — that can be checked and
//! falsified; "we cannot tell" is the truth that can be supported. The total
//! follows the same rule: a partial sum is reported as a floor with the number
//! of unknown components attached, never as a whole. O05's "accurate listing" at
//! this layer means the components that *can* be counted are, and the ones that
//! cannot are not dressed up as counted.
use crate::manifest::{CostSpec, UpgradePackage};

/// One line of the estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// An amount in the estimate's currency, in minor units.
    Known(u64),
    /// Cannot be estimated; the token says why.
    Unknown(&'static str),
}
impl Component {
    pub fn amount(self) -> Option<u64> {
        match self {
            Self::Known(amount) => Some(amount),
            Self::Unknown(_) => None,
        }
    }
}

/// The total, which is either whole or explicitly short of whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Total {
    Complete(u64),
    /// `floor` is the sum of the known components and `unknown` counts the ones
    /// left out. Presenting `floor` alone as "the cost" would be the same
    /// not-available violation the components avoid, one level up.
    Partial {
        floor: u64,
        unknown: usize,
    },
}

/// What an upgrade is declared to cost, per component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostEstimate {
    pub currency: String,
    pub cloud: Component,
    pub download: Component,
    pub storage: Component,
    pub runtime_delta: Component,
}

impl CostEstimate {
    /// Read a package's declared cost. A component the manifest leaves out is
    /// `Unknown`, not zero.
    pub fn from_package(package: &UpgradePackage) -> Self {
        Self::from_spec(&package.cost)
    }

    pub fn from_spec(spec: &CostSpec) -> Self {
        Self {
            currency: spec.currency.clone(),
            cloud: component(spec.cloud),
            download: component(spec.download),
            storage: component(spec.storage),
            runtime_delta: component(spec.runtime_delta),
        }
    }

    /// The four components in report order.
    pub fn components(&self) -> [(&'static str, Component); 4] {
        [
            ("cloud", self.cloud),
            ("download", self.download),
            ("storage", self.storage),
            ("runtime_delta", self.runtime_delta),
        ]
    }

    /// Sum what is known and be explicit about what is not.
    pub fn total(&self) -> Total {
        let mut floor = 0u64;
        let mut unknown = 0usize;
        for (_, component) in self.components() {
            match component {
                Component::Known(amount) => floor = floor.saturating_add(amount),
                Component::Unknown(_) => unknown += 1,
            }
        }
        if unknown == 0 {
            Total::Complete(floor)
        } else {
            Total::Partial { floor, unknown }
        }
    }
}

fn component(amount: Option<u64>) -> Component {
    match amount {
        Some(amount) => Component::Known(amount),
        None => Component::Unknown("not declared by the package manifest"),
    }
}
