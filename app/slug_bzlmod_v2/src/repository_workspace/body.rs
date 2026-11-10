//! Pure immutable logical bytes. Encoding is private storage, never authority.
use std::alloc::Layout;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::Weak;

use allocative::Allocative;
use starlark_map::StarlarkHashValue;
use starlark_map::StarlarkHasher;

mod builder;
mod codec;
#[cfg(test)]
mod construction_work_for_test;
#[cfg(test)]
mod construction_workspace_tests;
mod range_comparison;
#[cfg(test)]
mod range_comparison_tests;
#[cfg(test)]
mod range_comparison_work_for_test;
pub use builder::RepositoryWorkspaceBodyBuildError;
pub use builder::RepositoryWorkspaceBodyBuilder;

pub const REPOSITORY_WORKSPACE_BODY_CHUNK_BYTES: usize = 64 * 1024;
const CHUNK: usize = REPOSITORY_WORKSPACE_BODY_CHUNK_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryWorkspaceBodyError {
    Length,
    Codec,
    Storage,
    Allocation,
}
impl std::fmt::Display for RepositoryWorkspaceBodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "workspace body: {self:?}")
    }
}
impl std::error::Error for RepositoryWorkspaceBodyError {}
impl From<RepositoryWorkspaceBodyError> for super::RepositoryWorkspaceError {
    fn from(error: RepositoryWorkspaceBodyError) -> Self {
        match error {
            RepositoryWorkspaceBodyError::Length | RepositoryWorkspaceBodyError::Codec => {
                Self::InvalidResult
            }
            RepositoryWorkspaceBodyError::Storage | RepositoryWorkspaceBodyError::Allocation => {
                Self::RetentionLimit
            }
        }
    }
}
type Error = RepositoryWorkspaceBodyError;

#[derive(Clone, Allocative)]
pub struct RepositoryWorkspaceBody(Repr);
impl dupe::Dupe for RepositoryWorkspaceBody {}
#[derive(Clone, Allocative)]
enum Repr {
    Raw {
        bytes: Arc<[u8]>,
        routing: StarlarkHashValue,
    },
    Packed(Arc<PackedData>),
}
#[derive(Allocative)]
struct Block {
    bytes: Arc<[u8]>,
    compressed: bool,
}
#[derive(Allocative)]
struct PackedData {
    logical: usize,
    routing: StarlarkHashValue,
    control: usize,
    native: usize,
    retained: usize,
    blocks: Box<[Block]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RepositoryWorkspaceBodyAllocation {
    address: usize,
    logical: usize,
    packed: bool,
}
impl std::fmt::Debug for RepositoryWorkspaceBodyAllocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorkspaceBodyAllocation(<opaque>)")
    }
}
#[derive(Clone)]
pub struct RepositoryWorkspaceBodyWeak(WeakRepr);
#[derive(Clone)]
enum WeakRepr {
    Raw(Weak<[u8]>, StarlarkHashValue),
    Packed(Weak<PackedData>),
}
impl RepositoryWorkspaceBodyWeak {
    pub fn upgrade(&self) -> Option<RepositoryWorkspaceBody> {
        match &self.0 {
            WeakRepr::Raw(bytes, routing) => bytes.upgrade().map(|bytes| {
                RepositoryWorkspaceBody(Repr::Raw {
                    bytes,
                    routing: *routing,
                })
            }),
            WeakRepr::Packed(data) => data
                .upgrade()
                .map(|data| RepositoryWorkspaceBody(Repr::Packed(data))),
        }
    }
    pub fn strong_count(&self) -> usize {
        match &self.0 {
            WeakRepr::Raw(b, _) => b.strong_count(),
            WeakRepr::Packed(b) => b.strong_count(),
        }
    }
}
fn arc_layout(payload: Layout) -> Result<usize, Error> {
    Layout::new::<[usize; 2]>()
        .extend(payload)
        .map(|(l, _)| l.pad_to_align().size())
        .map_err(|_| Error::Storage)
}
fn arc_bytes(length: usize) -> Result<usize, Error> {
    arc_layout(Layout::array::<u8>(length).map_err(|_| Error::Storage)?)
}
fn table_bytes(count: usize) -> Result<usize, Error> {
    Layout::array::<Block>(count)
        .map(|l| l.size())
        .map_err(|_| Error::Storage)
}
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Storage)
}
impl RepositoryWorkspaceBody {
    pub fn from_arc(bytes: Arc<[u8]>) -> Self {
        let mut hasher = StarlarkHasher::new();
        bytes.len().hash(&mut hasher);
        for chunk in bytes.chunks(CHUNK) {
            hasher.write(chunk);
        }
        Self(Repr::Raw {
            bytes,
            routing: hasher.finish_small(),
        })
    }
    pub fn len(&self) -> usize {
        match &self.0 {
            Repr::Raw { bytes, .. } => bytes.len(),
            Repr::Packed(data) => data.logical,
        }
    }
    /// Preserve the admitted decoded-block/control workspace across codec changes.
    /// The old bound was 4*65536 + 2*10504 + 8192; no headroom is reclaimed.
    pub fn decode_scratch_bytes() -> usize {
        291_344
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn raw_arc(&self) -> Option<&Arc<[u8]>> {
        match &self.0 {
            Repr::Raw { bytes, .. } => Some(bytes),
            _ => None,
        }
    }
    pub fn ptr_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Repr::Raw { bytes: a, .. }, Repr::Raw { bytes: b, .. }) => Arc::ptr_eq(a, b),
            (Repr::Packed(a), Repr::Packed(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub fn allocation_identity(&self) -> RepositoryWorkspaceBodyAllocation {
        RepositoryWorkspaceBodyAllocation {
            address: match &self.0 {
                Repr::Raw { bytes, .. } => bytes.as_ptr() as usize,
                Repr::Packed(data) => Arc::as_ptr(data) as usize,
            },
            logical: self.len(),
            packed: matches!(&self.0, Repr::Packed(_)),
        }
    }
    pub fn downgrade(&self) -> RepositoryWorkspaceBodyWeak {
        RepositoryWorkspaceBodyWeak(match &self.0 {
            Repr::Raw { bytes, routing } => WeakRepr::Raw(Arc::downgrade(bytes), *routing),
            Repr::Packed(data) => WeakRepr::Packed(Arc::downgrade(data)),
        })
    }
    #[cfg(test)]
    pub(super) fn set_test_routing_hash(&mut self, hash: StarlarkHashValue) {
        match &mut self.0 {
            Repr::Raw { routing, .. } => *routing = hash,
            Repr::Packed(data) => {
                Arc::get_mut(data)
                    .expect("test has unique body owner")
                    .routing = hash
            }
        }
    }
    pub(crate) fn routing_hash(&self) -> StarlarkHashValue {
        match &self.0 {
            Repr::Raw { routing, .. } => *routing,
            Repr::Packed(data) => data.routing,
        }
    }
    pub fn control_storage_bytes(&self) -> usize {
        match &self.0 {
            Repr::Raw { bytes, .. } => {
                arc_bytes(bytes.len()).expect("live Arc has valid allocation layout") - bytes.len()
            }
            Repr::Packed(data) => data.control,
        }
    }
    pub fn native_storage_bytes(&self) -> usize {
        match &self.0 {
            Repr::Raw { bytes, .. } => {
                arc_bytes(bytes.len()).expect("live Arc has valid allocation layout")
            }
            Repr::Packed(data) => data.native,
        }
    }
    pub fn retained_storage_bytes(&self) -> usize {
        match &self.0 {
            Repr::Raw { .. } => self.native_storage_bytes(),
            Repr::Packed(data) => data.retained,
        }
    }
    pub fn try_for_each_chunk<E>(
        &self,
        visit: &mut impl FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.0 {
            Repr::Raw { bytes, .. } => {
                for chunk in bytes.chunks(CHUNK) {
                    visit(chunk)?;
                }
            }
            Repr::Packed(data) => {
                let mut output = [0u8; CHUNK];
                for (index, block) in data.blocks.iter().enumerate() {
                    let length = (data.logical - index * CHUNK).min(CHUNK);
                    if block.compressed {
                        codec::decode(&block.bytes, &mut output[..length])
                            .expect("closed workspace body invariant");
                        visit(&output[..length])?;
                    } else {
                        visit(&block.bytes)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn for_each_chunk(&self, visit: &mut impl FnMut(&[u8])) {
        self.try_for_each_chunk(&mut |chunk| {
            visit(chunk);
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    }
    pub fn copy_into(&self, offset: usize, out: &mut [u8]) -> bool {
        if offset
            .checked_add(out.len())
            .is_none_or(|end| end > self.len())
        {
            return false;
        }
        match &self.0 {
            Repr::Raw { bytes, .. } => out.copy_from_slice(&bytes[offset..offset + out.len()]),
            Repr::Packed(data) => {
                let mut scratch = [0u8; CHUNK];
                let mut position = offset;
                let mut written = 0;
                while written < out.len() {
                    let block_index = position / CHUNK;
                    let within = position % CHUNK;
                    let block = &data.blocks[block_index];
                    let length = (data.logical - block_index * CHUNK).min(CHUNK);
                    let bytes = if block.compressed {
                        codec::decode(&block.bytes, &mut scratch[..length])
                            .expect("closed workspace body invariant");
                        &scratch[..length]
                    } else {
                        &block.bytes
                    };
                    let count = (length - within).min(out.len() - written);
                    out[written..written + count].copy_from_slice(&bytes[within..within + count]);
                    position += count;
                    written += count;
                }
            }
        }
        true
    }
    pub fn range_eq(&self, offset: usize, bytes: &[u8]) -> bool {
        if offset
            .checked_add(bytes.len())
            .is_none_or(|end| end > self.len())
        {
            return false;
        }
        match &self.0 {
            Repr::Raw { bytes: raw, .. } => &raw[offset..offset + bytes.len()] == bytes,
            Repr::Packed(data) => {
                // Compare the requested portion directly, with one block
                // workspace. Calling copy_into here would retain a second
                // decoded block beneath the caller's own streaming block.
                range_comparison::single_packed_range_eq(data, offset, bytes)
            }
        }
    }
    pub fn equals_slice(&self, bytes: &[u8]) -> bool {
        self.len() == bytes.len() && self.range_eq(0, bytes)
    }
}
impl From<Arc<[u8]>> for RepositoryWorkspaceBody {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::from_arc(bytes)
    }
}
impl From<Vec<u8>> for RepositoryWorkspaceBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_arc(Arc::from(bytes))
    }
}
impl PartialEq for RepositoryWorkspaceBody {
    fn eq(&self, other: &Self) -> bool {
        if self.ptr_eq(other) {
            return true;
        }
        if self.len() != other.len() {
            return false;
        }
        let mut offset = 0;
        other.with_range_comparison(|compare| {
            self.try_for_each_chunk(&mut |chunk| {
                if !compare(offset, chunk) {
                    return Err(());
                }
                offset += chunk.len();
                Ok(())
            })
            .is_ok()
        })
    }
}
impl Eq for RepositoryWorkspaceBody {}
impl Hash for RepositoryWorkspaceBody {
    fn hash<H: Hasher>(&self, state: &mut H) {
        #[cfg(test)]
        record_work(true, self.len());
        self.len().hash(state);
        self.for_each_chunk(&mut |chunk| state.write(chunk));
    }
}
impl std::fmt::Debug for RepositoryWorkspaceBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceBody")
            .field("logical_bytes", &self.len())
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
std::thread_local! { static WORK: std::cell::Cell<(usize,usize,usize,usize)> = const {std::cell::Cell::new((0,0,0,0))}; }
#[cfg(test)]
fn record_work(hash: bool, bytes: usize) {
    WORK.with(|w| {
        let (h, hb, d, db) = w.get();
        w.set(if hash {
            (
                h.checked_add(1).unwrap(),
                hb.checked_add(bytes).unwrap(),
                d,
                db,
            )
        } else {
            (
                h,
                hb,
                d.checked_add(1).unwrap(),
                db.checked_add(bytes).unwrap(),
            )
        });
    });
}
#[cfg(test)]
pub(super) fn reset_work() {
    WORK.set((0, 0, 0, 0));
}
#[cfg(test)]
pub(super) fn work() -> (usize, usize, usize, usize) {
    WORK.get()
}
#[cfg(test)]
mod tests;
