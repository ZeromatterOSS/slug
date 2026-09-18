//! Content-only file facts and their observation-backed provenance.

use std::fmt;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;

use crate::NormalizedAbsolutePath;
use crate::ObservedPathFrontierError;
use crate::PathFileBytesError;
use crate::PathNodeKind;
use crate::PathObservationDemand;
use crate::PathObservationEpoch;
use crate::PathObservationKey;
use crate::PathObservationNamespace;
use crate::PathObservationOperation;
use crate::PathObservationResult;
use crate::PathOperationResult;
use crate::PathOutcome;
use crate::PathResult;
use crate::ResolvedPathObservationKey;
use crate::ResolvedPathState;

/// SHA-256 of the bytes actually read, and their REAPI-representable length.
/// This is content identity, not authority to upload a mutable host path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct FileContentDigest {
    sha256: [u8; 32],
    size_bytes: u64,
}

impl FileContentDigest {
    pub fn new(sha256: [u8; 32], size_bytes: u64) -> Option<Self> {
        (size_bytes <= i64::MAX as u64).then_some(Self { sha256, size_bytes })
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub const fn size_bytes(self) -> u64 {
        self.size_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum PathFileDigest {
    Present(FileContentDigest),
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum PathFileDigestError {
    Read(PathFileBytesError),
    Frontier(ObservedPathFrontierError),
    SizeChanged {
        logical_path: NormalizedAbsolutePath,
        expected: i64,
        observed: u64,
    },
}

/// The content result plus every observation required to validate its provenance.
/// Consumers must validate this frontier before publishing a request result.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct ObservedPathFileDigest {
    result: Result<PathFileDigest, PathFileDigestError>,
    observations: PathObservationEpoch,
}

impl ObservedPathFileDigest {
    pub fn result(&self) -> &Result<PathFileDigest, PathFileDigestError> {
        &self.result
    }
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct PathFileDigestKey {
    namespace: PathObservationNamespace,
    logical_path: NormalizedAbsolutePath,
}

impl PathFileDigestKey {
    pub fn new(namespace: PathObservationNamespace, logical_path: NormalizedAbsolutePath) -> Self {
        Self {
            namespace,
            logical_path,
        }
    }
}

impl fmt::Display for PathFileDigestKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "path-file-digest:{:?}:{:?}",
            self.namespace,
            self.logical_path.as_path()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct PathFileDigestObservationKey(PathFileDigestKey);

impl PathFileDigestObservationKey {
    pub fn new(namespace: PathObservationNamespace, logical_path: NormalizedAbsolutePath) -> Self {
        Self(PathFileDigestKey::new(namespace, logical_path))
    }
}

impl fmt::Display for PathFileDigestObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[async_trait]
impl Key for PathFileDigestKey {
    type Value = PathResult<PathFileDigest, PathFileDigestError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&PathFileDigestObservationKey(self.dupe()))
            .await
            .expect("digest observation DICE invariant")
            .map(|observed| match observed {
                Ok(observed) => observed.result,
                Err(error) => Err(PathFileDigestError::Frontier(error)),
            })
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for PathFileDigestObservationKey {
    type Value = PathResult<ObservedPathFileDigest, ObservedPathFrontierError>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let key = &self.0;
        let resolved = match ctx
            .compute(&ResolvedPathObservationKey::new(
                key.namespace,
                key.logical_path.dupe(),
            ))
            .await
            .expect("resolution DICE invariant")
        {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => return PathOutcome::Complete(Err(error)),
            PathOutcome::Complete(Ok(resolved)) => resolved,
        };
        let mut observations = resolved.observations().dupe();
        let complete = |result, observations| {
            PathOutcome::Complete(Ok(ObservedPathFileDigest {
                result,
                observations,
            }))
        };
        let read_error = |error| Err(PathFileDigestError::Read(error));
        let resolved = match resolved.result() {
            Ok(resolved) => resolved,
            Err(error) => {
                return complete(
                    read_error(PathFileBytesError::from_resolution(
                        key.logical_path.dupe(),
                        error.dupe(),
                    )),
                    observations,
                );
            }
        };
        let lstat = match resolved.state() {
            ResolvedPathState::Missing => {
                return complete(Ok(PathFileDigest::Missing), observations);
            }
            ResolvedPathState::Present(lstat) if lstat.kind() == PathNodeKind::RegularFile => lstat,
            ResolvedPathState::Present(lstat) => {
                return complete(
                    read_error(PathFileBytesError::WrongKind {
                        logical_path: key.logical_path.dupe(),
                        expected: PathNodeKind::RegularFile,
                        actual: lstat.kind(),
                    }),
                    observations,
                );
            }
        };
        let demand = PathObservationDemand::new(
            key.namespace,
            resolved.real_path().dupe(),
            PathObservationOperation::FileDigest,
        );
        let result = match ctx
            .compute(&PathObservationKey::new(demand.dupe()))
            .await
            .expect("digest observation DICE invariant")
        {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(result) => result,
        };
        observations = match PathObservationEpoch::from_shared(
            observations
                .observations()
                .iter()
                .map(|(demand, result)| (demand.dupe(), result.dupe()))
                .chain(std::iter::once((demand.dupe(), result.dupe()))),
        ) {
            Ok(epoch) => epoch,
            Err(error) => return PathOutcome::Complete(Err(error.into())),
        };
        let result = match result.as_ref() {
            PathObservationResult::FileDigest(PathOperationResult::Present(digest)) => {
                if u64::try_from(lstat.size()).ok() != Some(digest.size_bytes()) {
                    Err(PathFileDigestError::SizeChanged {
                        logical_path: key.logical_path.dupe(),
                        expected: lstat.size(),
                        observed: digest.size_bytes(),
                    })
                } else {
                    Ok(PathFileDigest::Present(*digest))
                }
            }
            PathObservationResult::FileDigest(PathOperationResult::Missing) => {
                read_error(PathFileBytesError::InconsistentState {
                    logical_path: key.logical_path.dupe(),
                    operation: demand.operation(),
                    before: Some(lstat),
                    after: None,
                })
            }
            PathObservationResult::FileDigest(PathOperationResult::Error(error)) => {
                read_error(PathFileBytesError::Observation {
                    logical_path: key.logical_path.dupe(),
                    operation: demand.operation(),
                    error: *error,
                })
            }
            _ => unreachable!("FileDigest demand must return FileDigest observation"),
        };
        complete(result, observations)
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests;
