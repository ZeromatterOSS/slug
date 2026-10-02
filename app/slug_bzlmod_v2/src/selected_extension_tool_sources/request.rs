use std::fmt;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ElfSourceDigestBinding;
use slug_workspace_v2::VerifiedElfMetadata;

use super::SelectedScratchElfMetadataSelector;
use super::SelectedToolOperand;
use super::SelectedToolSourceError;
use super::SelectedToolSourcesPolicy;
use crate::SelectedScratchBlob;
use crate::SelectedScratchExternalPath;
use crate::SelectedScratchOperand;
use crate::SelectedScratchOperation;
use crate::SelectedScratchPath;
use crate::SelectedScratchPrefix;
use crate::SelectedScratchResources;

#[derive(Eq, PartialEq, Allocative)]
enum SelectionKind {
    Scratch { entry_index: usize },
    External,
}
#[derive(Eq, PartialEq, Allocative)]
struct SelectionInner {
    selector: SelectedScratchElfMetadataSelector,
    prefix: SelectedScratchPrefix,
    operand: SelectedToolOperand,
    kind: SelectionKind,
}
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct SelectedToolSourceSelection(BudgetedImageBlock<SelectionInner>);
impl SelectedToolSourceSelection {
    pub fn from_prefix(
        prefix: &SelectedScratchPrefix,
        policy: SelectedToolSourcesPolicy,
        operand: SelectedToolOperand,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        if !prefix.same_resources(resources) {
            return Err(SelectedToolSourceError::ForeignResources);
        }
        if !policy.operands().contains(&operand) {
            return Err(SelectedToolSourceError::InvalidPolicy);
        }
        let argument = prefix
            .process()
            .arguments()
            .get(operand.argument_index as usize)
            .ok_or(SelectedToolSourceError::MissingArgument)?;
        let kind = match argument {
            SelectedScratchOperand::Literal(_) => {
                return Err(SelectedToolSourceError::LiteralOperand);
            }
            SelectedScratchOperand::Scratch(path) => {
                let entry_index = prefix
                    .entries()
                    .iter()
                    .position(|entry| entry.path() == path)
                    .ok_or(SelectedToolSourceError::MissingEntry)?;
                let entry = &prefix.entries()[entry_index];
                if entry.blob().is_none() {
                    return Err(SelectedToolSourceError::WrongKind);
                }
                if entry.mode() & 0o111 == 0 {
                    return Err(SelectedToolSourceError::NotExecutable);
                }
                SelectionKind::Scratch { entry_index }
            }
            SelectedScratchOperand::ExternalPath(path) => {
                if !prefix.operations().iter().any(|op| {
                    matches!(op, SelectedScratchOperation::ExternalPath { path: issued } if issued == path)
                }) {
                    return Err(SelectedToolSourceError::InvalidRequest);
                }
                SelectionKind::External
            }
        };
        let selector = SelectedScratchElfMetadataSelector::from_prefix(prefix, policy, resources)?;
        Ok(Self(resources.descriptor_budget().try_reserve(0)?.freeze(
            SelectionInner {
                selector,
                prefix: prefix.dupe(),
                operand,
                kind,
            },
        )?))
    }
    pub fn selector(&self) -> &SelectedScratchElfMetadataSelector {
        &self.0.get().selector
    }
    pub fn prefix(&self) -> &SelectedScratchPrefix {
        &self.0.get().prefix
    }
    pub fn operand(&self) -> SelectedToolOperand {
        self.0.get().operand
    }
    pub fn spelling(&self) -> &str {
        self.prefix().process().arguments()[self.operand().argument_index as usize].spelling()
    }
    pub fn scratch_entry(
        &self,
    ) -> Option<(usize, &SelectedScratchPath, &SelectedScratchBlob, u32)> {
        let SelectionKind::Scratch { entry_index } = self.0.get().kind else {
            return None;
        };
        let entry = &self.prefix().entries()[entry_index];
        Some((
            entry_index,
            entry.path(),
            entry.blob().expect("validated regular entry"),
            entry.mode(),
        ))
    }
    pub fn external_path(&self) -> Option<&SelectedScratchExternalPath> {
        self.prefix().process().arguments()[self.operand().argument_index as usize].external_path()
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.selector().same_resources(resources)
            && self.prefix().same_resources(resources)
            && self
                .scratch_entry()
                .is_none_or(|(_, _, blob, _)| blob.same_resources(resources))
    }
}
impl fmt::Debug for SelectedToolSourceSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedToolSourceSelection")
            .field("operand", &self.operand())
            .field("scratch", &self.scratch_entry().is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Eq, PartialEq, Allocative)]
struct RequestInner {
    selection: SelectedToolSourceSelection,
    expected_digest: ElfSourceDigestBinding,
}
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct SelectedScratchElfMetadataRequest(BudgetedImageBlock<RequestInner>);
impl SelectedScratchElfMetadataRequest {
    pub fn new(
        selection: &SelectedToolSourceSelection,
        expected_digest: ElfSourceDigestBinding,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        if !selection.same_resources(resources) {
            return Err(SelectedToolSourceError::ForeignResources);
        }
        let (_, _, blob, _) = selection
            .scratch_entry()
            .ok_or(SelectedToolSourceError::WrongKind)?;
        if expected_digest.size_bytes() != blob.bytes().len() as u64 {
            return Err(SelectedToolSourceError::InvalidRequest);
        }
        Ok(Self(resources.descriptor_budget().try_reserve(0)?.freeze(
            RequestInner {
                selection: selection.dupe(),
                expected_digest,
            },
        )?))
    }
    pub fn selection(&self) -> &SelectedToolSourceSelection {
        &self.0.get().selection
    }
    pub fn selector(&self) -> &SelectedScratchElfMetadataSelector {
        self.selection().selector()
    }
    pub fn prefix(&self) -> &SelectedScratchPrefix {
        self.selection().prefix()
    }
    pub fn operand(&self) -> SelectedToolOperand {
        self.selection().operand()
    }
    pub fn entry_index(&self) -> usize {
        self.selection().scratch_entry().unwrap().0
    }
    pub fn member(&self) -> &SelectedScratchPath {
        self.selection().scratch_entry().unwrap().1
    }
    pub fn blob(&self) -> &SelectedScratchBlob {
        self.selection().scratch_entry().unwrap().2
    }
    pub fn mode(&self) -> u32 {
        self.selection().scratch_entry().unwrap().3
    }
    pub fn expected_digest(&self) -> ElfSourceDigestBinding {
        self.0.get().expected_digest
    }
    pub fn policy(&self) -> ElfMetadataPolicy {
        self.selector().policy().elf_policy()
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.selection().same_resources(resources)
    }
}
impl fmt::Debug for SelectedScratchElfMetadataRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchElfMetadataRequest")
            .field("operand", &self.operand())
            .field("entry_index", &self.entry_index())
            .field("digest", &self.expected_digest())
            .finish_non_exhaustive()
    }
}

#[derive(Eq, PartialEq, Allocative)]
struct ResultInner {
    request: SelectedScratchElfMetadataRequest,
    verified: VerifiedElfMetadata,
}
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct SelectedScratchElfMetadataResult(BudgetedImageBlock<ResultInner>);
impl SelectedScratchElfMetadataResult {
    pub fn new(
        request: &SelectedScratchElfMetadataRequest,
        verified: VerifiedElfMetadata,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        if !request.same_resources(resources)
            || !verified.same_budget(resources.descriptor_budget())
        {
            return Err(SelectedToolSourceError::ForeignResources);
        }
        if verified.digest() != request.expected_digest() || verified.policy() != request.policy() {
            return Err(SelectedToolSourceError::ObservationMismatch);
        }
        Ok(Self(resources.descriptor_budget().try_reserve(0)?.freeze(
            ResultInner {
                request: request.dupe(),
                verified,
            },
        )?))
    }
    pub fn request(&self) -> &SelectedScratchElfMetadataRequest {
        &self.0.get().request
    }
    pub fn verified(&self) -> &VerifiedElfMetadata {
        &self.0.get().verified
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.request().same_resources(resources)
            && self.verified().same_budget(resources.descriptor_budget())
    }
}
impl fmt::Debug for SelectedScratchElfMetadataResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchElfMetadataResult")
            .field("request", self.request())
            .finish_non_exhaustive()
    }
}
