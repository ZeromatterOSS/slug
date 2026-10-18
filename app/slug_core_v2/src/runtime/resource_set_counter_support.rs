//! Closed, default-off test instrumentation. No callback/spec injection API.
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::actions::ResourceEstimate;
use slug_build_api_v2::actions::ResourceSetError;
use slug_build_api_v2::actions::ResourceSetEvaluationContext;
use slug_build_api_v2::actions::ResourceSetInputs;
use slug_build_api_v2::actions::ResourceSetRuntime;
use slug_build_api_v2::actions::RetainedSpawnResourceSet;

use super::BuildCommandEvaluation;
use crate::runtime::configured_action_closure::ValidatedActionClosure;

#[derive(Debug)]
struct CountingRuntime {
    original: RetainedSpawnResourceSet,
    calls: Arc<AtomicUsize>,
}
impl ResourceSetRuntime for CountingRuntime {
    fn evaluate(
        &self,
        inputs: ResourceSetInputs,
        context: &dyn ResourceSetEvaluationContext,
    ) -> Result<ResourceEstimate, ResourceSetError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.original.evaluate(inputs, context)
    }
}

// Only the two private DTO-owner children supply coordinates obtained from
// their existing selection/plan. No caller-supplied replacement is accepted.
pub(super) fn instrument(
    original: &BuildCommandEvaluation,
    selected: &BTreeSet<(usize, usize)>,
    calls: &Arc<AtomicUsize>,
) -> Result<BuildCommandEvaluation, String> {
    if selected.is_empty() {
        return Err("counter requires an existing resource callback".into());
    }
    let mut remaining = selected.len();
    let mut owners = Vec::with_capacity(original.action_closure.owners().len());
    for (owner_index, owner) in original.action_closure.owners().iter().enumerate() {
        if !selected.iter().any(|(index, _)| *index == owner_index) {
            owners.push(owner.clone());
            continue;
        }
        let mut specs = Vec::with_capacity(owner.actions().len());
        let mut contexts = BTreeMap::new();
        for (action_index, action) in owner.actions().iter().enumerate() {
            let context = action.context();
            if let Some(previous) = contexts.insert(context.exec_group().clone(), context.clone()) {
                if !Arc::ptr_eq(&previous, context) {
                    return Err("counter encountered foreign contexts for one exec group".into());
                }
            }
            let spec: &ActionSpec = action;
            let mut copy = spec.clone();
            if selected.contains(&(owner_index, action_index)) {
                let spawn = action
                    .spawn_spec()
                    .ok_or("counter selected a non-Spawn action")?;
                let resource = spawn
                    .resource_set()
                    .ok_or("counter selected no resource callback")?;
                let wrapped = RetainedSpawnResourceSet::from_published(
                    resource.published_callable().clone(),
                    Arc::new(CountingRuntime {
                        original: resource.clone(),
                        calls: calls.clone(),
                    }),
                )
                .map_err(|error| error.to_string())?;
                if resource != &wrapped
                    || !resource
                        .published_callable()
                        .same_original(wrapped.published_callable())
                {
                    return Err("counter changed published resource identity".into());
                }
                copy = ActionSpec::spawn(spawn.clone().with_resource_set(wrapped))
                    .with_exec_properties(spec.exec_properties().clone());
                if let Some(group) = spec.exec_group() {
                    copy = copy.with_exec_group(group);
                }
                if copy != *spec {
                    return Err("counter changed action semantics".into());
                }
                remaining -= 1;
            }
            specs.push(copy);
        }
        let replacement = owner
            .as_ref()
            .clone()
            .with_action_specs(specs, contexts.into_values().collect())?;
        if replacement != **owner || replacement.actions().len() != owner.actions().len() {
            return Err("counter changed configured owner semantics".into());
        }
        for (old, new) in owner.actions().iter().zip(replacement.actions()) {
            if old != new || !Arc::ptr_eq(old.context(), new.context()) {
                return Err("counter changed action order or actual context".into());
            }
        }
        owners.push(Arc::new(replacement));
    }
    if remaining != 0 {
        return Err("counter selection was absent from original closure".into());
    }
    let closure =
        ValidatedActionClosure::new(owners.into()).map_err(|error| format!("{error:?}"))?;
    for (owner_index, owner) in original.action_closure.owners().iter().enumerate() {
        for action_index in 0..owner.actions().len() {
            if original
                .action_closure
                .execution_representative(owner_index, action_index)
                != closure.execution_representative(owner_index, action_index)
            {
                return Err("counter changed execution representative".into());
            }
        }
    }
    let mut result = original.clone();
    result.action_closure = closure;
    if result != *original {
        return Err("counter changed build evaluation semantics".into());
    }
    Ok(result)
}
