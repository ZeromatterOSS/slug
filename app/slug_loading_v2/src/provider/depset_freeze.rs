//! Prime actual transitive children before ordinary original-value freezing.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Frame<'v> {
    value: Value<'v>,
    next: usize,
}

fn push<'v>(
    frames: &mut Vec<Frame<'v>>,
    peak: &mut usize,
    value: Value<'v>,
    freezer: &Freezer,
) -> FreezeResult<()> {
    if frames.len() == frames.capacity() {
        let old = frames.capacity();
        let next = old.checked_mul(2).map(|n| n.max(4));
        let bytes = next
            .and_then(|n| old.checked_add(n))
            .and_then(|n| n.checked_mul(std::mem::size_of::<Frame<'v>>()))
            .ok_or_else(|| FreezeError::new("depset freeze scratch overflow".into()))?;
        let new_peak = (*peak).max(bytes);
        freezer.charge_traversal_scratch(new_peak - *peak)?;
        *peak = new_peak;
        frames
            .try_reserve_exact(next.unwrap() - frames.len())
            .map_err(|_| FreezeError::new("depset freeze scratch allocation failed".into()))?;
    }
    frames.push(Frame { value, next: 0 });
    Ok(())
}

fn prime<'v>(
    successors: &[StarlarkDepsetSuccessorGen<Value<'v>>],
    freezer: &Freezer,
) -> FreezeResult<()> {
    let mut frames = Vec::new();
    let mut peak = 0;
    for successor in successors {
        freezer.charge_traversal_work(1)?;
        let StarlarkDepsetSuccessorGen::Transitive(value) = successor else {
            continue;
        };
        if freezer.is_frozen_or_forwarded(*value) {
            continue;
        }
        push(&mut frames, &mut peak, *value, freezer)?;
        while let Some(frame) = frames.last().copied() {
            freezer.charge_traversal_work(1)?;
            // Another direct-value freeze can have forwarded any saved frame.
            // Never borrow its typed payload before checking the header again.
            if freezer.is_frozen_or_forwarded(frame.value) {
                frames.pop();
                continue;
            }
            let next = {
                let value = StarlarkDepset::from_value(frame.value).ok_or_else(|| {
                    FreezeError::new("depset transitive successor is not a depset".into())
                })?;
                let starlark::__macro_refs::Either::Left(value) = value else {
                    unreachable!("frozen children were skipped before typed inspection")
                };
                value.successors.get(frame.next).cloned()
            };
            match next {
                Some(successor) => {
                    frames.last_mut().unwrap().next += 1;
                    if let StarlarkDepsetSuccessorGen::Transitive(value) = successor {
                        if !freezer.is_frozen_or_forwarded(value) {
                            push(&mut frames, &mut peak, value, freezer)?;
                        }
                    }
                }
                None => {
                    frames.pop();
                    // All transitive children are now forwarded. Freeze this same
                    // object shallowly; no typed payload borrow crosses the call.
                    freezer.freeze(frame.value)?;
                }
            }
        }
    }
    Ok(())
}

impl Freeze for StarlarkDepset<'_> {
    type Frozen = FrozenStarlarkDepset;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        freezer.with_traversal(|| {
            prime(&self.successors, freezer)?;
            Ok(StarlarkDepsetGen {
                order: self.order,
                element_type: self.element_type,
                occurrence: self.occurrence,
                retained: self.retained,
                depth: self.depth,
                successors: self.successors.freeze(freezer)?,
            })
        })
    }
}
