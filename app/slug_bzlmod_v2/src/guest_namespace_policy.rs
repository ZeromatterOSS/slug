//! Explicit Slug-native guest metadata configuration; no source or IO authority.
use std::fmt;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::ImageDescriptorBudgetError;

#[cfg(test)]
mod tests;
mod transaction;
pub use transaction::GuestNamespaceTransaction;
pub use transaction::RawGuestNamespacePolicyKey;

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub enum GuestNamespaceMetadataScheme {
    UniformLinuxV1,
}

/// Semantic occurrence limits count source rows and scaffolds before sharing.
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct GuestNamespaceLimits {
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_path_bytes: usize,
    pub max_descriptor_bytes: usize,
    pub max_total_file_bytes: u64,
}
impl Default for GuestNamespaceLimits {
    fn default() -> Self {
        Self {
            max_entries: 32_768,
            max_depth: 32,
            max_path_bytes: 4096,
            max_descriptor_bytes: 4 << 20,
            max_total_file_bytes: 2 << 30,
        }
    }
}
impl GuestNamespaceLimits {
    fn validate(self) -> Result<(), GuestNamespacePolicyError> {
        let ceiling = Self::default();
        if self.max_entries == 0
            || self.max_entries > ceiling.max_entries
            || self.max_depth > ceiling.max_depth
            || self.max_path_bytes == 0
            || self.max_path_bytes > ceiling.max_path_bytes
            || self.max_descriptor_bytes > ceiling.max_descriptor_bytes
            || self.max_total_file_bytes > ceiling.max_total_file_bytes
        {
            return Err(GuestNamespacePolicyError::InvalidPolicy);
        }
        Ok(())
    }
}

/// UniformLinuxV1 is explicit configuration, not a projection of staging stat.
/// No default policy exists: credentials, virtual device and epoch are supplied.
#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct GuestNamespacePolicy {
    scheme: GuestNamespaceMetadataScheme,
    owner_uid: u32,
    owner_gid: u32,
    caller_uid: u32,
    caller_gid: u32,
    device: u64,
    epoch_seconds: i64,
    epoch_nanoseconds: u32,
    scaffold_mode: u32,
    limits: GuestNamespaceLimits,
}
impl GuestNamespacePolicy {
    pub fn new(
        owner_uid: u32,
        owner_gid: u32,
        caller_uid: u32,
        caller_gid: u32,
        device: u64,
        epoch_seconds: i64,
        epoch_nanoseconds: u32,
        scaffold_mode: u32,
        limits: GuestNamespaceLimits,
    ) -> Result<Self, GuestNamespacePolicyError> {
        limits.validate()?;
        if owner_uid == 0
            || caller_uid == 0
            || device == 0
            || epoch_seconds < 0
            || epoch_nanoseconds >= 1_000_000_000
            || scaffold_mode > 0o777
        {
            return Err(GuestNamespacePolicyError::InvalidPolicy);
        }
        Ok(Self {
            scheme: GuestNamespaceMetadataScheme::UniformLinuxV1,
            owner_uid,
            owner_gid,
            caller_uid,
            caller_gid,
            device,
            epoch_seconds,
            epoch_nanoseconds,
            scaffold_mode,
            limits,
        })
    }
    pub fn scheme(&self) -> GuestNamespaceMetadataScheme {
        self.scheme
    }
    pub fn owner_uid(&self) -> u32 {
        self.owner_uid
    }
    pub fn owner_gid(&self) -> u32 {
        self.owner_gid
    }
    pub fn caller_uid(&self) -> u32 {
        self.caller_uid
    }
    pub fn caller_gid(&self) -> u32 {
        self.caller_gid
    }
    pub fn device(&self) -> u64 {
        self.device
    }
    pub fn epoch_seconds(&self) -> i64 {
        self.epoch_seconds
    }
    pub fn epoch_nanoseconds(&self) -> u32 {
        self.epoch_nanoseconds
    }
    pub fn scaffold_mode(&self) -> u32 {
        self.scaffold_mode
    }
    pub fn limits(&self) -> GuestNamespaceLimits {
        self.limits
    }
}

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Allocative)]
pub enum GuestNamespacePolicyError {
    InvalidPolicy,
    Resource(ImageDescriptorBudgetError),
    UnauthorizedCurrentPolicy,
    Operational,
}
impl GuestNamespacePolicyError {
    pub fn is_transient(&self) -> bool {
        !matches!(self, Self::InvalidPolicy)
    }
}
impl From<ImageDescriptorBudgetError> for GuestNamespacePolicyError {
    fn from(value: ImageDescriptorBudgetError) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for GuestNamespacePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "guest namespace policy: {self:?}")
    }
}
impl std::error::Error for GuestNamespacePolicyError {}
