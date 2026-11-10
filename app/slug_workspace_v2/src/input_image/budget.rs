//! Workspace-scoped, nonsemantic credits retained by shared image descriptors.

use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering as AtomicOrdering;

use allocative::Allocative;
use dupe::Dupe;

use crate::NormalizedAbsolutePath;
use crate::PathDirectoryName;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ImageDescriptorBudgetError {
    ResourceLimit,
    ChargeOverflow,
    ForeignBudget,
}

impl fmt::Display for ImageDescriptorBudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "input image descriptor budget: {self:?}")
    }
}
impl std::error::Error for ImageDescriptorBudgetError {}

#[derive(Allocative)]
struct BudgetInner {
    limit: usize,
    used: AtomicUsize,
    peak_reserved: AtomicUsize,
}

#[derive(Clone, Dupe, Allocative)]
pub struct ImageDescriptorBudget(Arc<BudgetInner>);

impl fmt::Debug for ImageDescriptorBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageDescriptorBudget")
            .field("limit", &self.limit_bytes())
            .field("used", &self.used_bytes())
            .finish()
    }
}

impl ImageDescriptorBudget {
    pub fn new(limit_bytes: usize) -> Self {
        Self(Arc::new(BudgetInner {
            limit: limit_bytes,
            used: AtomicUsize::new(0),
            peak_reserved: AtomicUsize::new(0),
        }))
    }

    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn limit_bytes(&self) -> usize {
        self.0.limit
    }
    pub fn used_bytes(&self) -> usize {
        self.0.used.load(AtomicOrdering::Acquire)
    }

    /// Lifetime high-water of admitted reservation credits, not allocated bytes.
    /// Live reads can lag an in-flight `try_grow`; the exact final maximum
    /// requires all reservation mutations to settle. It is never reset.
    #[doc(hidden)]
    pub fn peak_reserved_bytes(&self) -> usize {
        self.0.peak_reserved.load(AtomicOrdering::Acquire)
    }

    pub fn try_reserve(
        &self,
        bytes: usize,
    ) -> Result<ImageDescriptorReservation, ImageDescriptorBudgetError> {
        let mut reservation = ImageDescriptorReservation {
            budget: self.dupe(),
            charged: 0,
        };
        reservation.try_grow(bytes)?;
        Ok(reservation)
    }
}

#[derive(Debug, Allocative)]
pub struct ImageDescriptorReservation {
    budget: ImageDescriptorBudget,
    charged: usize,
}

impl ImageDescriptorReservation {
    pub fn try_grow(&mut self, additional: usize) -> Result<(), ImageDescriptorBudgetError> {
        let charged = self
            .charged
            .checked_add(additional)
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
        let mut previous = self.budget.used_bytes();
        loop {
            let next = previous
                .checked_add(additional)
                .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
            if next > self.budget.limit_bytes() {
                return Err(ImageDescriptorBudgetError::ResourceLimit);
            }
            match self.budget.0.used.compare_exchange_weak(
                previous,
                next,
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
            ) {
                Ok(_) => {
                    self.charged = charged;
                    // Record the admitted overlap, even if another owner drops
                    // before this publication. Callers allocate only on return.
                    self.budget
                        .0
                        .peak_reserved
                        .fetch_max(next, AtomicOrdering::AcqRel);
                    return Ok(());
                }
                Err(actual) => previous = actual,
            }
        }
    }

    pub fn charged_bytes(&self) -> usize {
        self.charged
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.budget.same_instance(budget)
    }
    pub(crate) fn budget(&self) -> &ImageDescriptorBudget {
        &self.budget
    }

    /// All heap storage reachable from `value` must already be reserved. This
    /// adds the shared block's fixed storage before creating its allocation.
    pub fn freeze<T>(self, value: T) -> Result<BudgetedImageBlock<T>, ImageDescriptorBudgetError> {
        self.freeze_preserving(value)
            .map_err(|(error, value, reservation)| {
                // Failed owners may remove storage while their exact credit is live.
                drop(value);
                drop(reservation);
                error
            })
    }

    /// Like `freeze`, but preserves both admitted storage and its reservation
    /// on refusal. The owner precedes the credit even if the error is dropped.
    #[doc(hidden)]
    pub fn freeze_preserving<T>(
        mut self,
        value: T,
    ) -> Result<BudgetedImageBlock<T>, (ImageDescriptorBudgetError, T, Self)> {
        let header = std::mem::size_of::<BlockInner<T>>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(std::mem::align_of::<BlockInner<T>>()));
        let Some(header) = header else {
            return Err((ImageDescriptorBudgetError::ChargeOverflow, value, self));
        };
        if let Err(error) = self.try_grow(header) {
            return Err((error, value, self));
        }
        Ok(BudgetedImageBlock(Arc::new(BlockInner {
            value,
            reservation: self,
        })))
    }
}

impl Drop for ImageDescriptorReservation {
    fn drop(&mut self) {
        self.budget
            .0
            .used
            .fetch_sub(self.charged, AtomicOrdering::AcqRel);
    }
}

#[derive(Allocative)]
struct BlockInner<T> {
    // Drop owned descriptors before returning their reservation.
    value: T,
    reservation: ImageDescriptorReservation,
}

#[derive(Allocative)]
pub struct BudgetedImageBlock<T>(Arc<BlockInner<T>>);

impl<T> Clone for BudgetedImageBlock<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Dupe for BudgetedImageBlock<T> {}
impl<T: fmt::Debug> fmt::Debug for BudgetedImageBlock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}
impl<T: PartialEq> PartialEq for BudgetedImageBlock<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}
impl<T: Eq> Eq for BudgetedImageBlock<T> {}
impl<T: Hash> Hash for BudgetedImageBlock<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.get().hash(h)
    }
}

impl<T> BudgetedImageBlock<T> {
    pub fn get(&self) -> &T {
        &self.0.value
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.reservation.same_budget(budget)
    }
    pub fn charged_bytes(&self) -> usize {
        self.0.reservation.charged_bytes()
    }
    fn budget(&self) -> &ImageDescriptorBudget {
        self.0.reservation.budget()
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct ImageChargedPath(BudgetedImageBlock<NormalizedAbsolutePath>);

impl ImageChargedPath {
    /// Same immutable allocation and its exact retained reservation, not merely
    /// equal normalized bytes or membership in the same budget.
    pub(crate) fn same_retention_owner(&self, other: &Self) -> bool {
        let Self(BudgetedImageBlock(left)) = self;
        let Self(BudgetedImageBlock(right)) = other;
        Arc::ptr_eq(left, right)
    }

    /// Conservative pin for a selector path already owned by the source owner.
    pub fn pin_existing(
        path: NormalizedAbsolutePath,
        budget: &ImageDescriptorBudget,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        let bytes = path
            .retained_capacity_bytes()
            .checked_add(64)
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
        Ok(Self(budget.try_reserve(bytes)?.freeze(path)?))
    }

    pub fn try_join_component(
        &self,
        name: &PathDirectoryName,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        self.try_join_name(name.as_os_str())
    }

    pub(crate) fn try_join_name(
        &self,
        name: &std::ffi::OsStr,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(ImageDescriptorBudgetError::ForeignBudget);
        }
        let length = self
            .as_path()
            .as_os_str()
            .len()
            .checked_add(name.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
        // Explicit capacity plus the normalized PathBuf Arc header. Parent and
        // one normal component are already normalized; no second copy occurs.
        let reservation = self.0.budget().try_reserve(
            length
                .checked_add(64)
                .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?,
        )?;
        let mut joined = std::path::PathBuf::with_capacity(length);
        joined.push(self.as_path());
        joined.push(name);
        let joined = NormalizedAbsolutePath::from_normalized_owned(joined);
        Ok(Self(reservation.freeze(joined)?))
    }

    pub(crate) fn try_resolve_alias(
        &self,
        target: &Path,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        let parent = self.as_path().parent().unwrap_or(self.as_path());
        let length = if target.is_absolute() {
            target.as_os_str().len()
        } else {
            parent
                .as_os_str()
                .len()
                .checked_add(target.as_os_str().len())
                .and_then(|n| n.checked_add(1))
                .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?
        };
        // Normalization owns an input buffer and a growing output buffer. Keep
        // their conservative high-water charge with the resulting path.
        let bytes = length
            .checked_mul(5)
            .and_then(|n| n.checked_add(128))
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
        let reservation = self.0.budget().try_reserve(bytes)?;
        let mut path = std::path::PathBuf::with_capacity(length);
        if !target.is_absolute() {
            path.push(parent);
        }
        path.push(target);
        let normalized = NormalizedAbsolutePath::new(path).expect("absolute parent or target");
        Ok(Self(reservation.freeze(normalized)?))
    }

    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
    }
    pub fn as_path(&self) -> &Path {
        self.0.get().as_path()
    }
    pub fn normalized_ref(&self) -> &NormalizedAbsolutePath {
        self.0.get()
    }
    pub(crate) fn budget(&self) -> &ImageDescriptorBudget {
        self.0.budget()
    }
    pub(crate) fn normalized(&self) -> NormalizedAbsolutePath {
        self.0.get().dupe()
    }

    // Pin temporary builder/Need storage through a returned charged demand.
    pub(crate) fn retain_charge(
        &self,
        mut reservation: ImageDescriptorReservation,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        if !self.same_budget(reservation.budget()) {
            return Err(ImageDescriptorBudgetError::ForeignBudget);
        }
        // This wrapper retains the normalized path buffer independently of
        // the original charged block. Pin its full capacity as well as the
        // temporary Need storage, even if every original owner retires.
        reservation.try_grow(
            self.normalized_ref()
                .retained_capacity_bytes()
                .checked_add(64)
                .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?,
        )?;
        Ok(Self(reservation.freeze(self.normalized())?))
    }
}

impl PartialEq for ImageChargedPath {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}
impl Eq for ImageChargedPath {}
impl PartialOrd for ImageChargedPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ImageChargedPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.get().cmp(other.0.get())
    }
}
impl Hash for ImageChargedPath {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.get().hash(h)
    }
}

pub(crate) fn reserve_vec_slot<T>(
    reservation: &mut ImageDescriptorReservation,
    values: &mut Vec<T>,
) -> Result<(), ImageDescriptorBudgetError> {
    if values.len() == values.capacity() {
        let capacity = values
            .capacity()
            .checked_mul(2)
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?
            .max(4);
        let bytes = std::mem::size_of::<T>()
            .checked_mul(capacity)
            .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
        // Charge the replacement peak conservatively until block retirement.
        reservation.try_grow(bytes)?;
        values
            .try_reserve_exact(capacity - values.len())
            .map_err(|_| ImageDescriptorBudgetError::ResourceLimit)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
