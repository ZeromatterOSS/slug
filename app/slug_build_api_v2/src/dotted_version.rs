//! Immutable Bazel DottedVersion payload, independent of the evaluator.
//!
//! Native equality preserves spelling. Language wrappers must explicitly use
//! `normalized_cmp` and `hash_normalized` instead.
use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem::align_of;
use std::mem::size_of;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

mod parser;

#[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash, Allocative)]
pub struct DottedVersion(Arc<Data>);

#[derive(Debug, PartialEq, Eq, Hash, Allocative)]
struct Data {
    spelling: CompactString,
    components: Box<[Component]>,
    original_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
struct Span {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
struct Component {
    first: i32,
    alpha: Option<Span>,
    second: i32,
    spelling: Span,
}

impl Component {
    fn is_zero(self) -> bool {
        self.first == 0 && self.alpha.is_none() && self.second == 0
    }
}

/// Allocation-free, validated plan. Admit its work and peak before `into_owned`.
#[derive(Debug)]
pub struct DottedVersionPlan<'a> {
    input: &'a str,
    original_count: usize,
    normalized_count: usize,
    construction_records: usize,
    storage: DottedVersionStorage,
}

#[derive(Debug, Clone, Copy)]
pub struct DottedVersionComponent<'a> {
    input: &'a str,
    component: Component,
}

/// Conservative complete layout bounds; construction includes transfer overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DottedVersionStorage {
    retained: usize,
    peak: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DottedVersionParseError<'a> {
    input: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DottedVersionPrepareError<'a, E> {
    Invalid(DottedVersionParseError<'a>),
    Admission(E),
    StorageOverflow,
}

impl fmt::Display for DottedVersionParseError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Dotted version components must all start with the form \\d+([a-z0-9]*?)?(\\d+)? but got '{}'",
            self.input
        )
    }
}

impl<E: fmt::Display> fmt::Display for DottedVersionPrepareError<'_, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
            Self::StorageOverflow => f.write_str("dotted version storage forecast overflow"),
        }
    }
}

impl DottedVersion {
    /// Pays before locating/reading each component; never copies input or allocates.
    /// The caller separately admits the complete input byte extent before scanning.
    pub fn prepare<'a, E>(
        input: &'a str,
        mut admit_record: impl FnMut() -> Result<(), E>,
    ) -> Result<DottedVersionPlan<'a>, DottedVersionPrepareError<'a, E>> {
        let scan = parser::scan(input, &mut admit_record, |_| {})?;
        let storage = storage_bound(input.len(), scan.normalized)
            .ok_or(DottedVersionPrepareError::StorageOverflow)?;
        let construction_records = scan
            .examined
            .checked_add(scan.normalized)
            .and_then(|n| n.checked_add(1))
            .ok_or(DottedVersionPrepareError::StorageOverflow)?;
        Ok(DottedVersionPlan {
            input,
            original_count: scan.original,
            normalized_count: scan.normalized,
            construction_records,
            storage,
        })
    }

    pub fn spelling(&self) -> &str {
        &self.0.spelling
    }

    pub fn components(&self) -> impl ExactSizeIterator<Item = DottedVersionComponent<'_>> {
        self.0
            .components
            .iter()
            .map(|&component| DottedVersionComponent {
                input: self.spelling(),
                component,
            })
    }

    pub fn original_component_count(&self) -> usize {
        self.0.original_count
    }

    pub fn normalized_cmp(&self, other: &Self) -> Ordering {
        let mut left = self.components();
        let mut right = other.components();
        loop {
            let a = left.next();
            let b = right.next();
            if a.is_none() && b.is_none() {
                return Ordering::Equal;
            }
            let (af, aa, az) = match a {
                Some(c) => (c.first_number(), c.alpha(), c.second_number()),
                None => (0, None, 0),
            };
            let (bf, ba, bz) = match b {
                Some(c) => (c.first_number(), c.alpha(), c.second_number()),
                None => (0, None, 0),
            };
            let alpha = || match (aa, ba) {
                (Some(a), Some(b)) => a.cmp(b),
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
            };
            let order = af.cmp(&bf).then_with(alpha).then_with(|| az.cmp(&bz));
            if order != Ordering::Equal {
                return order;
            }
        }
    }

    /// Hashes language equality, deliberately excluding original spelling/count.
    pub fn hash_normalized<H: Hasher>(&self, state: &mut H) {
        self.0.components.len().hash(state);
        for c in self.components() {
            (c.first_number(), c.alpha(), c.second_number()).hash(state);
        }
    }

    pub fn storage(&self) -> DottedVersionStorage {
        storage_bound(self.spelling().len(), self.0.components.len())
            .expect("validated dotted version layout")
    }
}

impl DottedVersionPlan<'_> {
    pub fn spelling(&self) -> &str {
        self.input
    }
    pub fn normalized_component_count(&self) -> usize {
        self.normalized_count
    }
    pub fn original_component_count(&self) -> usize {
        self.original_count
    }
    pub fn storage(&self) -> DottedVersionStorage {
        self.storage
    }
    pub fn construction_records(&self) -> usize {
        self.construction_records
    }

    /// Caller must admit `construction_records` and the peak before this copy.
    pub fn into_owned(self) -> DottedVersion {
        let mut components = Vec::with_capacity(self.normalized_count);
        let mut count = 0;
        let scan = parser::scan(
            self.input,
            &mut || Ok::<_, std::convert::Infallible>(()),
            |c| {
                if count < self.normalized_count {
                    components.push(c);
                }
                count += 1;
            },
        )
        .expect("immutable validated dotted version input");
        debug_assert_eq!(scan.original, self.original_count);
        DottedVersion(Arc::new(Data {
            spelling: CompactString::new(self.input),
            components: components.into_boxed_slice(),
            original_count: self.original_count,
        }))
    }
}

impl DottedVersionStorage {
    pub fn retained_bytes(self) -> usize {
        self.retained
    }
    pub fn construction_peak_bytes(self) -> usize {
        self.peak
    }
}

impl<'a> DottedVersionComponent<'a> {
    pub fn first_number(self) -> i32 {
        self.component.first
    }
    pub fn alpha(self) -> Option<&'a str> {
        self.component.alpha.map(|s| &self.input[s.start..s.end])
    }
    pub fn second_number(self) -> i32 {
        self.component.second
    }
    pub fn spelling(self) -> &'a str {
        let s = self.component.spelling;
        &self.input[s.start..s.end]
    }
}

fn storage_bound(bytes: usize, components: usize) -> Option<DottedVersionStorage> {
    let array = std::alloc::Layout::array::<Component>(components)
        .ok()?
        .size();
    // compact_str 0.8: heap minimum is inline size + one word. Two more
    // words cover optional out-of-line capacity and allocation alignment.
    // Inline strings are deliberately overestimated by this same bound.
    let text = bytes
        .max(size_of::<CompactString>().checked_add(size_of::<usize>())?)
        .checked_add(2usize.checked_mul(size_of::<usize>())?)?;
    let retained = size_of::<DottedVersion>()
        .checked_add(size_of::<Data>())?
        .checked_add(2usize.checked_mul(size_of::<usize>())?)?
        .checked_add(align_of::<Data>() - 1)?
        .checked_add(text)?
        .checked_add(array)?;
    // Both component buffers may coexist during Vec-to-Box transfer. Retain
    // the Vec header and a second text extent as conservative construction peak.
    let peak = retained
        .checked_add(array)?
        .checked_add(size_of::<Vec<Component>>())?
        .checked_add(text)?;
    Some(DottedVersionStorage { retained, peak })
}

#[cfg(test)]
mod tests;
