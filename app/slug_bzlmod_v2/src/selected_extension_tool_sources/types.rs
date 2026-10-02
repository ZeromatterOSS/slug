use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudgetError;

use crate::HostSelectedExtensionOwner;
use crate::SelectedScratchPolicy;
use crate::SelectedScratchPrefix;
use crate::SelectedScratchResources;

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub enum SelectedToolRole {
    Generator,
    Cargo,
    Rustc,
}

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct SelectedToolOperand {
    pub role: SelectedToolRole,
    pub argument_index: u16,
}

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct SelectedToolSourcesPolicy {
    operands: [SelectedToolOperand; 3],
    len: u8,
    elf: ElfMetadataPolicy,
}
impl SelectedToolSourcesPolicy {
    pub fn new(
        operands: &[SelectedToolOperand],
        elf: ElfMetadataPolicy,
    ) -> Result<Self, SelectedToolSourceError> {
        if operands.is_empty()
            || operands.len() > 3
            || !operands
                .iter()
                .any(|p| p.role == SelectedToolRole::Generator && p.argument_index == 0)
        {
            return Err(SelectedToolSourceError::InvalidPolicy);
        }
        let mut result = Self {
            operands: [SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            }; 3],
            len: operands.len() as u8,
            elf,
        };
        for (i, operand) in operands.iter().enumerate() {
            if (operand.role == SelectedToolRole::Generator && operand.argument_index != 0)
                || operands[..i].iter().any(|prior| {
                    prior.role == operand.role || prior.argument_index == operand.argument_index
                })
            {
                return Err(SelectedToolSourceError::InvalidPolicy);
            }
            result.operands[i] = *operand;
        }
        Ok(result)
    }
    pub fn operands(&self) -> &[SelectedToolOperand] {
        &self.operands[..self.len as usize]
    }
    pub fn elf_policy(&self) -> ElfMetadataPolicy {
        self.elf
    }
}

#[derive(Debug, Clone, Copy, Dupe, Eq, PartialEq, Allocative)]
pub enum SelectedToolSourceError {
    InvalidPolicy,
    MissingArgument,
    LiteralOperand,
    MissingEntry,
    WrongKind,
    NotExecutable,
    InvalidRequest,
    ConflictingRequest,
    ConflictingCheckpoint,
    TooManyRequests,
    MissingResources,
    ForeignResources,
    MissingTransaction,
    UnauthorizedRequest,
    ObservationMismatch,
    Descriptor(ImageDescriptorBudgetError),
    Allocation,
}
impl SelectedToolSourceError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::MissingResources
                | Self::ForeignResources
                | Self::MissingTransaction
                | Self::UnauthorizedRequest
                | Self::ObservationMismatch
                | Self::Descriptor(_)
                | Self::Allocation
        )
    }
}
impl From<ImageDescriptorBudgetError> for SelectedToolSourceError {
    fn from(value: ImageDescriptorBudgetError) -> Self {
        Self::Descriptor(value)
    }
}
impl fmt::Display for SelectedToolSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selected tool source: {self:?}")
    }
}
impl std::error::Error for SelectedToolSourceError {}

#[derive(Allocative)]
struct SelectorInner {
    workspace: ImageChargedPath,
    owner: Arc<HostSelectedExtensionOwner>,
    scratch_policy: SelectedScratchPolicy,
    policy: SelectedToolSourcesPolicy,
    resources: SelectedScratchResources,
}
impl PartialEq for SelectorInner {
    fn eq(&self, other: &Self) -> bool {
        self.workspace == other.workspace
            && self.owner == other.owner
            && self.scratch_policy == other.scratch_policy
            && self.policy == other.policy
    }
}
impl Eq for SelectorInner {}
impl Hash for SelectorInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        self.owner.hash(state);
        self.scratch_policy.hash(state);
        self.policy.hash(state);
    }
}

#[derive(Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct SelectedScratchElfMetadataSelector(BudgetedImageBlock<SelectorInner>);
impl SelectedScratchElfMetadataSelector {
    pub fn from_prefix(
        prefix: &SelectedScratchPrefix,
        policy: SelectedToolSourcesPolicy,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        if !prefix.same_resources(resources) {
            return Err(SelectedToolSourceError::ForeignResources);
        }
        Ok(Self(resources.descriptor_budget().try_reserve(0)?.freeze(
            SelectorInner {
                workspace: prefix.workspace().dupe(),
                owner: prefix.owner().dupe(),
                scratch_policy: *prefix.policy(),
                policy,
                resources: resources.dupe(),
            },
        )?))
    }
    pub fn workspace(&self) -> &ImageChargedPath {
        &self.0.get().workspace
    }
    pub fn owner(&self) -> &Arc<HostSelectedExtensionOwner> {
        &self.0.get().owner
    }
    pub fn scratch_policy(&self) -> SelectedScratchPolicy {
        self.0.get().scratch_policy
    }
    pub fn policy(&self) -> SelectedToolSourcesPolicy {
        self.0.get().policy
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.0.get().resources.same_instance(resources)
    }
    pub(super) fn resources(&self) -> &SelectedScratchResources {
        &self.0.get().resources
    }
}
impl fmt::Debug for SelectedScratchElfMetadataSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchElfMetadataSelector")
            .field("policy", &self.policy())
            .finish_non_exhaustive()
    }
}
