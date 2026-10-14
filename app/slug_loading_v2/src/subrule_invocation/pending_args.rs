//! GC-traced pending Args snapshots; native action state retains opaque tokens only.
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::environment::Module;
use starlark::starlark_complex_value;
use starlark::values::Coerce;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::starlark_value;

use super::EvaluatorArgsSnapshot;
use super::EvaluatorArgsSnapshotGen;

/// Evaluator-local native phase token; contains no moving Starlark values.
#[derive(Debug, Clone, Allocative)]
pub struct ArgsSnapshotRegistration {
    owner: Arc<()>,
    index: usize,
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
struct RegisteredSnapshotGen<V> {
    #[trace(unsafe_ignore)]
    owner: Arc<()>,
    snapshots: Arc<Mutex<Vec<EvaluatorArgsSnapshotGen<V>>>>,
}
starlark::starlark_complex_value!(RegisteredSnapshot);
unsafe impl<'v> Coerce<RegisteredSnapshotGen<Value<'v>>> for RegisteredSnapshotGen<FrozenValue> {}
impl<V> fmt::Display for RegisteredSnapshotGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<pending Args snapshots>")
    }
}
impl<'v> Freeze for RegisteredSnapshot<'v> {
    type Frozen = FrozenRegisteredSnapshot;
    fn freeze(self, _: &Freezer) -> FreezeResult<Self::Frozen> {
        Err(FreezeError::new(
            "pending Args snapshots must be published before module freeze".to_owned(),
        ))
    }
}
#[starlark_value(type = "pending_args_snapshots")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for RegisteredSnapshotGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenRegisteredSnapshot;
}

/// Register a complete snapshot under its evaluator's GC-traced module root.
pub fn register_args_snapshot<'v>(
    module: &'v Module,
    snapshot: EvaluatorArgsSnapshot<'v>,
) -> anyhow::Result<ArgsSnapshotRegistration> {
    let collector = match crate::evaluator_roots::pending_args_snapshots(module) {
        Some(value) => value,
        None => {
            let value = module.heap().alloc_complex(RegisteredSnapshot {
                owner: Arc::new(()),
                snapshots: Arc::new(Mutex::new(Vec::new())),
            });
            crate::evaluator_roots::set_pending_args_snapshots(module, value);
            value
        }
    };
    let collector = RegisteredSnapshot::from_value(collector)
        .ok_or_else(|| anyhow::anyhow!("invalid pending Args snapshot collector"))?;
    let mut snapshots = collector
        .snapshots
        .lock()
        .map_err(|_| anyhow::anyhow!("pending Args snapshot lock is poisoned"))?;
    let registration = ArgsSnapshotRegistration {
        owner: collector.owner.clone(),
        index: snapshots.len(),
    };
    snapshots.push(snapshot);
    Ok(registration)
}

/// Borrow a collector by module and clone its traced snapshot for synchronous publication.
/// Unknown, stale, cross-module and invalid-index tokens fail closed.
pub fn args_snapshot<'v>(
    module: &'v Module,
    registration: &ArgsSnapshotRegistration,
) -> anyhow::Result<EvaluatorArgsSnapshot<'v>> {
    let collector = crate::evaluator_roots::pending_args_snapshots(module)
        .and_then(RegisteredSnapshot::from_value)
        .ok_or_else(|| anyhow::anyhow!("pending Args snapshot is unavailable"))?;
    anyhow::ensure!(
        Arc::ptr_eq(&collector.owner, &registration.owner),
        "Args snapshot belongs to a different collector"
    );
    collector
        .snapshots
        .lock()
        .map_err(|_| anyhow::anyhow!("pending Args snapshot lock is poisoned"))?
        .get(registration.index)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown Args snapshot registration"))
}

/// Release this evaluator's pending snapshot root after successful native publication.
pub fn clear_args_snapshots(module: &Module) {
    crate::evaluator_roots::set_pending_args_snapshots(module, Value::new_none());
}
