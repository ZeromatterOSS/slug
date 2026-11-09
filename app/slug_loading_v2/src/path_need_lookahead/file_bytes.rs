//! Known sizes bound homogeneous discovery, never fresh native IO or authority.
use std::future::Future;
use std::mem::align_of;
use std::mem::size_of;
use std::mem::size_of_val;
use std::pin::Pin;
use std::sync::Arc;
#[cfg(any(test, feature = "native-probe-observer"))]
use std::sync::atomic::AtomicBool;
#[cfg(any(test, feature = "native-probe-observer"))]
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use dice::DiceComputations;
use dupe::Dupe;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationKey;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::PathOutcome;

use super::*;

#[cfg(test)]
#[path = "file_bytes_tests.rs"]
mod tests;

const MAX_KNOWN_BYTES: u64 = 256 * 1024;

enum FirstClass {
    Lstat,
    FileBytes,
    Other,
}

enum Window {
    Unselected,
    Lstat(PathLookahead),
    FileBytes(ByteWindow),
}

pub(crate) struct HomogeneousPathLookahead {
    window: Window,
}

impl HomogeneousPathLookahead {
    pub(crate) fn new() -> Self {
        Self {
            window: Window::Unselected,
        }
    }

    pub(crate) fn can_inspect_next(&self) -> bool {
        match &self.window {
            Window::Unselected => true,
            Window::Lstat(window) => window.can_inspect_next(),
            Window::FileBytes(window) => window.inspected < MAX_ROWS,
        }
    }

    pub(crate) fn observe<'a, E: PathNeedBreak + 'a>(
        &'a mut self,
        ctx: &'a mut DiceComputations<'_>,
        result: Result<(), E>,
    ) -> Observation<'a, E> {
        debug_assert!(self.can_inspect_next());
        // Borrow the installed Lstat owner throughout reduction: a moved window
        // must not overlap the adapter's potentially large returned error.
        if let Window::Lstat(window) = &mut self.window {
            return Observation::ready(window.observe(result));
        }
        if let Window::FileBytes(window) = &mut self.window {
            window.inspected += 1;
            // Consume/drop the adapter while the previous window stays inside
            // the single installed wrapper, before taking its pending owner.
            return match result.map_err(E::into_need) {
                Ok(()) => Observation::ready(ControlFlow::Continue(())),
                Err(Err(error)) => {
                    drop(error);
                    let Window::FileBytes(window) =
                        std::mem::replace(&mut self.window, Window::Unselected)
                    else {
                        unreachable!("byte window was installed");
                    };
                    Observation::ready(ControlFlow::Break(E::from_need(window.pending)))
                }
                Err(Ok(need)) => {
                    let Window::FileBytes(window) =
                        std::mem::replace(&mut self.window, Window::Unselected)
                    else {
                        unreachable!("byte window was installed");
                    };
                    byte_observation(self, ctx, Some(window), need)
                }
            };
        }
        match std::mem::replace(&mut self.window, Window::Unselected) {
            Window::Lstat(_) => unreachable!("installed Lstat window was handled in place"),
            Window::Unselected => match result.map_err(E::into_need) {
                Ok(()) => Observation::ready(ControlFlow::Continue(())),
                Err(Err(error)) => Observation::ready(ControlFlow::Break(error)),
                Err(Ok(need)) => {
                    let class = need
                        .only_path_observations()
                        .and_then(|paths| paths.demands().first())
                        .map_or(FirstClass::Other, |demand| match demand.operation() {
                            PathObservationOperation::Lstat => FirstClass::Lstat,
                            PathObservationOperation::FileBytes => FirstClass::FileBytes,
                            _ => FirstClass::Other,
                        });
                    // Only this small tag survives classification; an Image
                    // operation's cloned owner has already been dropped.
                    match class {
                        FirstClass::Lstat => {
                            self.window = Window::Lstat(PathLookahead::new());
                            let Window::Lstat(window) = &mut self.window else {
                                unreachable!("Lstat window was just installed");
                            };
                            Observation::ready(window.observe(Err(E::from_need(need))))
                        }
                        FirstClass::FileBytes => byte_observation(self, ctx, None, need),
                        FirstClass::Other => {
                            Observation::ready(ControlFlow::Break(E::from_need(need)))
                        }
                    }
                }
            },
            Window::FileBytes(_) => unreachable!("installed byte window was handled in place"),
        }
    }

    pub(crate) fn finish<E: PathNeedBreak>(self) -> Result<(), E> {
        match self.window {
            Window::Unselected => Ok(()),
            Window::Lstat(window) => window.finish(),
            Window::FileBytes(window) => Err(E::from_need(window.pending)),
        }
    }
}

// Ready reduction is synchronous: no byte future is constructed on the Lstat,
// Complete, or error paths. Only the separate byte helper owns a boxed frame.
pub(crate) enum Observation<'a, E> {
    Ready(Option<ControlFlow<E>>),
    Bytes(Pin<Box<dyn Future<Output = ControlFlow<E>> + Send + 'a>>),
}

impl<E> Unpin for Observation<'_, E> {}

impl<E> Observation<'_, E> {
    fn ready(outcome: ControlFlow<E>) -> Self {
        Self::Ready(Some(outcome))
    }
}

impl<E> Future for Observation<'_, E> {
    type Output = ControlFlow<E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match this {
            Self::Ready(outcome) => {
                Poll::Ready(outcome.take().expect("observation polled after completion"))
            }
            Self::Bytes(future) => match future.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(outcome) => {
                    // Retire the frame and its captured owners before returning.
                    *this = Self::Ready(None);
                    Poll::Ready(outcome)
                }
            },
        }
    }
}

// Keep byte-frame construction out of the synchronous metadata dispatcher.
#[inline(never)]
fn byte_observation<'a, E: PathNeedBreak + 'a>(
    owner: &'a mut HomogeneousPathLookahead,
    ctx: &'a mut DiceComputations<'_>,
    mut previous: Option<ByteWindow>,
    need: SourcePreparationNeeds,
) -> Observation<'a, E> {
    let base = byte_candidate(&need).and_then(|right| match &previous {
        Some(previous) => {
            let left = byte_candidate(&previous.pending)?;
            byte_base_peak(&left, Some(&right))
        }
        None => byte_base_peak(&right, None),
    });
    let mut incoming = Some(need);
    // Merely querying this closure's output type does not construct its future.
    // Retain the original owners until the complete frame peak is admitted.
    let make = || {
        // Moving the borrowed handles makes this a FnOnce constructor; no
        // returned future borrows the constructor's temporary closure frame.
        let owner = owner;
        let ctx = ctx;
        run_byte_observation::<E>(owner, ctx, previous.take(), incoming.take().unwrap())
    };
    let (frame_bytes, frame_align) = future_output_layout(&make);
    let admitted = base
        .is_some_and(|(_, base)| byte_work_peak_aligned(base, frame_bytes, frame_align).is_some());
    if admitted {
        Observation::Bytes(Box::pin(make()))
    } else {
        drop(make);
        let need = match previous {
            Some(previous) => {
                drop(incoming.take());
                previous.pending
            }
            None => incoming.take().unwrap(),
        };
        Observation::ready(ControlFlow::Break(E::from_need(need)))
    }
}

fn future_output_layout<F: Future>(_: &impl FnOnce() -> F) -> (usize, usize) {
    (size_of::<F>(), align_of::<F>())
}

async fn run_byte_observation<E: PathNeedBreak>(
    owner: &mut HomogeneousPathLookahead,
    ctx: &mut DiceComputations<'_>,
    previous: Option<ByteWindow>,
    need: SourcePreparationNeeds,
) -> ControlFlow<E> {
    let outcome = match previous {
        Some(previous) => previous.extend(ctx, need).await,
        None => ByteWindow::first(ctx, need)
            .await
            .map(|window| (window, false)),
    };
    match outcome {
        Ok((window, _extended)) => {
            owner.window = Window::FileBytes(window);
            #[cfg(feature = "native-probe-observer")]
            mark_extended(&EXTENDED, _extended);
            ControlFlow::Continue(())
        }
        Err(need) => ControlFlow::Break(E::from_need(need)),
    }
}

struct ByteWindow {
    pending: SourcePreparationNeeds,
    inspected: usize,
    known_bytes: u64,
}

impl ByteWindow {
    async fn first(
        ctx: &mut DiceComputations<'_>,
        need: SourcePreparationNeeds,
    ) -> Result<Self, SourcePreparationNeeds> {
        let Some(right) = byte_candidate(&need) else {
            return Err(need);
        };
        let future = added_known_bytes(ctx, None, right.paths.demands(), MAX_KNOWN_BYTES);
        let future_bytes = size_of_val(&future);
        if byte_forecast(&right, None, future_bytes).is_none() {
            drop(future);
            return Err(need);
        }
        let Some(known_bytes) = future.await else {
            return Err(need);
        };
        if byte_forecast(&right, None, future_bytes).is_none() {
            return Err(need);
        }
        Ok(Self {
            pending: need,
            inspected: 1,
            known_bytes,
        })
    }

    async fn extend(
        mut self,
        ctx: &mut DiceComputations<'_>,
        need: SourcePreparationNeeds,
    ) -> Result<(Self, bool), SourcePreparationNeeds> {
        let Some(right) = byte_candidate(&need) else {
            return Err(self.pending);
        };
        let left = byte_candidate(&self.pending).expect("pending byte window was admitted");
        let future = added_known_bytes(
            ctx,
            Some(left.paths.demands()),
            right.paths.demands(),
            MAX_KNOWN_BYTES - self.known_bytes,
        );
        let future_bytes = size_of_val(&future);
        let Some(unique) = byte_forecast(&left, Some(&right), future_bytes) else {
            drop(future);
            return Err(self.pending);
        };
        let Some(added) = future.await else {
            return Err(self.pending);
        };
        let Some(known_bytes) = self.known_bytes.checked_add(added) else {
            return Err(self.pending);
        };
        if known_bytes > MAX_KNOWN_BYTES
            || byte_forecast(&left, Some(&right), future_bytes).is_none()
        {
            return Err(self.pending);
        }
        let extended = unique != left.paths.demands().len();
        if extended {
            let joined = left.paths.union(right.paths);
            self.pending = SourcePreparationNeeds::path(joined);
            self.known_bytes = known_bytes;
        }
        Ok((self, extended))
    }
}

fn byte_candidate(need: &SourcePreparationNeeds) -> Option<Candidate<'_>> {
    let paths = need.only_path_observations()?;
    if paths.demands().len() > MAX_DEMANDS {
        return None;
    }
    let control_bytes = control_bytes(need)?;
    if control_bytes > MAX_METADATA {
        return None;
    }
    let mut path_bytes = 0usize;
    for demand in paths.demands() {
        if demand.operation() != PathObservationOperation::FileBytes
            || demand.path().as_path().as_os_str().len() > MAX_PATH
        {
            return None;
        }
        path_bytes = path_bytes.checked_add(demand.path().retained_storage_bytes()?)?;
        if path_bytes > MAX_METADATA {
            return None;
        }
    }
    Some(Candidate {
        paths,
        path_bytes,
        control_bytes,
    })
}

fn byte_forecast(
    left: &Candidate<'_>,
    right: Option<&Candidate<'_>>,
    future_bytes: usize,
) -> Option<usize> {
    let (unique, base) = byte_base_peak(left, right)?;
    byte_work_peak(base, future_bytes).map(|_| unique)
}

fn byte_base_peak(left: &Candidate<'_>, right: Option<&Candidate<'_>>) -> Option<(usize, usize)> {
    let unique = forecast(left, right)?;
    let controls = right.map_or(left.control_bytes, |right| {
        left.control_bytes.max(right.control_bytes)
    });
    let paths = left
        .path_bytes
        .checked_add(right.map_or(0, |right| right.path_bytes))?;
    let base = peak_bytes(
        left.paths.demands().len(),
        right.map_or(0, |right| right.paths.demands().len()),
        unique,
        controls,
        paths,
    )?;
    Some((unique, base))
}

fn byte_work_peak(base: usize, future_bytes: usize) -> Option<usize> {
    byte_work_peak_aligned(base, future_bytes, align_of::<u128>())
}

fn byte_work_peak_aligned(base: usize, future_bytes: usize, frame_align: usize) -> Option<usize> {
    // Include simultaneous stack construction and heap destination, alignment,
    // fat-box control, and the actual key/result/window controls. Admission runs
    // before frame construction, allocation, and the first metadata-key poll.
    let extra = future_bytes
        .checked_mul(2)?
        .checked_add(frame_align)?
        .checked_add(size_of::<Pin<Box<dyn Future<Output = ()> + Send>>>())?
        .checked_add(size_of::<ByteWindow>())?
        .checked_add(size_of::<HomogeneousPathLookahead>())?
        .checked_add(size_of::<PathObservationKey>())?
        .checked_add(size_of::<PathObservationDemand>())?
        .checked_add(size_of::<PathOutcome<Arc<PathObservationResult>>>())?
        .checked_add(2 * size_of::<usize>())?
        .checked_add(size_of::<Option<u64>>())?;
    let peak = base.checked_add(extra)?;
    (peak <= MAX_METADATA).then_some(peak)
}

async fn added_known_bytes(
    ctx: &mut DiceComputations<'_>,
    pending: Option<&[PathObservationDemand]>,
    right: &[PathObservationDemand],
    maximum: u64,
) -> Option<u64> {
    let mut added = 0u64;
    for demand in right {
        if pending.is_some_and(|pending| pending.binary_search(demand).is_ok()) {
            continue;
        }
        let key = PathObservationKey::new(PathObservationDemand::new(
            demand.namespace(),
            demand.path().dupe(),
            PathObservationOperation::Lstat,
        ));
        let Ok(PathOutcome::Complete(result)) = ctx.compute(&key).await else {
            return None;
        };
        let PathObservationResult::Lstat(PathOperationResult::Present(stat)) = result.as_ref()
        else {
            return None;
        };
        if stat.kind() != PathNodeKind::RegularFile {
            return None;
        }
        let size = u64::try_from(stat.size()).ok()?;
        added = added.checked_add(size)?;
        if added > maximum {
            return None;
        }
    }
    Some(added)
}

// Concrete caller tests cover both adapter types and all three production paths.
// These are phase-disjoint semantic object/control forecasts, not physical ABI
// stack or RSS bounds. Byte-frame construction/boxing has its own 64 KiB peak.
#[cfg(test)]
pub(crate) fn assert_observer_layout<E>(observation: &Observation<'_, E>) {
    let wrapper = size_of::<HomogeneousPathLookahead>();
    let ready = size_of::<E>()
        .max(size_of::<Result<(), E>>())
        .max(size_of::<ControlFlow<E>>())
        .max(size_of_val(observation))
        + wrapper;
    // Classification consumes the adapter before producing the Need. Candidate
    // and iterator scratch does not coexist with another live adapter error.
    let candidate = size_of::<SourcePreparationNeeds>()
        + wrapper
        + 2 * size_of::<Candidate<'_>>()
        + 4 * size_of::<std::slice::Iter<'_, PathObservationDemand>>();
    let peak = ready.max(candidate);
    assert!(
        peak <= FIXED_SCRATCH,
        "lookahead semantic control exceeds old Lstat slack: peak={peak}; ready={ready}, candidate={candidate}, adapter={}, result={}, observer={}, wrapper={wrapper}, need={}, byte_window={}",
        size_of::<E>(),
        size_of::<Result<(), E>>(),
        size_of_val(observation),
        size_of::<SourcePreparationNeeds>(),
        size_of::<ByteWindow>()
    );
}

#[cfg(feature = "native-probe-observer")]
static EXTENDED: AtomicBool = AtomicBool::new(false);

#[cfg(any(test, feature = "native-probe-observer"))]
fn mark_extended(witness: &AtomicBool, extended: bool) {
    if extended {
        witness.store(true, Ordering::Relaxed);
    }
}

/// Process-wide monotonic expansion witness, not servicing or request identity.
/// A true initial sample makes attribution to a later command unknown.
#[cfg(feature = "native-probe-observer")]
pub fn file_bytes_lookahead_extended() -> bool {
    EXTENDED.load(Ordering::Relaxed)
}
