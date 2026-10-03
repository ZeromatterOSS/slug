//! Private descriptor view of genuine normal namespace facts; no kernel mount.
#![allow(dead_code)]
use std::fs::File;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::GuestNamespaceTransaction;
use slug_loading_v2::GuestNamespaceBinding;
use slug_loading_v2::GuestNamespaceNode;
use slug_loading_v2::GuestNamespaceNodeKind;
use slug_loading_v2::ObservedGuestNamespaceFacts;
use slug_loading_v2::SelectedGuestNamespaceFactsKey;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;

use super::*;

#[path = "guest_namespace/attempt.rs"]
pub(super) mod attempt;
#[path = "guest_namespace/reader.rs"]
mod reader;
#[cfg(all(test, target_os = "linux"))]
#[path = "guest_namespace/tests.rs"]
pub(in crate::runtime::dice) mod tests;

fn failure(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}
pub(super) fn install_transaction(
    data: &mut UserComputationData,
    runtime: &WorkspaceRuntime,
    policy: Option<GuestNamespacePolicy>,
) -> anyhow::Result<()> {
    if let Some(policy) = policy {
        data.data.set(GuestNamespaceTransaction::new(
            runtime.repository_materializer.workspace().dupe(),
            Some(policy),
            runtime.image_resources.descriptor_budget().dupe(),
        )?);
    }
    Ok(())
}
pub(super) fn validate(
    command: &NativeDemandCommand<'_>,
    key: &SelectedGuestNamespaceFactsKey,
    facts: &ObservedGuestNamespaceFacts,
    profile: &slug_loading_v2::ObservedRuntimeSourceProfile,
) -> Result<(), NativeDemandSessionError> {
    let policy = command
        .inputs
        .request
        .guest_namespace
        .ok_or_else(|| failure("missing guest policy"))?;
    let tx = GuestNamespaceTransaction::new(
        command.runtime.repository_materializer.workspace().dupe(),
        Some(policy),
        command.runtime.image_resources.descriptor_budget().dupe(),
    )
    .map_err(|_| failure("guest transaction resource"))?;
    let scratch = command
        .scratch_metadata
        .as_ref()
        .ok_or_else(|| failure("missing scratch authority"))?
        .transaction()?;
    if !facts.matches_key(key)
        || facts.profile() != profile
        || facts.policy() != policy
        || !facts.same_resources(&command.runtime.scratch_resources)
        || tx.workspace() != key.workspace()
        || !tx.same_budget(command.runtime.image_resources.descriptor_budget())
        || !facts.authorized_by(
            &tx,
            &runtime_sources::declaration_transaction(command)?,
            &scratch,
            &command.runtime.scratch_resources,
        )
    {
        return Err(failure("foreign guest namespace authority"));
    }
    Ok(())
}
struct ViewOwner {
    facts: ObservedGuestNamespaceFacts,
    sources: runtime_sources::CertifiedRuntimeSourcesLease,
    data: Option<selected_label_data::prepare::Lease>,
    invalid: AtomicBool,
    budget: slug_workspace_v2::ImageDescriptorBudget,
}
#[derive(Clone, Dupe)]
pub(super) struct PreparedGuestNamespaceViewLease(BudgetedImageBlock<ViewOwner>);
impl fmt::Debug for PreparedGuestNamespaceViewLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedGuestNamespaceViewLease")
            .field("valid", &self.is_valid())
            .finish_non_exhaustive()
    }
}
impl WorkspaceRuntime {
    pub(super) fn prepare_selected_guest_namespace_view(
        &self,
        key: SelectedGuestNamespaceFactsKey,
        runtime_policy: Option<slug_bzlmod_v2::RuntimeSourceDeclarationPolicy>,
        guest_policy: Option<GuestNamespacePolicy>,
    ) -> Result<PreparedGuestNamespaceViewLease, NativeDemandSessionError> {
        self.prepare_selected_guest_namespace_view_inner(
            key,
            runtime_policy,
            guest_policy,
            #[cfg(test)]
            None,
        )
    }
    fn prepare_selected_guest_namespace_view_inner(
        &self,
        key: SelectedGuestNamespaceFactsKey,
        runtime_policy: Option<slug_bzlmod_v2::RuntimeSourceDeclarationPolicy>,
        guest_policy: Option<GuestNamespacePolicy>,
        #[cfg(test)] hook: Option<
            &dyn Fn(selected_tool_source::ToolPhase<'_>) -> Result<(), NativeDemandSessionError>,
        >,
    ) -> Result<PreparedGuestNamespaceViewLease, NativeDemandSessionError> {
        let guest_policy = guest_policy.ok_or_else(|| failure("missing guest policy"))?;
        let runtime_policy = runtime_policy.ok_or_else(|| failure("missing runtime policy"))?;
        if key.workspace().as_path() != self.workspace
            || runtime_policy.workspace().as_path() != self.workspace
            || !runtime_policy.same_budget(self.image_resources.descriptor_budget())
        {
            return Err(failure("foreign guest workspace or resources"));
        }
        let runtime_key = key.profile_key().clone();
        match self.prepare_selected_sources_inner(
            runtime_key.tool_sources_key(),
            selected_tool_source::PreparationMode::Namespace {
                key,
                runtime_key,
                runtime_policy,
                guest_policy,
            },
            #[cfg(test)]
            hook,
        )? {
            selected_tool_source::PreparedLease::Namespace(view) => Ok(view),
            _ => unreachable!("namespace preparation mode"),
        }
    }
}
pub(super) fn reserve_owner(
    runtime: &WorkspaceRuntime,
) -> Result<ImageDescriptorReservation, NativeDemandSessionError> {
    runtime
        .image_resources
        .descriptor_budget()
        .try_reserve(std::mem::size_of::<ViewOwner>() + 256)
        .map_err(|e| NativeDemandSessionError::InputImage(e.into()))
}
pub(in crate::runtime::dice) struct ViewCapture {
    partial: Option<ViewOwner>,
    complete: Option<PreparedGuestNamespaceViewLease>,
    credit: Option<ImageDescriptorReservation>,
}
impl ViewCapture {
    pub(in crate::runtime::dice) fn new(credit: ImageDescriptorReservation) -> Self {
        Self {
            partial: None,
            complete: None,
            credit: Some(credit),
        }
    }
}
pub(super) fn finish(
    facts: ObservedGuestNamespaceFacts,
    sources: runtime_sources::CertifiedRuntimeSourcesLease,
    data: Option<selected_label_data::prepare::Lease>,
    credit: ImageDescriptorReservation,
    budget: &slug_workspace_v2::ImageDescriptorBudget,
) -> Result<PreparedGuestNamespaceViewLease, NativeDemandSessionError> {
    finish_retaining(facts, sources, data, &mut ViewCapture::new(credit), budget)
}
pub(super) fn finish_retaining(
    facts: ObservedGuestNamespaceFacts,
    sources: runtime_sources::CertifiedRuntimeSourcesLease,
    data: Option<selected_label_data::prepare::Lease>,
    capture: &mut ViewCapture,
    budget: &slug_workspace_v2::ImageDescriptorBudget,
) -> Result<PreparedGuestNamespaceViewLease, NativeDemandSessionError> {
    if capture.partial.is_some() || capture.complete.is_some() {
        return Err(failure("guest capture already attempted"));
    }
    capture.partial = Some(ViewOwner {
        facts,
        sources,
        data,
        invalid: AtomicBool::new(false),
        budget: budget.dupe(),
    });
    let owner = capture.partial.as_ref().unwrap();
    if owner.facts.profile() != owner.sources.profile()
        || !capture
            .credit
            .as_ref()
            .is_some_and(|r| r.same_budget(budget))
    {
        return Err(failure("guest capture profile mismatch"));
    }
    if owner.facts.data_sources() != owner.data.as_ref().map(|lease| lease.sources()) {
        return Err(failure("guest data capture association mismatch"));
    }
    let view = match capture
        .credit
        .take()
        .unwrap()
        .freeze_preserving(capture.partial.take().unwrap())
    {
        Ok(block) => PreparedGuestNamespaceViewLease(block),
        Err((error, owner, credit)) => {
            capture.partial = Some(owner);
            capture.credit = Some(credit);
            return Err(NativeDemandSessionError::InputImage(error.into()));
        }
    };
    capture.complete = Some(view.dupe());
    for node in view.facts().nodes() {
        if !matches!(
            node.binding(),
            GuestNamespaceBinding::Scaffold | GuestNamespaceBinding::DataScaffold
        ) {
            view.with_bound_source(node, |_, _| Ok(()))
                .map_err(|_| failure("guest source association mismatch"))?;
        }
    }
    Ok(view)
}

impl PreparedGuestNamespaceViewLease {
    #[cfg(test)]
    pub(in crate::runtime::dice) fn sources(
        &self,
    ) -> &runtime_sources::CertifiedRuntimeSourcesLease {
        &self.0.get().sources
    }
    pub(super) fn facts(&self) -> &ObservedGuestNamespaceFacts {
        &self.0.get().facts
    }
    pub(super) fn is_valid(&self) -> bool {
        !self.0.get().invalid.load(Ordering::Acquire)
    }
    fn poison(&self) {
        self.0.get().invalid.store(true, Ordering::Release);
    }
    fn budget(&self) -> &slug_workspace_v2::ImageDescriptorBudget {
        &self.0.get().budget
    }
    fn with_bound_source<R>(
        &self,
        node: &GuestNamespaceNode,
        visitor: impl FnOnce(&File, &Path) -> Result<R, reader::ViewError>,
    ) -> Result<R, reader::ViewError> {
        if self.facts().node(node.index()) != Some(node) {
            return Err(reader::ViewError::Stale);
        }
        let sources = &self.0.get().sources;
        match node.binding() {
            GuestNamespaceBinding::Scaffold | GuestNamespaceBinding::DataScaffold => {
                Err(reader::ViewError::Stale)
            }
            GuestNamespaceBinding::DataLeaf { occurrence } => {
                self.with_data_source(occurrence, node, visitor)
            }
            GuestNamespaceBinding::ScratchEntry { entry } => {
                sources.tools().with_scratch_root(|root, observed| {
                    if observed != self.facts().profile().tool_sources().prefix() {
                        return Err(reader::ViewError::Stale);
                    }
                    let e = observed
                        .prefix()
                        .entries()
                        .get(entry)
                        .ok_or(reader::ViewError::Stale)?;
                    let matches = match (node.kind(), e.blob()) {
                        (GuestNamespaceNodeKind::Directory, None) => true,
                        (GuestNamespaceNodeKind::RegularFile(d), Some(blob)) => {
                            // FULL immutable observed prefix equality above binds
                            // the selected bytes. Actual reads verify the normal
                            // digest incrementally with cancellation per chunk.
                            d.size_bytes() == blob.bytes().len() as u64
                        }
                        _ => false,
                    };
                    if !matches || node.mode() != e.mode() {
                        return Err(reader::ViewError::Stale);
                    }
                    visitor(root, Path::new(e.path().relative()))
                })
            }
            GuestNamespaceBinding::ToolImage { role, entry } => {
                let expected = self
                    .facts()
                    .profile()
                    .tool_sources()
                    .sources()
                    .nth(role)
                    .and_then(|s| s.label_metadata())
                    .ok_or(reader::ViewError::Stale)?
                    .image();
                sources
                    .tools()
                    .with_image_root(role, |root, image| {
                        image_member(root, image, expected, entry, node, visitor)
                    })
                    .ok_or(reader::ViewError::Stale)?
            }
            GuestNamespaceBinding::RuntimeImage { declaration, entry } => {
                let expected = self
                    .facts()
                    .profile()
                    .images()
                    .nth(declaration)
                    .ok_or(reader::ViewError::Stale)?;
                sources
                    .with_image_source(declaration, |root, base, image| {
                        image_member_at_base(root, base, image, expected, entry, node, visitor)
                    })
                    .ok_or(reader::ViewError::Stale)?
            }
        }
    }
}
impl PreparedGuestNamespaceViewLease {
    fn with_data_source<R>(
        &self,
        occurrence: usize,
        node: &GuestNamespaceNode,
        visitor: impl FnOnce(&File, &Path) -> Result<R, reader::ViewError>,
    ) -> Result<R, reader::ViewError> {
        let lease = self.0.get().data.as_ref().ok_or(reader::ViewError::Stale)?;
        let expected = self
            .facts()
            .data_sources()
            .ok_or(reader::ViewError::Stale)?;
        if lease.sources() != expected || expected.profile() != self.facts().profile() {
            return Err(reader::ViewError::Stale);
        }
        let request = expected
            .requests()
            .requests()
            .nth(occurrence)
            .ok_or(reader::ViewError::Stale)?;
        if node.path()
            != request
                .selection()
                .selected_file()
                .as_path()
                .to_str()
                .ok_or(reader::ViewError::Stale)?
        {
            return Err(reader::ViewError::Stale);
        }
        let source = expected
            .sources()
            .nth(occurrence)
            .ok_or(reader::ViewError::Stale)?;
        let (image, entry) = expected.member(source).ok_or(reader::ViewError::Stale)?;
        match source.binding(){
            slug_loading_v2::SelectedDataLabelBinding::Leaf{..}=>lease.with_leaf(occurrence,|root,relative,actual|{
                if actual!=image || !actual.is_regular_file_root() || actual.entries().len()!=1{return Err(reader::ViewError::Stale)}
                let e=&actual.entries()[0];
                if e.mode()!=node.mode() || !matches!((node.kind(),e.kind()),(GuestNamespaceNodeKind::RegularFile(a),slug_workspace_v2::InputImageEntryKind::RegularFile(b)) if a==b){return Err(reader::ViewError::Stale)}
                visitor(root,relative)
            }).ok_or(reader::ViewError::Stale)?,
            slug_loading_v2::SelectedDataLabelBinding::ToolMember{role,..}=>self.0.get().sources.tools().with_image_root(*role,|root,actual|image_member(root,actual,image,entry,node,visitor)).ok_or(reader::ViewError::Stale)?,
            slug_loading_v2::SelectedDataLabelBinding::RuntimeMember{declaration,..}=>self.0.get().sources.with_image_source(*declaration,|root,base,actual|image_member_at_base(root,base,actual,image,entry,node,visitor)).ok_or(reader::ViewError::Stale)?,
        }
    }
}

fn image_member<R>(
    root: &File,
    image: &slug_workspace_v2::ObservedInputImage,
    expected: &slug_workspace_v2::ObservedInputImage,
    entry: usize,
    node: &GuestNamespaceNode,
    visitor: impl FnOnce(&File, &Path) -> Result<R, reader::ViewError>,
) -> Result<R, reader::ViewError> {
    image_member_at_base(root, Path::new(""), image, expected, entry, node, visitor)
}

fn image_member_at_base<R>(
    root: &File,
    base: &Path,
    image: &slug_workspace_v2::ObservedInputImage,
    expected: &slug_workspace_v2::ObservedInputImage,
    entry: usize,
    node: &GuestNamespaceNode,
    visitor: impl FnOnce(&File, &Path) -> Result<R, reader::ViewError>,
) -> Result<R, reader::ViewError> {
    if image != expected {
        return Err(reader::ViewError::Stale);
    }
    let e = image.entries().get(entry).ok_or(reader::ViewError::Stale)?;
    let same = match (node.kind(), e.kind()) {
        (GuestNamespaceNodeKind::Directory, slug_workspace_v2::InputImageEntryKind::Directory) => {
            true
        }
        (
            GuestNamespaceNodeKind::RegularFile(a),
            slug_workspace_v2::InputImageEntryKind::RegularFile(b),
        ) => a == b,
        _ => false,
    };
    if !same || node.mode() != e.mode() {
        return Err(reader::ViewError::Stale);
    }
    // Compose the certified relative base without allocating a second path:
    // strip the exact suffix from the normal absolute image root, then borrow
    // the member relative to that actual backing root. No ambient pathname IO.
    let mut backing_root = image.source_root().as_path();
    for component in base.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(reader::ViewError::Stale);
        }
        backing_root = backing_root.parent().ok_or(reader::ViewError::Stale)?;
    }
    if image
        .source_root()
        .as_path()
        .strip_prefix(backing_root)
        .ok()
        != Some(base)
    {
        return Err(reader::ViewError::Stale);
    }
    let relative = e
        .path()
        .as_path()
        .strip_prefix(backing_root)
        .map_err(|_| reader::ViewError::Stale)?;
    visitor(root, relative)
}
