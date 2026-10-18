//! General resource callback eligibility and evaluator-local registration.
use starlark::environment::Module;
use starlark::eval::inspect_def;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::typing::StarlarkCallable;

mod pending;
pub(crate) use pending::ResourceRegistry;
pub use pending::ResourceSetRegistration;
pub use pending::register_resource_set;
pub use pending::resource_set_value;
pub use pending::transfer_resource_sets;

/// Additive pre-copy debit for private action payload storage, even resource None.
/// It grants no certificate/credit and uses this invocation's same rooted ledger.
#[doc(hidden)]
pub fn prepay_action_storage(module: &Module, records: usize, bytes: usize) -> anyhow::Result<()> {
    pending::prepay_validation(module, records, bytes)
}

/// Eligibility work uses the invocation module only as its accounting owner;
/// it never substitutes that module for the callback's actual original module.
pub(crate) fn validate_for_invocation<'v>(
    module: &'v Module,
    callback: Value<'v>,
) -> anyhow::Result<()> {
    validate_with_charge(callback, None, |records, bytes| {
        pending::prepay_validation(module, records, bytes)
    })
}
fn validate_with_charge<'v>(
    callback: Value<'v>,
    actual_original_unfrozen_module: Option<&'v Module>,
    mut charge: impl FnMut(usize, usize) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    charge(8, 8 * std::mem::size_of::<usize>())?;
    let callable: Option<StarlarkCallable<'v>> =
        StarlarkCallable::unpack_value(callback).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    anyhow::ensure!(callable.is_some(), "resource_set must be callable or None");
    if let Some(definition) = inspect_def(callback, actual_original_unfrozen_module) {
        charge(2, 0)?;
        let name = definition.name();
        charge(1, name.len())?;
        anyhow::ensure!(
            definition
                .is_global()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            "resource_set Starlark function must be global under its original name"
        );
    }
    Ok(())
}
/// Semantic-only unit-test entry. Production uses the prepaid invocation path.
/// A mutable fallback MUST be the actual defining module; frozen original wins.
#[cfg(test)]
fn validate_resource_set<'v>(
    callback: Value<'v>,
    actual_original_unfrozen_module: Option<&'v Module>,
) -> anyhow::Result<()> {
    validate_with_charge(callback, actual_original_unfrozen_module, |_, _| Ok(()))
}

#[cfg(test)]
mod tests;
