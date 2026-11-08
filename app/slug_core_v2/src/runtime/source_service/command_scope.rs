//! Fresh command acquisition control on the existing native retention bank.
//! The bank is shared across commands; a fresh scope never creates a new budget.
use super::*;
use crate::runtime::repository_physical_root::RepositoryRootResources;
use crate::runtime::repository_physical_root::ResourceCredit;

pub(super) struct CommandScope {
    pub(super) resources: Arc<RepositoryRootResources>,
    // The scope payload and bank reference retire before their control credit.
    _control: ResourceCredit,
}

impl SourceServiceScope {
    pub(in crate::runtime) fn new_command(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
        resources: Arc<RepositoryRootResources>,
    ) -> Result<Arc<Self>, SourceServiceError> {
        Self::validate(work_deadline, cleanup_deadline, limits)?;
        let bytes = command_control_bytes(&workspace)?;
        // Keep the allowance armed before Scope Arc/watch construction begins.
        let control = resources
            .reserve(bytes, 0, 0)
            .map_err(|_| SourceServiceError::Allocation)?;
        let mut scope = Self::new(workspace, work_deadline, cleanup_deadline, limits)?;
        Arc::get_mut(&mut scope)
            .expect("new command scope is uniquely owned")
            .command = Some(CommandScope {
            resources,
            _control: control,
        });
        Ok(scope)
    }
    pub(in crate::runtime) fn command_resources(&self) -> Option<&Arc<RepositoryRootResources>> {
        self.command.as_ref().map(|command| &command.resources)
    }
    pub(super) fn reserve_command_bytes(
        &self,
        bytes: usize,
    ) -> Result<Option<ResourceCredit>, SourceServiceError> {
        self.command
            .as_ref()
            .map(|command| {
                command
                    .resources
                    .reserve(bytes, 0, 0)
                    .map_err(|_| SourceServiceError::Allocation)
            })
            .transpose()
    }
}

fn command_control_bytes(workspace: &NormalizedAbsolutePath) -> Result<usize, SourceServiceError> {
    let allocation = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<SourceServiceScope>())
        .map_err(|_| SourceServiceError::Allocation)?
        .0
        .pad_to_align()
        .size();
    // Pinned Tokio 1.53.1 watch::Shared<bool> is one Arc containing a RwLock,
    // state/ref-count atomics, eight Notify shards and one sender Notify.
    // Sixteen Notify/usize slots conservatively include that fixed control and
    // its Arc/alignment. Waiter futures retain their own enclosing IO owners.
    let watch = 16 * std::mem::size_of::<tokio::sync::Notify>()
        + std::mem::size_of::<std::sync::RwLock<bool>>()
        + 16 * std::mem::size_of::<usize>();
    allocation
        .checked_add(watch)
        .and_then(|bytes| bytes.checked_add(workspace.retained_storage_bytes()?))
        .ok_or(SourceServiceError::Allocation)
}

#[cfg(test)]
#[path = "command_scope_tests.rs"]
mod tests;
