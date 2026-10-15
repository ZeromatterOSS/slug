//! Borrow completed child/environment mapping pairs in original encounter order.
pub(crate) use super::mapping::bank::Bank as Inherited;
use super::*;
use crate::bzl_environment::PreparedBzlEnvironment;
fn context<'a>(
    context: &'a crate::BzlModuleSourceContext,
    bank: &mut Inherited<'a>,
    work: &mut Budget,
) -> Result<(), String> {
    work.charge(2, std::mem::size_of::<&[mapping::MappingPair]>())?;
    let mut pairs = context.certified_sources().mapping_pairs().iter();
    loop {
        work.charge(1, 0)?;
        let Some(pair) = pairs.next() else { break };
        bank.inherit(pair, work)?;
    }
    Ok(())
}
pub(crate) fn prepare<'a>(
    capacity: usize,
    rows: usize,
    children: impl Iterator<Item = &'a crate::BzlModuleSourceContext>,
    env: &'a PreparedBzlEnvironment,
    work: &mut Budget,
) -> Result<Inherited<'a>, String> {
    let mut bank = Inherited::new(capacity, rows, work)?;
    let mut children = children;
    loop {
        work.charge(2, std::mem::size_of::<&crate::BzlModuleSourceContext>())?;
        let Some(child) = children.next() else { break };
        context(child, &mut bank, work)?;
    }
    work.charge(1, 0)?;
    let mut modules = env.modules();
    loop {
        work.charge(3, 2 * std::mem::size_of::<usize>())?;
        let Some(module) = modules.next() else { break };
        let source = module
            .module
            .client_data()
            .and_then(|d| crate::starlark_label::defining_module_source_context(d.to_value()))
            .ok_or("original environment module has no effective source context")?;
        context(source, &mut bank, work)?;
    }
    Ok(bank)
}
