//! Synchronous, allocation-forecasted adapter. Numeric accounting never owns IO
//! authority and never counts internal checkpoints as filesystem demands.
use super::*;

pub(super) fn path_bytes(path: &NormalizedAbsolutePath) -> usize {
    path.retained_storage_bytes().unwrap_or(usize::MAX)
}
pub(super) fn symlink_bytes(links: &[ResolvedSymlink]) -> usize {
    links.iter().fold(0, |sum, link| {
        sum.saturating_add(path_bytes(&link.path))
            .saturating_add(link.target.capacity())
            .saturating_add(64)
    })
}
fn array<T>(count: usize) -> usize {
    count
        .saturating_mul(std::mem::size_of::<T>())
        .saturating_add(64)
}
fn joined_bytes(parent: &NormalizedAbsolutePath, basename: &OsStr) -> usize {
    parent
        .retained_capacity_bytes()
        .saturating_add(basename.len())
        .saturating_add(1)
        .saturating_mul(4)
        .saturating_add(256)
}
fn frame_bytes(frame: &ResolverFrame) -> usize {
    let phase = match &frame.phase {
        ResolverPhase::Begin => 0,
        ResolverPhase::AwaitParent { basename } => basename.len().saturating_add(64),
        ResolverPhase::ReplayParentRoute {
            entries,
            real_child,
            ..
        } => array::<NormalizedAbsolutePath>(entries.capacity())
            .saturating_add(frame.projection_bytes)
            .saturating_add(path_bytes(real_child)),
        ResolverPhase::AwaitLstat { real_path } => path_bytes(real_path),
        ResolverPhase::AwaitReadLink { physical_link, .. } => path_bytes(physical_link),
    };
    path_bytes(&frame.original_path)
        .saturating_add(path_bytes(&frame.current_path))
        .saturating_add(array::<NormalizedAbsolutePath>(
            frame.logical_chain.capacity(),
        ))
        .saturating_add(array::<NormalizedAbsolutePath>(
            frame.sorted_logical_chain.capacity(),
        ))
        .saturating_add(frame.route_bytes.saturating_mul(2))
        .saturating_add(array::<ResolvedSymlink>(frame.symlinks.capacity()))
        .saturating_add(frame.symlink_bytes)
        .saturating_add(phase)
        // Expansion stores a split Arc route while the mutable arrays remain.
        .saturating_add(if frame.ancestor_expansion.is_some() {
            frame
                .route_bytes
                .saturating_add(path_bytes(&frame.current_path))
                .saturating_add(array::<NormalizedAbsolutePath>(
                    frame.logical_chain.len().saturating_add(1),
                ))
                .saturating_add(128)
        } else {
            0
        })
}
fn route_work(frame: &ResolverFrame, candidate_bytes: usize) -> usize {
    // Vec growth and old/new arrays, combined cycle/expansion scratch, split
    // Arcs and completion Arcs may overlap. Full path owners overcount sharing.
    array::<NormalizedAbsolutePath>(frame.logical_chain.len().saturating_add(1))
        .saturating_mul(8)
        .saturating_add(
            frame
                .route_bytes
                .saturating_add(candidate_bytes)
                .saturating_mul(4),
        )
        .saturating_add(
            array::<ResolvedSymlink>(frame.symlinks.len().saturating_add(1)).saturating_mul(4),
        )
        .saturating_add(frame.symlink_bytes.saturating_mul(3))
        .saturating_add(1024)
}
struct Budget {
    live: usize,
    peak: usize,
}
impl Budget {
    fn check<E>(
        &mut self,
        extra: usize,
        check: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        self.peak = self.peak.max(self.live.saturating_add(extra));
        check(self.peak)
    }
    fn replace(&mut self, old: usize, new: usize) {
        self.live = self.live.saturating_sub(old).saturating_add(new);
    }
}
enum Failure<E> {
    Guard(E),
    Allocation,
}

// Preflight the entire projection before allocation; callbacks inside BOTH
// loops also check cancellation for a large parent route with no new IO facts.
fn finish<E>(
    machine: &mut ResolutionMachine,
    result: Result<ResolvedPath, ResolutionCause>,
    check: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<MachineStep>, Failure<E>> {
    if machine.frames.len() < 2 || result.is_err() {
        return Ok(machine.finish_frame(result));
    }
    let caller = &machine.frames[machine.frames.len() - 2];
    let ResolverPhase::AwaitParent { basename } = &caller.phase else {
        unreachable!()
    };
    let parent = result.as_ref().unwrap();
    let parent_links = symlink_bytes(parent.symlinks());
    let mut extra =
        array::<ResolvedSymlink>(caller.symlinks.len().saturating_add(parent.symlinks.len()))
            .saturating_mul(4)
            .saturating_add(parent_links)
            .saturating_add(joined_bytes(parent.real_path(), basename));
    let mut projection = 0usize;
    let directory = matches!(parent.state(),ResolvedPathState::Present(stat) if stat.kind()==PathNodeKind::Directory);
    if directory {
        extra = extra.saturating_add(array::<NormalizedAbsolutePath>(parent.route().len()));
        for path in parent.route() {
            projection = projection.saturating_add(joined_bytes(path, basename));
            check(extra.saturating_add(projection)).map_err(Failure::Guard)?;
        }
    } else {
        extra = extra
            .saturating_add(route_work(caller, 0))
            .saturating_add(parent_links.saturating_mul(3));
    }
    check(extra.saturating_add(projection)).map_err(Failure::Guard)?;
    machine.frames.pop().expect("completed frame");
    let caller = machine.frames.last_mut().unwrap();
    let ResolverPhase::AwaitParent { basename } = &caller.phase else {
        unreachable!()
    };
    let basename = basename.dupe();
    let parent = result.unwrap();
    caller.content_origin = parent.content_origin.dupe();
    caller.symlink_bytes = caller.symlink_bytes.saturating_add(parent_links);
    caller.symlinks.extend(parent.symlinks.iter().cloned());
    let real_child = join_normalized(parent.real_path(), basename.as_ref());
    if !directory {
        return Ok(Some(MachineStep::Complete(Ok(caller.completion(
            machine.namespace,
            real_child,
            ResolvedPathState::Missing,
        )))));
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(parent.route().len())
        .map_err(|_| Failure::Allocation)?;
    // Vec's actual capacity must fit the forecast, not just its requested length.
    if entries.capacity() > parent.route().len() {
        return Err(Failure::Allocation);
    }
    for path in parent.route() {
        check(extra.saturating_add(projection)).map_err(Failure::Guard)?;
        entries.push(join_normalized(path, basename.as_ref()));
    }
    caller.projection_bytes = projection;
    caller.phase = ResolverPhase::ReplayParentRoute {
        entries,
        next: 0,
        real_child,
    };
    Ok(None)
}

pub(super) fn resolve<E>(
    key: &AnchoredResolvedPathKey,
    mut observe: impl FnMut(&PathObservationDemand) -> Result<PathObservationResult, E>,
    mut check: impl FnMut(usize) -> Result<(), E>,
) -> Result<Result<ResolvedPath, AnchoredPathResolutionError>, E> {
    let mut budget = Budget { live: 0, peak: 0 };
    budget.check(
        array::<ResolverFrame>(1)
            .saturating_add(path_bytes(key.logical_path()).saturating_mul(4))
            .saturating_add(1024),
        &mut check,
    )?;
    let mut machine = ResolutionMachine::new(key.anchor().namespace(), key.logical_path().dupe());
    machine.set_anchor(Some(key.anchor().dupe()));
    budget.live = array::<ResolverFrame>(machine.frames.capacity())
        .saturating_add(frame_bytes(&machine.frames[0]));
    let mut next = None;
    loop {
        let step = if let Some(step) = next.take() {
            step
        } else {
            let frame = machine.frames.last().unwrap();
            let old = frame_bytes(frame);
            let candidate = match &frame.phase {
                ResolverPhase::ReplayParentRoute { entries, next, .. } => {
                    entries.get(*next).map_or(0, path_bytes)
                }
                _ => path_bytes(&frame.current_path)
                    .saturating_mul(4)
                    .saturating_add(256),
            };
            budget.check(route_work(frame, candidate), &mut check)?;
            let Some(step) = machine.transition_one() else {
                budget.replace(old, frame_bytes(machine.frames.last().unwrap()));
                continue;
            };
            budget.replace(old, frame_bytes(machine.frames.last().unwrap()));
            step
        };
        match step {
            MachineStep::PushParent(parent) => {
                let old_array = array::<ResolverFrame>(machine.frames.capacity());
                let new_array = array::<ResolverFrame>(machine.frames.len().saturating_add(1))
                    .saturating_mul(4);
                budget.check(
                    new_array
                        .saturating_add(path_bytes(&parent).saturating_mul(4))
                        .saturating_add(512),
                    &mut check,
                )?;
                machine.push_parent(parent);
                budget.replace(
                    old_array,
                    array::<ResolverFrame>(machine.frames.capacity())
                        .saturating_add(frame_bytes(machine.frames.last().unwrap())),
                );
            }
            MachineStep::Observe(demand) => {
                budget.check(256, &mut check)?;
                let result = observe(&demand)?;
                assert_eq!(
                    demand.operation(),
                    result.operation(),
                    "resolver provider operation"
                );
                let frame = machine.frames.last().unwrap();
                let old = frame_bytes(frame);
                let link_target = match &result {
                    PathObservationResult::ReadLink(PathOperationResult::Present(target)) => {
                        (match &frame.phase {
                            ResolverPhase::AwaitReadLink { physical_link, .. } => {
                                physical_link.retained_capacity_bytes()
                            }
                            _ => frame.current_path.retained_capacity_bytes(),
                        })
                        .saturating_add(target.capacity())
                        .saturating_add(1)
                        .saturating_mul(4)
                        .saturating_add(256)
                    }
                    _ => 0,
                };
                budget.check(
                    route_work(frame, link_target).saturating_add(link_target.saturating_mul(2)),
                    &mut check,
                )?;
                next = machine.observe(demand, Arc::new(result));
                budget.replace(old, frame_bytes(machine.frames.last().unwrap()));
            }
            MachineStep::Complete(result) => {
                let frame = machine.frames.last().unwrap();
                let old = frame_bytes(frame);
                // The completed route/links coexist with frame arrays until pop.
                let pending = route_work(frame, 0);
                budget.check(pending, &mut check)?;
                let old_array = array::<ResolverFrame>(machine.frames.capacity());
                let old_parent = machine.frames.iter().rev().nth(1).map_or(0, frame_bytes);
                let completed = finish(&mut machine, result, &mut |extra| {
                    budget.check(pending.saturating_add(extra), &mut check)
                });
                let completed = match completed {
                    Ok(step) => step,
                    Err(Failure::Guard(error)) => return Err(error),
                    Err(Failure::Allocation) => {
                        return Ok(Err(AnchoredPathResolutionError::WorkingStorageRefused));
                    }
                };
                budget.replace(
                    old.saturating_add(old_parent).saturating_add(old_array),
                    array::<ResolverFrame>(machine.frames.capacity())
                        .saturating_add(machine.frames.last().map_or(0, frame_bytes)),
                );
                if let Some(step) = completed {
                    if machine.frames.is_empty() {
                        let MachineStep::Complete(result) = step else {
                            unreachable!()
                        };
                        return Ok(result
                            .map(|mut resolved| {
                                resolved.anchor = Some(key.anchor().dupe());
                                resolved
                            })
                            .map_err(|cause| {
                                anchor::anchored_error(key.anchor(), key.logical_path(), cause)
                            }));
                    }
                    next = Some(step);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn projection() -> (ResolutionMachine, ResolvedPath) {
        let namespace =
            PathObservationNamespace::Materialization(crate::PathObservationInstanceId::new(41));
        let path = |text: String| NormalizedAbsolutePath::new(text).unwrap();
        let mut caller = ResolverFrame::new(
            path("/repo/link/child".into()),
            ResolvedPathContentOrigin {
                namespace,
                anchor: None,
            },
        );
        caller.phase = ResolverPhase::AwaitParent {
            basename: Arc::from(OsStr::new("child")),
        };
        let mut parent = ResolverFrame::new(
            path("/repo/link".into()),
            ResolvedPathContentOrigin {
                namespace,
                anchor: None,
            },
        );
        for i in 0..128 {
            parent
                .logical_chain
                .push(path(format!("/repo/route{i:03}")));
        }
        parent.route_bytes = parent
            .logical_chain
            .iter()
            .map(path_bytes)
            .fold(0, usize::saturating_add);
        let value = parent.completion(
            namespace,
            path("/repo/terminal".into()),
            ResolvedPathState::Present(PathLstat::new(PathNodeKind::Directory, 0, 0, 0, 0, 0o755)),
        );
        (
            ResolutionMachine {
                namespace,
                anchor: None,
                frames: vec![caller, parent],
            },
            value,
        )
    }
    #[test]
    fn guarded_parent_projection_prepays_all_children_before_mutation_and_checks_each_join() {
        let (mut machine, parent) = projection();
        let mut checks = 0;
        let result = finish(&mut machine, Ok(parent), &mut |_| {
            checks += 1;
            if checks == 3 { Err("cancel") } else { Ok(()) }
        });
        assert!(matches!(result, Err(Failure::Guard("cancel"))));
        assert_eq!(machine.frames.len(), 2);
        assert!(matches!(
            machine.frames[0].phase,
            ResolverPhase::AwaitParent { .. }
        ));
        let (mut machine, parent) = projection();
        let expected = parent
            .route()
            .iter()
            .map(|path| join_normalized(path, OsStr::new("child")))
            .collect::<Vec<_>>();
        let mut checks = 0;
        let mut bound = 0;
        let result = finish(&mut machine, Ok(parent), &mut |bytes| -> Result<(), ()> {
            checks += 1;
            bound = bound.max(bytes);
            Ok(())
        });
        assert!(matches!(result, Ok(None)));
        assert_eq!(checks, 128 + 1 + 128);
        assert!(bound >= expected.iter().map(path_bytes).sum::<usize>());
        let ResolverPhase::ReplayParentRoute { entries, .. } = &machine.frames[0].phase else {
            panic!("projected parent route")
        };
        assert_eq!(entries, &expected);
    }
}
