//! The one evaluator-local registration ledger; neither tokens nor owners own it.
use std::mem::size_of;
use std::sync::Mutex;

use allocative::Allocative;
use starlark::values::Trace;
use starlark::values::Tracer;

use super::ResourceSetRegistration;
use crate::bzl_module::semantic_evaluation::budget::Budget;

pub(crate) const INITIAL_WORK: usize = 16;
pub(crate) const REGISTER_WORK: usize = 8;
pub(crate) const TRANSFER_WORK: usize = 8;
pub(crate) const LOOKUP_WORK: usize = 8;
#[derive(Debug, Clone, Copy, Allocative)]
pub(crate) enum Phase {
    Registering,
    Transferring,
    Transferred,
    Poisoned,
}
#[derive(Debug, Allocative, Trace)]
pub(crate) struct Registry<V> {
    pub(super) values: Vec<V>,
    #[trace(unsafe_ignore)]
    pub(super) work: Budget,
    #[trace(unsafe_ignore)]
    pub(super) vector_credit: usize,
    #[trace(unsafe_ignore)]
    pub(super) phase: Phase,
}
impl<V> Registry<V> {
    pub(super) fn pay(&mut self, records: usize, bytes: usize) -> Result<(), String> {
        if !matches!(self.phase, Phase::Registering) {
            self.phase = Phase::Poisoned;
            return Err("resource_set registration ledger is closed or poisoned".into());
        }
        self.work.charge(records, bytes).map_err(|error| {
            self.phase = Phase::Poisoned;
            self.work.context("resource_set registration", error)
        })
    }
    pub(super) fn prepare_push(&mut self) -> anyhow::Result<()> {
        self.pay(REGISTER_WORK, size_of::<ResourceSetRegistration>())
            .map_err(anyhow::Error::msg)?;
        let forecast = self
            .values
            .len()
            .checked_add(1)
            .and_then(|n| n.max(4).checked_mul(4))
            .and_then(|n| n.checked_mul(size_of::<V>()))
            .and_then(|n| n.checked_add(size_of::<Vec<V>>()));
        let Some(forecast) = forecast else {
            self.phase = Phase::Poisoned;
            anyhow::bail!("resource_set registration vector overflow");
        };
        let additional = forecast
            .checked_sub(self.vector_credit)
            .expect("monotone vector envelope");
        self.pay(1, additional).map_err(anyhow::Error::msg)?;
        self.vector_credit = forecast;
        Ok(())
    }
}
/// Inline in the existing traced Args collector; no second Arc or root slot.
#[derive(Debug, Allocative)]
pub(crate) struct ResourceRegistry<V> {
    pub(super) registry: Mutex<Registry<V>>,
}
// GC has exclusive mutable access. No native lock spans inspection, GC-capable
// allocation or evaluation. Recover poison ONLY to trace every retained root.
unsafe impl<'v, V: Trace<'v>> Trace<'v> for ResourceRegistry<V> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.registry
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .trace(tracer);
    }
}
impl<V> ResourceRegistry<V> {
    pub(crate) fn entry() -> Result<Budget, String> {
        let mut entry = Budget::default();
        entry.charge(2, 0)?;
        Ok(entry)
    }
    pub(crate) fn new(mut work: Budget, collector_bytes: usize) -> Result<Self, String> {
        let bytes = collector_bytes
            .checked_add(size_of::<Registry<V>>())
            .and_then(|n| n.checked_add(size_of::<ResourceSetRegistration>()))
            .and_then(|n| n.checked_add(16 * size_of::<V>()))
            .and_then(|n| n.checked_add(32 * size_of::<usize>()))
            .ok_or("resource_set collector layout overflow")?;
        work.charge(INITIAL_WORK, bytes)
            .map_err(|error| work.context("resource_set collector", error))?;
        Ok(Self {
            registry: Mutex::new(Registry {
                values: Vec::new(),
                work,
                vector_credit: 0,
                phase: Phase::Registering,
            }),
        })
    }
    pub(crate) fn pay_entry(&self, entry: Budget) -> Result<(), String> {
        self.pay(entry.records, entry.bytes)
    }
    pub(crate) fn pay(&self, records: usize, bytes: usize) -> Result<(), String> {
        self.registry
            .lock()
            .map_err(|_| "resource_set collector lock is poisoned")?
            .pay(records, bytes)
    }
    /// Isolated host-test teardown, never a production spent-ledger reset route.
    pub(crate) fn ensure_clearable(&self) -> anyhow::Result<()> {
        let state = self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("resource_set collector lock is poisoned"))?;
        anyhow::ensure!(
            matches!(state.phase, Phase::Registering) && state.values.is_empty(),
            "cannot clear live, transferred or poisoned resource_set roots"
        );
        Ok(())
    }
}
