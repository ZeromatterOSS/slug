use allocative::Allocative;
use dupe::Dupe;

use super::PublishedValueError;

macro_rules! id {
    ($name:ident, $limit:expr) => {
        #[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
        pub struct $name(pub(super) u32);
        impl $name {
            pub fn new(index: usize) -> Result<Self, PublishedValueError> {
                if index >= $limit {
                    return Err(PublishedValueError::Limit(stringify!($name)));
                }
                Ok(Self(index as u32))
            }
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}
id!(PublishedNodeId, super::NODE_LIMIT);
id!(PublishedImportId, super::NODE_LIMIT);
id!(PublishedRootSlot, super::NODE_LIMIT);
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub enum PublishedEdge {
    Local(PublishedNodeId),
    Imported {
        arena: PublishedImportId,
        node: PublishedNodeId,
    },
}
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Allocative)]
pub struct PublishedEdgeRange {
    pub(super) start: u32,
    pub(super) len: u32,
}
