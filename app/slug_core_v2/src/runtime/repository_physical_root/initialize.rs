//! Fixed native factory for a reserved, claimed general root. Registry callers
//! receive only the consuming command, never a root File or mount seed.
use super::mount_lease::launch::BootstrapLaunch;
use super::process_owner::ProcessPlatform;
use super::process_owner::{self};
use super::progress::command::RepositoryWorkspaceCommand;
use super::*;
pub(super) fn platform() -> io::Result<ProcessPlatform> {
    #[cfg(any(test, feature = "repository-workspace-test-support"))]
    {
        let image = std::env::var_os("SLUG_V2_BIN").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "actual compiled CLI required for native workspace initialization",
            )
        })?;
        Ok(ProcessPlatform::compiled_cli(
            image.into(),
            "/home/linuxbrew/.linuxbrew/Cellar/bubblewrap/0.11.2/bin/bwrap".into(),
        ))
    }
    #[cfg(not(any(test, feature = "repository-workspace-test-support")))]
    {
        ProcessPlatform::current()
    }
}
impl RepositoryWorkspaceCommand {
    pub(in crate::runtime) async fn initialize_claimed_root(
        self,
        reservation: Arc<RepositoryWorkspaceReservation>,
        claim: Arc<crate::runtime::repository_io::reservations::SourceRootClaim>,
        resources: Arc<RepositoryRootResources>,
        scope: Arc<SourceServiceScope>,
        active: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, PhysicalRootError> {
        self.check_initialization(&reservation, &claim, &resources, &scope, active)?;
        let producer = BootstrapLaunch::new_claimed_in_domain(
            reservation,
            resources,
            scope.clone(),
            claim,
            self.mount_domain().cloned(),
            active,
        )?
        .with_retirement_observer(self.retirement_observer());
        #[cfg(test)]
        let producer = producer.with_process_probe(self.process_probe());
        let (builder, initial) = process_owner::run(producer, platform()?, active).await?;
        self.reserve_root(builder, initial, &scope, active)
    }
}

#[cfg(test)]
mod tests;
