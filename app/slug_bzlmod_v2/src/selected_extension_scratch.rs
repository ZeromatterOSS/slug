//! Bounded first-process input data. Only a normal Loading producer grants provenance.
//! These builders perform no I/O and do not mint a native source capability.

mod budget;
mod builder;
#[cfg(test)]
mod tests;
mod types;

pub use budget::SelectedScratchPayloadBudget;
pub use budget::SelectedScratchResources;
pub use builder::SelectedScratchPrefixBuilder;
pub use types::*;
