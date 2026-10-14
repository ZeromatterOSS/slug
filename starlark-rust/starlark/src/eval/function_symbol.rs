//! Actual function tokens belong to the current logical evaluation, not its code module.
use std::any::Any;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use dupe::Dupe;

use crate::collections::StarlarkHasher;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::values::Value;
use crate::values::ValueLike;

/// Host-certified immutable identity. Equality/hash must include the complete
/// evaluation domain and its tracked input validity, never only a digest.
pub trait FunctionSymbolOwner: Any + Debug + Send + Sync + Allocative {
    /// Borrow the host's concrete immutable owner for typed publication.
    fn as_any(&self) -> &dyn Any;
    /// Compare complete structural owner and tracked-input identities.
    fn equals(&self, other: &dyn FunctionSymbolOwner) -> bool;
    /// Hash the same fields used by `equals` without allocating.
    fn hash_owner(&self, state: &mut dyn Hasher);
}

#[derive(Debug, Allocative)]
struct TransientOwner(Arc<()>);
impl FunctionSymbolOwner for TransientOwner {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn FunctionSymbolOwner) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| Arc::ptr_eq(&self.0, &other.0))
    }
    fn hash_owner(&self, state: &mut dyn Hasher) {
        state.write_usize(Arc::as_ptr(&self.0) as usize);
    }
}

#[derive(Debug, Allocative)]
struct SessionData {
    owner: Arc<dyn FunctionSymbolOwner>,
    next: AtomicU64,
}

/// One persistent, serial function-creation sequence. Explicit continuations
/// share a clone; independent evaluators start with different transient owners.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct FunctionSymbolSession(Arc<SessionData>);
impl FunctionSymbolSession {
    /// Start a fresh function-only creation sequence for a certified owner.
    pub fn new(owner: Arc<dyn FunctionSymbolOwner>) -> Self {
        Self(Arc::new(SessionData {
            owner,
            next: AtomicU64::new(0),
        }))
    }
    /// This private owner domain is deliberately not a publishable host owner.
    pub fn transient() -> Self {
        Self::new(Arc::new(TransientOwner(Arc::new(()))))
    }
    /// Borrow the shared evaluation owner.
    pub fn owner(&self) -> &Arc<dyn FunctionSymbolOwner> {
        &self.0.owner
    }
    pub(crate) fn started(&self) -> bool {
        self.0.next.load(Ordering::Relaxed) != 0
    }
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(crate) fn next(&self) -> crate::Result<FunctionSymbol> {
        let ordinal = self
            .0
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| {
                crate::Error::new_other(anyhow::anyhow!("function symbol ordinal exhausted"))
            })?;
        Ok(FunctionSymbol(SymbolData {
            owner: self.0.owner.dupe(),
            ordinal,
            exported: OnceLock::new(),
            local_hash_observed: AtomicBool::new(false),
        }))
    }
}

/// Whether successful top-level assignments export actual function tokens.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FunctionExportMode {
    /// Keep local tokens; used outside host-enabled `.bzl` evaluation.
    Disabled,
    /// Export after successful, nonindented assignment and def statements.
    TopLevelAssignments,
}

impl crate::eval::Evaluator<'_, '_, '_> {
    /// Install before function creation, or explicitly share a serial continuation.
    pub fn set_function_symbol_session(
        &mut self,
        session: FunctionSymbolSession,
    ) -> crate::Result<()> {
        if self.call_stack.count() != 0
            || (self.function_symbols.started() && !self.function_symbols.same_session(&session))
        {
            return Err(crate::Error::new_other(anyhow::anyhow!(
                "function symbol session cannot change after function creation or during evaluation"
            )));
        }
        self.function_symbols = session;
        Ok(())
    }
    /// Borrow the persistent session without resetting its creation counter.
    pub fn function_symbol_session(&self) -> &FunctionSymbolSession {
        &self.function_symbols
    }
    /// Select the host's export policy before evaluation creates functions.
    pub fn set_function_export_mode(&mut self, mode: FunctionExportMode) -> crate::Result<()> {
        if self.call_stack.count() != 0
            || (self.function_symbols.started() && self.function_export_mode != mode)
        {
            return Err(crate::Error::new_other(anyhow::anyhow!(
                "function export mode must be set before evaluation"
            )));
        }
        self.function_export_mode = mode;
        Ok(())
    }
}

#[derive(Debug, Allocative)]
struct SymbolData {
    owner: Arc<dyn FunctionSymbolOwner>,
    ordinal: u64,
    exported: OnceLock<Arc<str>>,
    local_hash_observed: AtomicBool,
}

#[derive(Debug, Allocative)]
pub(crate) struct FunctionSymbol(SymbolData);
impl FunctionSymbol {
    pub(crate) fn equals(&self, other: &Self) -> bool {
        self.0.owner.equals(&*other.0.owner)
            && match (self.0.exported.get(), other.0.exported.get()) {
                (Some(a), Some(b)) => a == b,
                (None, None) => self.0.ordinal == other.0.ordinal,
                _ => false,
            }
    }
    pub(crate) fn write_hash(&self, state: &mut StarlarkHasher) {
        self.0.owner.hash_owner(state);
        match self.0.exported.get() {
            Some(name) => {
                1u8.hash(state);
                name.hash(state);
            }
            None => {
                self.0.local_hash_observed.store(true, Ordering::Relaxed);
                0u8.hash(state);
                self.0.ordinal.hash(state);
            }
        }
    }
    fn export(&self, session: &FunctionSymbolSession, name: &str) -> crate::Result<()> {
        if !self.0.owner.equals(&**session.owner()) || self.0.exported.get().is_some() {
            return Ok(());
        }
        if self.0.local_hash_observed.load(Ordering::Relaxed) {
            return Err(crate::Error::new_other(anyhow::anyhow!(
                "exporting a local function after hashing is unsupported"
            )));
        }
        let _ = self.0.exported.set(Arc::from(name));
        Ok(())
    }
}

/// Current token identity, independent of the function's lexical module.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FunctionSymbolStateRef<'a> {
    /// An unexported actual source function.
    Local {
        /// Function-only creation index in the current logical evaluation.
        ordinal: u64,
    },
    /// An actual first same-owner top-level export.
    Global {
        /// First binding name; subsequent aliases do not replace it.
        name: &'a str,
    },
}

/// Borrowed actual Def/FrozenDef token facts; inspection has no hash side effects.
#[derive(Clone, Copy)]
pub struct FunctionSymbolInspection<'a>(&'a FunctionSymbol);
impl<'a> FunctionSymbolInspection<'a> {
    /// Borrow the actual creation owner.
    pub fn owner(&self) -> &'a dyn FunctionSymbolOwner {
        &*self.0.0.owner
    }
    /// Borrow the current local ordinal or first export name.
    pub fn state(&self) -> FunctionSymbolStateRef<'a> {
        match self.0.0.exported.get() {
            Some(name) => FunctionSymbolStateRef::Global { name },
            None => FunctionSymbolStateRef::Local {
                ordinal: self.0.0.ordinal,
            },
        }
    }
    /// Borrow the original creation index, including after a same-name export.
    /// This witness does not change exported language equality or hashing.
    pub fn creation_ordinal(&self) -> u64 {
        self.0.0.ordinal
    }
    /// Whether this is the Engine-private, nonpublishable temporary owner domain.
    pub fn is_transient(&self) -> bool {
        self.owner().as_any().is::<TransientOwner>()
    }
    /// Whether hashing observed this function while its token was still local.
    pub fn local_hash_observed(&self) -> bool {
        self.0.0.local_hash_observed.load(Ordering::Relaxed)
    }
}

fn symbol<'v>(value: Value<'v>) -> Option<&'v FunctionSymbol> {
    if let Some(function) = value.downcast_ref::<Def>() {
        Some(&function.symbol)
    } else {
        value
            .downcast_ref::<FrozenDef>()
            .map(|function| &function.symbol)
    }
}
/// Typed, allocation-free and hash-free inspection of actual Def/FrozenDef only.
pub fn inspect_function_symbol<'v>(value: Value<'v>) -> Option<FunctionSymbolInspection<'v>> {
    symbol(value).map(FunctionSymbolInspection)
}
/// Recognize only actual Engine Def/FrozenDef values, without hashing them.
pub fn is_starlark_def(value: Value<'_>) -> bool {
    symbol(value).is_some()
}
pub(crate) fn export_function(
    value: Value<'_>,
    session: &FunctionSymbolSession,
    name: &str,
) -> crate::Result<()> {
    if let Some(symbol) = symbol(value) {
        symbol.export(session, name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
