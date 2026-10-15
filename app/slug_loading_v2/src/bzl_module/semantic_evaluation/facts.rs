//! Flat exact source inputs: equality never recursively expands a load diamond.
use std::hash::Hash;
use std::hash::Hasher;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct StableKey {
    pub(crate) identity: BzlModuleIdentity,
    pub(crate) source_name: CompactString,
    pub(crate) context: BzlModuleContext,
    pub(crate) environment: Arc<crate::bzl_environment::BzlEnvironmentInputs>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct Binding {
    pub(crate) raw: Arc<str>,
    pub(crate) child: Arc<StableKey>,
}
#[derive(Debug, Allocative)]
pub(crate) struct LocalFacts {
    pub(crate) key: Arc<StableKey>,
    pub(crate) source: Arc<[u8]>,
    pub(crate) bindings: Arc<[Binding]>,
    pub(crate) budget: budget::Budget,
    pub(crate) comparison: budget::Budget,
}
impl PartialEq for LocalFacts {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.source == other.source && self.bindings == other.bindings
    }
}
impl Eq for LocalFacts {}
impl Hash for LocalFacts {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.key.hash(h);
        self.source.hash(h);
        self.bindings.hash(h);
    }
}

fn row_budget_impl<'a>(
    identity: &BzlModuleIdentity,
    source_name: &str,
    source: &[u8],
    environment: &crate::bzl_environment::PreparedBzlEnvironment,
    children: impl Iterator<Item = (&'a str, &'a StableKey)>,
    complete: bool,
) -> Result<budget::Budget, String> {
    let context = PeakContext::new(
        PeakOwner::SemanticInputs,
        if complete {
            PeakPhase::CompleteRowForecast
        } else {
            PeakPhase::LocalRowForecast
        },
        if complete {
            FirstLedger::Work
        } else {
            FirstLedger::Local
        },
    );
    let mut b = budget::Budget::default();
    b.charge_with_context(
        context,
        1,
        std::mem::size_of::<LocalFacts>()
            + std::mem::size_of::<StableKey>()
            + 4 * std::mem::size_of::<usize>(),
    )?;
    b.identity_with_context(context, identity)?;
    let env = if complete {
        environment.inputs().budget()
    } else {
        budget::Budget {
            records: 1,
            bytes: std::mem::size_of::<Arc<crate::bzl_environment::BzlEnvironmentInputs>>(),
            depth: 0,
        }
    };
    b.charge_with_context(context, env.records, env.bytes)
        .map_err(|e| b.context("semantic local root environment", e))?;
    b.charge_with_context(
        context,
        1,
        source_name
            .len()
            .saturating_add(source.len())
            .saturating_add(2 * std::mem::size_of::<usize>()),
    )?;
    // Reserve HEAD's parser-owned source even when the inherited live parser
    // shares bytes. Equal complete inputs keep identical event admission; the
    // owned projection copies this String only on an actual semantic miss.
    reserve_parser_source_copy(&mut b, source.len(), context)?;
    let mut count = 0usize;
    for (raw, child) in children {
        b.charge_with_context(
            context,
            1,
            raw.len().saturating_add(2 * std::mem::size_of::<usize>()),
        )?;
        // Derived Binding Eq/Hash visits the COMPLETE stable key at every
        // ordered occurrence, even when the immutable child Arc is shared.
        // Logical admission must forecast that repeated work before any copy;
        // actual payload sharing is only a separate physical-retention fact.
        b.charge_with_context(
            context,
            1,
            std::mem::size_of::<StableKey>().saturating_add(child.source_name.len()),
        )?;
        b.identity_with_context(context, &child.identity)?;
        let env = if complete {
            child.environment.budget()
        } else {
            budget::Budget {
                records: 1,
                bytes: std::mem::size_of::<Arc<crate::bzl_environment::BzlEnvironmentInputs>>(),
                depth: 0,
            }
        };
        b.charge_with_context(context, env.records, env.bytes)
            .map_err(|e| b.context("semantic local ordered child environment", e))?;
        count = count.checked_add(1).ok_or_else(budget::limit)?;
    }
    b.array_with_context::<Binding>(context, count)?;
    Ok(b)
}

pub(crate) fn row_budget<'a>(
    identity: &BzlModuleIdentity,
    source_name: &str,
    source: &[u8],
    environment: &crate::bzl_environment::PreparedBzlEnvironment,
    children: impl Iterator<Item = (&'a str, &'a StableKey)>,
) -> Result<budget::Budget, String> {
    row_budget_impl(identity, source_name, source, environment, children, false)
}
pub(crate) fn comparison_budget<'a>(
    identity: &BzlModuleIdentity,
    source_name: &str,
    source: &[u8],
    environment: &crate::bzl_environment::PreparedBzlEnvironment,
    children: impl Iterator<Item = (&'a str, &'a StableKey)>,
) -> Result<budget::Budget, String> {
    row_budget_impl(identity, source_name, source, environment, children, true)
}
pub(crate) fn key_fields_eq(a: &StableKey, b: &StableKey) -> bool {
    a.identity == b.identity && a.source_name == b.source_name && a.context == b.context
}
pub(crate) fn hash_key_fields<H: Hasher>(a: &StableKey, h: &mut H) {
    a.identity.hash(h);
    a.source_name.hash(h);
    a.context.hash(h);
}
pub(crate) fn local_fields_eq(a: &LocalFacts, b: &LocalFacts) -> bool {
    key_fields_eq(&a.key, &b.key)
        && a.source == b.source
        && a.bindings.len() == b.bindings.len()
        && a.bindings
            .iter()
            .zip(b.bindings.iter())
            .all(|(a, b)| a.raw == b.raw && key_fields_eq(&a.child, &b.child))
}
pub(crate) fn hash_local_fields<H: Hasher>(a: &LocalFacts, h: &mut H) {
    hash_key_fields(&a.key, h);
    a.source.hash(h);
    a.bindings.len().hash(h);
    for b in &*a.bindings {
        b.raw.hash(h);
        hash_key_fields(&b.child, h);
    }
}

pub(crate) fn reserve_parser_source_copy(
    budget: &mut budget::Budget,
    source_len: usize,
    context: PeakContext,
) -> Result<(), String> {
    budget.charge_with_context(
        context,
        0,
        source_len.saturating_add(std::mem::size_of::<String>()),
    )
}
