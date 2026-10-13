//! A consumed diagnostic engine: acquisition cannot escape into ordinary commands.
use std::future::Future;
#[cfg(test)]
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use super::super::source_preparation::PreparedSelectedSourceView;
use super::super::source_preparation::SelectedSourcePreparationRequest;
use super::super::*;
use super::SourceCounter;
use super::SourceServiceCounters;
use super::SourceServiceError;
use super::SourceServiceLimits;
use super::SourceServiceScope;

pub(in crate::runtime) enum WorkspaceSourceServices {
    Native,
    Diagnostic(Arc<SourceServiceScope>),
}

pub(in crate::runtime::dice) struct SourcePreparationDiagnosticRuntime {
    runtime: Option<WorkspaceRuntime>,
    scope: Arc<SourceServiceScope>,
    completed: bool,
}

#[derive(Clone, Dupe)]
pub(in crate::runtime::dice) struct ScopedPreparedSourceViewLease {
    // The captured owner is destroyed before its accounting pin.
    view: PreparedSelectedSourceView,
    scope: Arc<SourceServiceScope>,
}

#[cfg(test)]
pub(in crate::runtime::dice) trait DiagnosticObserver {
    fn after_restore<'a>(
        &'a self,
        runtime: &'a WorkspaceRuntime,
        prepared: &'a Result<PreparedSelectedSourceView, NativeDemandSessionError>,
    ) -> Pin<Box<dyn Future<Output = Result<(), NativeDemandSessionError>> + 'a>>;
}

impl SourcePreparationDiagnosticRuntime {
    pub(in crate::runtime::dice) fn new(
        workspace: NormalizedAbsolutePath,
        host: Arc<crate::runtime::ProcessHostOwner>,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
    ) -> anyhow::Result<Self> {
        let scope =
            SourceServiceScope::new(workspace.clone(), work_deadline, cleanup_deadline, limits)?;
        let runtime = WorkspaceRuntime::new_with_source_services(
            workspace.as_path(),
            host,
            WorkspaceSourceServices::Diagnostic(scope.clone()),
        )?;
        Ok(Self {
            runtime: Some(runtime),
            scope,
            completed: false,
        })
    }

    pub(in crate::runtime::dice) fn workspace(&self) -> &NormalizedAbsolutePath {
        self.scope.workspace()
    }

    pub(in crate::runtime::dice) fn resources(&self) -> &slug_bzlmod_v2::SelectedScratchResources {
        &self
            .runtime
            .as_ref()
            .expect("unconsumed diagnostic")
            .scratch_resources
    }

    pub(in crate::runtime::dice) fn descriptor_budget(
        &self,
    ) -> &slug_workspace_v2::ImageDescriptorBudget {
        self.runtime
            .as_ref()
            .expect("unconsumed diagnostic")
            .image_resources
            .descriptor_budget()
    }

    #[cfg(test)]
    pub(in crate::runtime::dice) fn inspect_for_test<T>(
        &self,
        inspect: impl FnOnce(&WorkspaceRuntime, &Arc<SourceServiceScope>) -> T,
    ) -> T {
        inspect(
            self.runtime.as_ref().expect("unconsumed diagnostic"),
            &self.scope,
        )
    }

    pub(in crate::runtime::dice) fn prepare_once(
        self,
        request: SelectedSourcePreparationRequest<'_>,
    ) -> Result<ScopedPreparedSourceViewLease, NativeDemandSessionError> {
        self.prepare_once_inner(
            request,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    pub(in crate::runtime::dice) fn prepare_once_observed(
        self,
        request: SelectedSourcePreparationRequest<'_>,
        hook: &dyn Fn(selected_tool_source::ToolPhase<'_>) -> Result<(), NativeDemandSessionError>,
        observer: &dyn DiagnosticObserver,
    ) -> Result<ScopedPreparedSourceViewLease, NativeDemandSessionError> {
        self.prepare_once_inner(request, Some((hook, observer)))
    }

    fn prepare_once_inner(
        mut self,
        request: SelectedSourcePreparationRequest<'_>,
        #[cfg(test)] observation: Option<(
            &dyn Fn(selected_tool_source::ToolPhase<'_>) -> Result<(), NativeDemandSessionError>,
            &dyn DiagnosticObserver,
        )>,
    ) -> Result<ScopedPreparedSourceViewLease, NativeDemandSessionError> {
        self.scope
            .check()
            .map_err(NativeDemandSessionError::SourceService)?;
        // No executor owned by this runtime may call this synchronous boundary.
        // The full existing driver owns one command and its restoration guard.
        let runtime = self.runtime.as_ref().expect("unconsumed diagnostic");
        #[cfg(not(test))]
        let prepared = runtime.prepare_selected_source_view(request);
        #[cfg(test)]
        let prepared = match observation {
            None => runtime.prepare_selected_source_view(request),
            Some((hook, observer)) => {
                let prepared = runtime.prepare_selected_source_view_observed(request, hook);
                // The callback awaits raw-only restoration reads directly in
                // this executor. It never nests another block_on invocation.
                let checked = runtime
                    .runtime
                    .block_on(runtime.source_cleanup(observer.after_restore(runtime, &prepared)));
                match (prepared, checked) {
                    (Err(primary), _) => Err(primary),
                    (Ok(view), Ok(())) => Ok(view),
                    (Ok(_), Err(error)) => Err(error),
                }
            }
        };
        let outcome = prepared.and_then(|view| {
            self.scope
                .close_acquisition()
                .map_err(NativeDemandSessionError::SourceService)?;
            Ok(view)
        });
        if outcome.is_err() {
            self.scope.cancel();
        }
        // Destruction is outside runtime.block_on, before successful export.
        drop(self.runtime.take());
        let view = outcome?;
        self.scope
            .check_closed_export()
            .map_err(NativeDemandSessionError::SourceService)?;
        self.completed = true;
        Ok(ScopedPreparedSourceViewLease {
            view,
            scope: self.scope.clone(),
        })
    }
}

impl Drop for SourcePreparationDiagnosticRuntime {
    fn drop(&mut self) {
        if !self.completed {
            self.scope.cancel();
        }
    }
}

impl ScopedPreparedSourceViewLease {
    pub(in crate::runtime::dice) fn view(
        &self,
    ) -> &guest_namespace::PreparedGuestNamespaceViewLease {
        self.view.view()
    }
    pub(in crate::runtime::dice) fn selected(
        &self,
    ) -> &slug_loading_v2::ObservedSelectedPreparationOwner {
        self.view.selected()
    }
    pub(in crate::runtime::dice) fn inputs(&self) -> &NativeDemandRequestInputBundle {
        self.view.inputs()
    }
    pub(in crate::runtime::dice) fn counters(&self) -> SourceServiceCounters {
        self.scope.snapshot()
    }
}

impl WorkspaceRuntime {
    pub(in crate::runtime::dice) fn source_scope(&self) -> Option<&Arc<SourceServiceScope>> {
        self.source_scope.as_ref()
    }

    #[cfg_attr(all(test, target_os = "linux"), track_caller)]
    pub(in crate::runtime::dice) fn check_source_scope(
        &self,
    ) -> Result<(), NativeDemandSessionError> {
        if let Some(scope) = self.source_scope() {
            scope
                .check()
                .map_err(NativeDemandSessionError::SourceService)?;
        }
        Ok(())
    }

    pub(in crate::runtime::dice) fn source_work_deadline(
        &self,
        inherited: Instant,
    ) -> Result<Instant, NativeDemandSessionError> {
        self.check_source_scope()?;
        Ok(self
            .source_scope()
            .map_or(inherited, |scope| inherited.min(scope.work_deadline())))
    }

    pub(in crate::runtime::dice) async fn source_cleanup<T, F>(
        &self,
        future: F,
    ) -> Result<T, NativeDemandSessionError>
    where
        F: Future<Output = Result<T, NativeDemandSessionError>>,
    {
        match self.source_scope() {
            None => future.await,
            Some(scope) => {
                // Keep the primary restoration error even if the enclosing
                // absolute cleanup clock expires during its final poll.
                let mut outcome = None;
                let completion = scope
                    .during_cleanup(async {
                        outcome = Some(future.await);
                        Ok(())
                    })
                    .await;
                match outcome {
                    Some(Err(primary)) => Err(primary),
                    Some(Ok(value)) => completion
                        .map(|()| value)
                        .map_err(NativeDemandSessionError::SourceService),
                    None => Err(NativeDemandSessionError::SourceService(
                        completion.err().expect("unfinished cleanup is refused"),
                    )),
                }
            }
        }
    }

    pub(in crate::runtime::dice) fn wait_source_idle(
        &self,
    ) -> Result<(), NativeDemandSessionError> {
        self.runtime.block_on(self.source_cleanup(async {
            self.dice.wait_for_idle().await;
            Ok(())
        }))
    }
}

impl NativeDemandCommand<'_> {
    #[cfg_attr(all(test, target_os = "linux"), track_caller)]
    pub(in crate::runtime::dice) fn check_source_scope(
        &self,
    ) -> Result<(), NativeDemandSessionError> {
        self.runtime.check_source_scope()?;
        if self.runtime.source_scope().is_some()
            && !self
                .runtime
                .repository_materializer
                .session_is_active(self.repository_session)
        {
            return Err(NativeDemandSessionError::StaleLease);
        }
        Ok(())
    }

    pub(in crate::runtime::dice) fn validate_source_progress(
        &self,
        needs: &slug_bzlmod_v2::SourcePreparationNeeds,
    ) -> Result<(), NativeDemandSessionError> {
        let Some(scope) = self.runtime.source_scope() else {
            return Ok(());
        };
        self.check_source_scope()?;
        // Validate the complete batch before any category starts acquisition.
        for request in needs.repository_materializations().values() {
            if &request.id.workspace != scope.workspace() {
                return Err(NativeDemandSessionError::SourceService(
                    SourceServiceError::ForeignScope,
                ));
            }
            if self
                .issued_requests
                .get(&request.id)
                .is_some_and(|known| known.as_ref() != request.as_ref())
            {
                return Err(NativeDemandSessionError::ConflictingRepository(
                    request.id.canonical_repo.clone(),
                ));
            }
        }
        for request in needs.selected_extension_downloads().values() {
            if request.workspace() != scope.workspace() {
                return Err(NativeDemandSessionError::SourceService(
                    SourceServiceError::ForeignScope,
                ));
            }
            if self
                .issued_downloads
                .get(request.owner())
                .is_some_and(|known| known != request)
            {
                return Err(NativeDemandSessionError::DownloadRequestConflict(
                    request.owner().unique_name().clone(),
                ));
            }
        }
        if needs
            .repository_environment()
            .is_some_and(|need| need.workspace() != scope.workspace())
        {
            return Err(NativeDemandSessionError::SourceService(
                SourceServiceError::ForeignScope,
            ));
        }
        // This diagnostic captures sources, and admits no child process owner.
        if !needs.repository_executes().is_empty() {
            return Err(NativeDemandSessionError::SourceService(
                scope.poison(SourceServiceError::UnsupportedSourceOwner),
            ));
        }
        for request in needs.repository_filesystem_operations().values() {
            if self
                .filesystem_transcript
                .find(request.id())
                .is_some_and(|known| known.request() != request.as_ref())
                || (self.filesystem_transcript.find(request.id()).is_none()
                    && request.id().call_index()
                        != self
                            .filesystem_transcript
                            .records_for(request.id().owner(), request.id().ordinal())
                            .count())
            {
                return Err(NativeDemandSessionError::ArchiveInternalNonProgress);
            }
        }
        // Counter access is a short lock and also checks sticky admission.
        scope
            .remaining(SourceCounter::SourceAttempts)
            .map_err(NativeDemandSessionError::SourceService)?;
        Ok(())
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "diagnostic_tests.rs"]
mod tests;
