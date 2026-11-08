//! Command lifetime identity is separate from permanent acquisition services.
use super::*;

impl RepositoryMaterializer {
    pub(in crate::runtime) fn begin_with_command_scope(
        &self,
        scope: Option<Arc<SourceServiceScope>>,
    ) -> Result<RepositorySessionToken, RepositorySessionError> {
        let mut state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        if state.active.is_some() {
            return Err(RepositorySessionError::Busy);
        }
        match (&self.source_scope, &scope) {
            (Some(original), Some(current)) if Arc::ptr_eq(original, current) => {}
            (None, None) => {}
            (None, Some(current)) => {
                #[cfg(all(
                    target_os = "linux",
                    target_arch = "x86_64",
                    target_pointer_width = "64"
                ))]
                if current.command_resources().is_none() {
                    return Err(RepositorySessionError::SourceService(
                        SourceServiceError::UnsupportedSourceOwner,
                    ));
                }
                #[cfg(not(all(
                    target_os = "linux",
                    target_arch = "x86_64",
                    target_pointer_width = "64"
                )))]
                return Err(RepositorySessionError::SourceService(
                    SourceServiceError::UnsupportedSourceOwner,
                ));
            }
            _ => {
                return Err(RepositorySessionError::SourceService(
                    SourceServiceError::ForeignScope,
                ));
            }
        }
        if let Some(scope) = &scope {
            if scope.workspace() != &self.workspace {
                return Err(RepositorySessionError::SourceService(
                    SourceServiceError::ForeignScope,
                ));
            }
            scope
                .check()
                .map_err(RepositorySessionError::SourceService)?;
        }
        let token = RepositorySessionToken(
            super::super::native_identity::next_repository_session()
                .map_err(|_| RepositorySessionError::TokenExhausted)?,
        );
        let workspace_lease = super::super::native_identity::next_workspace_lease()
            .map_err(|_| RepositorySessionError::TokenExhausted)?;
        state.active = Some(ActiveRepositorySession {
            token,
            workspace_lease,
            command_scope: scope,
            #[cfg(target_os = "linux")]
            general: None,
            validation: RepositorySessionValidation::Pending,
            reusable: Vec::new(),
            entries: Vec::new(),
            provisional_roots: Vec::new(),
            #[cfg(target_os = "linux")]
            source_snapshots: Vec::new(),
            #[cfg(target_os = "linux")]
            source_snapshot_credit: None,
        });
        Ok(token)
    }

    pub(in crate::runtime) fn command_source_scope(
        &self,
        token: RepositorySessionToken,
    ) -> Result<Option<Arc<SourceServiceScope>>, RepositorySessionError> {
        let scope = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            matching_active(&state, token)?.command_scope.clone()
        };
        if let Some(scope) = &scope {
            scope
                .check()
                .map_err(RepositorySessionError::SourceService)?;
        }
        Ok(scope)
    }

    pub(in crate::runtime) fn check_command_scope(
        &self,
        token: RepositorySessionToken,
        supplied: &Arc<SourceServiceScope>,
    ) -> Result<(), RepositorySessionError> {
        let current = self.command_source_scope(token)?;
        source_service::check_identity(current.as_ref(), Some(supplied))
            .map_err(RepositorySessionError::SourceService)
    }
}

fn same_scope(
    left: Option<&Arc<SourceServiceScope>>,
    right: Option<&Arc<SourceServiceScope>>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        _ => false,
    }
}

pub(super) fn scope_is_current(
    active: &ActiveRepositorySession,
    expected: Option<&Arc<SourceServiceScope>>,
) -> bool {
    same_scope(active.command_scope.as_ref(), expected)
}
