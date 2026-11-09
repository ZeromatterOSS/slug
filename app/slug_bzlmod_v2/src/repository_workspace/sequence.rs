use async_trait::async_trait;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceRetention {
    state_versions: usize,
    executes: usize,
    metadata_bytes: usize,
    body_bytes: u64,
}
impl RepositoryWorkspaceRetention {
    pub fn state_versions(self) -> usize {
        self.state_versions
    }
    pub fn executes(self) -> usize {
        self.executes
    }
    pub fn metadata_bytes(self) -> usize {
        self.metadata_bytes
    }
    pub fn body_bytes(self) -> u64 {
        self.body_bytes
    }
    fn check(
        self,
        limits: RepositoryWorkspaceRetentionLimits,
        process: bool,
    ) -> Result<(), RepositoryWorkspaceError> {
        let minimum = self
            .state_versions
            .checked_mul(RepositoryWorkspaceState::minimum_metadata_bytes()?)
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
        if minimum > self.metadata_bytes
            || self.metadata_bytes > limits.metadata_bytes
            || self.body_bytes > limits.archive_body_bytes
            || (process && self.body_bytes > REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES)
        {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        if self.executes > REPOSITORY_WORKSPACE_EXECUTES {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceRequest {
    reservation: Arc<RepositoryWorkspaceReservation>,
    index: usize,
    predecessor: Arc<RepositoryWorkspaceState>,
    operation: RepositoryWorkspaceOperation,
}
impl RepositoryWorkspaceRequest {
    /// Exact comparison whose every state edge uses the caller's bounded
    /// comparator. No digest substitutes for structural equality.
    #[doc(hidden)]
    pub fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            reservation,
            index,
            predecessor,
            operation,
        } = self;
        let Self {
            reservation: other_reservation,
            index: other_index,
            predecessor: other_predecessor,
            operation: other_operation,
        } = other;
        if reservation != other_reservation || index != other_index {
            return Ok(false);
        }
        if !compare(
            RepositoryWorkspaceReadViewStateRef::Workspace(predecessor),
            RepositoryWorkspaceReadViewStateRef::Workspace(other_predecessor),
        )? {
            return Ok(false);
        }
        operation.try_eq_with_states(other_operation, compare)
    }

    pub fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        &self.reservation
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn predecessor(&self) -> &Arc<RepositoryWorkspaceState> {
        &self.predecessor
    }
    pub fn operation(&self) -> &RepositoryWorkspaceOperation {
        &self.operation
    }

    /// Complete conservative native request guard. Equal foreign owners are
    /// charged fully until protocol completion replaces them; sharing may be
    /// overcounted, never silently omitted.
    #[doc(hidden)]
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let mut bytes = add(
            std::mem::size_of::<Self>(),
            add(
                self.reservation.retained_storage_bytes()?,
                add(
                    self.predecessor.storage_partition()?.owned_bytes(),
                    self.operation.metadata()?,
                )?,
            )?,
        )?;
        let mut failure = None;
        self.operation.bodies(&mut |body| {
            if failure.is_none() {
                match array_bytes::<u8>(body.len()).and_then(|size| add(bytes, size)) {
                    Ok(next) => bytes = next,
                    Err(error) => failure = Some(error),
                }
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        RepositoryWorkspaceStoragePartition::new(
            bytes,
            read_views::source_storage_bytes(|visit| {
                self.visit_read_views(&mut |view| visit(view))
            })?,
        )
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.storage_partition()?.total_bytes()
    }
    pub fn visit_read_views(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        self.predecessor.visit_read_views(visit);
        self.operation.visit_read_views(visit)
    }
    /// Prepay the candidate Vec before calling source_candidates; no source body is copied.
    pub fn source_candidates_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        state_sources::BindingSelection::new(
            &self.reservation,
            &[],
            self.predecessor
                .source_bindings()
                .iter()
                .chain(self.operation.direct_source_candidates()),
        )?;
        let count = self
            .predecessor
            .source_bindings()
            .len()
            .checked_add(self.operation.direct_source_candidates().count())
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
        let mut bytes = array_bytes::<RepositoryWorkspaceReadView>(count)?;
        for view in self
            .predecessor
            .source_bindings()
            .iter()
            .chain(self.operation.direct_source_candidates())
        {
            bytes = add(bytes, view.storage_partition()?.owned_bytes())?;
        }
        Ok(bytes)
    }
    pub fn source_candidates(
        &self,
    ) -> Result<Vec<RepositoryWorkspaceReadView>, RepositoryWorkspaceError> {
        self.source_candidates_storage_bytes()?;
        let capacity = add(
            self.predecessor.source_bindings().len(),
            self.operation.direct_source_candidates().count(),
        )?;
        let mut rows = Vec::with_capacity(capacity);
        for view in self
            .predecessor
            .source_bindings()
            .iter()
            .chain(self.operation.direct_source_candidates())
        {
            if !rows.iter().any(|old| old == view) {
                rows.push(view.clone());
            }
        }
        Ok(rows)
    }
    pub fn with_symlink_source(
        mut self,
        source: RepositoryWorkspaceReadView,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let RepositoryWorkspaceOperation::Symlink { source: slot, .. } = &mut self.operation else {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        };
        if source.source_state().is_none() {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        }
        *slot = Some(source);
        self.validate_symlink()?;
        Ok(self)
    }
    fn validate_symlink(&self) -> Result<(), RepositoryWorkspaceError> {
        let RepositoryWorkspaceOperation::Symlink { target, source, .. } = &self.operation else {
            return Ok(());
        };
        if let Some(view) = source {
            state_sources::BindingSelection::new(&self.reservation, &[], std::iter::once(view))?;
            if !target.as_path().starts_with(view.display_root().as_path()) {
                return Err(RepositoryWorkspaceError::InvalidPath);
            }
        } else if !target
            .as_path()
            .starts_with(self.reservation.display_root().as_path())
        {
            return Err(RepositoryWorkspaceError::InvalidPath);
        }
        Ok(())
    }
    /// Rehome an independently validated equal source projection without
    /// changing argv/env/cwd or copying any source body. Core checks full Eq.
    pub fn with_read_views(
        mut self,
        manifest: RepositoryWorkspaceReadViewManifest,
    ) -> Result<Self, RepositoryWorkspaceError> {
        manifest.validate_for(&self.reservation)?;
        let RepositoryWorkspaceOperation::Execute(execute) = self.operation else {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        };
        self.operation = RepositoryWorkspaceOperation::Execute(execute.with_read_views(manifest)?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceRecord {
    request: RepositoryWorkspaceRequest,
    result: RepositoryWorkspaceResult,
    next: Arc<RepositoryWorkspaceState>,
}
impl RepositoryWorkspaceRecord {
    /// Exact comparison whose every state edge uses the caller's bounded
    /// comparator. No digest substitutes for structural equality.
    #[doc(hidden)]
    pub(super) fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            request,
            result,
            next,
        } = self;
        let Self {
            request: other_request,
            result: other_result,
            next: other_next,
        } = other;
        if !request.try_eq_with_states(other_request, compare)? || result != other_result {
            return Ok(false);
        }
        compare(
            RepositoryWorkspaceReadViewStateRef::Workspace(next),
            RepositoryWorkspaceReadViewStateRef::Workspace(other_next),
        )
    }

    pub fn request(&self) -> &RepositoryWorkspaceRequest {
        &self.request
    }
    pub fn result(&self) -> &RepositoryWorkspaceResult {
        &self.result
    }
    pub fn next(&self) -> &Arc<RepositoryWorkspaceState> {
        &self.next
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceTranscript {
    reservation: Arc<RepositoryWorkspaceReservation>,
    initial: Arc<RepositoryWorkspaceState>,
    records: Arc<[Arc<RepositoryWorkspaceRecord>]>,
    retention: RepositoryWorkspaceRetention,
}
impl RepositoryWorkspaceTranscript {
    /// Exact comparison whose every state edge uses the caller's bounded
    /// comparator. No digest substitutes for structural equality.
    #[doc(hidden)]
    pub fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            reservation,
            initial,
            records,
            retention,
        } = self;
        let Self {
            reservation: other_reservation,
            initial: other_initial,
            records: other_records,
            retention: other_retention,
        } = other;
        if reservation != other_reservation
            || retention != other_retention
            || records.len() != other_records.len()
        {
            return Ok(false);
        }
        if !compare(
            RepositoryWorkspaceReadViewStateRef::Workspace(initial),
            RepositoryWorkspaceReadViewStateRef::Workspace(other_initial),
        )? {
            return Ok(false);
        }
        for (record, other) in records.iter().zip(other_records.iter()) {
            if !record.try_eq_with_states(other, compare)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn new(
        reservation: Arc<RepositoryWorkspaceReservation>,
        root_mode: u16,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let budget = reservation.limits;
        Self::new_with_budget(reservation, root_mode, budget)
    }
    fn initial_retention(
        reservation: &RepositoryWorkspaceReservation,
        root_mode: u16,
        budget: RepositoryWorkspaceRetentionLimits,
    ) -> Result<(RepositoryWorkspaceEntry, RepositoryWorkspaceRetention), RepositoryWorkspaceError>
    {
        let root = RepositoryWorkspaceEntry::new(
            "".into(),
            root_mode,
            RepositoryWorkspaceNode::Directory,
        )?;
        let entries = [root];
        let metadata = RepositoryWorkspaceState::validate(
            reservation,
            RepositoryWorkspaceMode::Archive,
            &entries,
        )?;
        let retention = RepositoryWorkspaceRetention {
            state_versions: 1,
            executes: 0,
            metadata_bytes: add(
                metadata,
                add(
                    array_bytes::<RepositoryWorkspaceEntry>(1)?,
                    add(std::mem::size_of::<Self>(), reservation.metadata()?)?,
                )?,
            )?,
            body_bytes: 0,
        };
        retention.check(budget, false)?;
        Ok((entries.into_iter().next().unwrap(), retention))
    }

    /// Natural producer bound before creating an empty transcript. Includes
    /// initial Vec/Arc transfer overlap and the complete reservation guard.
    #[doc(hidden)]
    pub fn initial_storage_bytes(
        reservation: &RepositoryWorkspaceReservation,
        root_mode: u16,
    ) -> Result<usize, RepositoryWorkspaceError> {
        Ok(
            Self::initial_retention(reservation, root_mode, reservation.limits)?
                .1
                .metadata_bytes,
        )
    }

    fn new_with_budget(
        reservation: Arc<RepositoryWorkspaceReservation>,
        root_mode: u16,
        budget: RepositoryWorkspaceRetentionLimits,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let (root, retention) = Self::initial_retention(&reservation, root_mode, budget)?;
        let entries = [root];
        let initial = Arc::new(RepositoryWorkspaceState::from_admitted(
            reservation.clone(),
            RepositoryWorkspaceMode::Archive,
            entries.into(),
        ));
        Ok(Self {
            reservation,
            initial,
            records: Arc::from([]),
            retention,
        })
    }
    pub fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        &self.reservation
    }
    pub fn records(&self) -> &[Arc<RepositoryWorkspaceRecord>] {
        &self.records
    }
    pub fn initial(&self) -> &Arc<RepositoryWorkspaceState> {
        &self.initial
    }
    pub fn current(&self) -> &Arc<RepositoryWorkspaceState> {
        self.records
            .last()
            .map_or(&self.initial, |record| &record.next)
    }
    pub fn retention(&self) -> RepositoryWorkspaceRetention {
        self.retention
    }
    pub fn request(
        &self,
        operation: RepositoryWorkspaceOperation,
    ) -> Result<RepositoryWorkspaceRequest, RepositoryWorkspaceError> {
        self.request_at(self.records.len(), operation)
    }
    /// Loading reconstructs requests from the exact state at each original
    /// operation index. New equal argv at a NEW index has no replay record.
    pub fn request_at(
        &self,
        index: usize,
        operation: RepositoryWorkspaceOperation,
    ) -> Result<RepositoryWorkspaceRequest, RepositoryWorkspaceError> {
        operation.validate()?;
        let predecessor = if index == 0 {
            &self.initial
        } else {
            &self
                .records
                .get(index - 1)
                .ok_or(RepositoryWorkspaceError::WrongIndex)?
                .next
        };
        if index > self.records.len() {
            return Err(RepositoryWorkspaceError::WrongIndex);
        }
        let path = match &operation {
            RepositoryWorkspaceOperation::File { path, .. }
            | RepositoryWorkspaceOperation::Template { path, .. }
            | RepositoryWorkspaceOperation::Symlink { path, .. }
            | RepositoryWorkspaceOperation::Read(path)
            | RepositoryWorkspaceOperation::Exists(path)
            | RepositoryWorkspaceOperation::Delete(path) => Some(path.as_str()),
            _ => None,
        };
        if let Some(path) = path {
            let root_len = self
                .reservation
                .display_root
                .as_path()
                .to_str()
                .ok_or(RepositoryWorkspaceError::InvalidPath)?
                .len();
            let suffix = if path.is_empty() {
                0
            } else {
                add(path.len(), 1)?
            };
            if add(root_len, suffix)? > 4096
                || (predecessor.mode() == RepositoryWorkspaceMode::Process
                    && !path.is_empty()
                    && path.split('/').count() > 64)
            {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
        }
        if matches!(&operation, RepositoryWorkspaceOperation::Execute(_)) {
            let RepositoryWorkspaceOperation::Execute(execute) = &operation else {
                unreachable!()
            };
            execute.read_views().validate_for(&self.reservation)?;
            let cwd = execute.working_directory();
            if cwd.namespace()
                != slug_workspace_v2::PathObservationNamespace::Materialization(
                    self.reservation.instance,
                )
                || !cwd
                    .path()
                    .as_path()
                    .starts_with(self.reservation.display_root.as_path())
            {
                return Err(RepositoryWorkspaceError::InvalidOperation);
            }
            let relative = cwd
                .path()
                .as_path()
                .strip_prefix(self.reservation.display_root.as_path())
                .map_err(|_| RepositoryWorkspaceError::InvalidOperation)?;
            if cwd.path_str().len() > 4096 || relative.components().count() > 64 {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
            // First capability admits writable repository cwd/root descendants
            // only. Foreign/Host cwd needs a separately tracked read-only binding
            // contract; Host Path/Label argv remains ordinary request data.
            if self.records[..index]
                .iter()
                .filter(|record| {
                    matches!(
                        &record.request.operation,
                        RepositoryWorkspaceOperation::Execute(_)
                    )
                })
                .count()
                >= REPOSITORY_WORKSPACE_EXECUTES
            {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
            RepositoryWorkspaceState::validate_base(
                &self.reservation,
                RepositoryWorkspaceMode::Process,
                predecessor.entries(),
            )?;
            state_sources::BindingSelection::for_state(predecessor)?;
            // Check ALL retained history, not just the latest body lengths.
            self.retention.check(self.reservation.limits, true)?;
        }
        let request = RepositoryWorkspaceRequest {
            reservation: self.reservation.clone(),
            index,
            predecessor: predecessor.clone(),
            operation,
        };
        request.validate_symlink()?;
        Ok(request)
    }
    pub fn replay(
        &self,
        request: &RepositoryWorkspaceRequest,
    ) -> Result<Option<&RepositoryWorkspaceRecord>, RepositoryWorkspaceError> {
        if request.reservation != self.reservation {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        let predecessor = if request.index == 0 {
            &self.initial
        } else {
            &self
                .records
                .get(request.index - 1)
                .ok_or(RepositoryWorkspaceError::WrongIndex)?
                .next
        };
        if &request.predecessor != predecessor {
            return Err(RepositoryWorkspaceError::ForeignPredecessor);
        }
        match self.records.get(request.index) {
            Some(record) if &record.request == request => Ok(Some(record.as_ref())),
            Some(_) => Err(RepositoryWorkspaceError::ReplayConflict),
            None if request.index == self.records.len() => Ok(None),
            None => Err(RepositoryWorkspaceError::WrongIndex),
        }
    }
    fn visit_bodies(&self, visit: &mut impl FnMut(&Arc<[u8]>)) {
        let mut state_bodies = |state: &RepositoryWorkspaceState| {
            for entry in state.entries() {
                if let RepositoryWorkspaceNode::File(body) = entry.node() {
                    visit(body);
                }
            }
        };
        state_bodies(&self.initial);
        for record in self.records.iter() {
            state_bodies(&record.next);
        }
        for record in self.records.iter() {
            record.request.operation.bodies(visit);
            record.result.bodies(visit);
        }
    }
    fn completion_metadata(
        &self,
        request: &RepositoryWorkspaceRequest,
        result: &RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
        budget: RepositoryWorkspaceRetentionLimits,
        process_body_limit: u64,
    ) -> Result<(RepositoryWorkspaceRetention, usize, usize), RepositoryWorkspaceError> {
        if self.replay(request)?.is_some() {
            return Err(RepositoryWorkspaceError::WrongIndex);
        }
        let execute = matches!(&request.operation, RepositoryWorkspaceOperation::Execute(_));
        if mode
            != if execute {
                RepositoryWorkspaceMode::Process
            } else {
                self.current().mode()
            }
        {
            return Err(RepositoryWorkspaceError::InvalidMode);
        }
        let state_metadata = add(
            RepositoryWorkspaceState::validate_base(&self.reservation, mode, entries)?,
            state_sources::BindingSelection::for_request(request, entries)?.owned_bytes()?,
        )?;
        validate_result(request, result, entries)?;
        let mut refs = 0usize;
        self.visit_bodies(&mut |_| {
            refs = refs.saturating_add(1);
        });
        request.operation.bodies(&mut |_| {
            refs = refs.saturating_add(1);
        });
        result.bodies(&mut |_| {
            refs = refs.saturating_add(1);
        });
        refs = add(
            refs,
            entries
                .iter()
                .filter(|e| matches!(e.node(), RepositoryWorkspaceNode::File(_)))
                .count(),
        )?;
        // Pinned SmallMap/SmallSet use separate entry/hash storage and optional
        // hashbrown indices. Eight times entry+hash layout plus1KiB covers both
        // maps, index/control overhead and rounding; actual borrowed storage is
        // checked below. Credits are conservative component bounds, not RSS.
        let scratch = add(
            refs.checked_mul(
                8 * (std::mem::size_of::<(Arc<[u8]>, ())>()
                    + std::mem::size_of::<(usize, usize)>()
                    + 16),
            )
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
            1024,
        )?;
        let mut retention = self.retention;
        retention.state_versions = add(retention.state_versions, 1)?;
        if matches!(&request.operation, RepositoryWorkspaceOperation::Execute(_)) {
            retention.executes = add(retention.executes, 1)?;
        }
        // New Vec and Arc slice coexist during transfer. Old records are Arc
        // handles: no deep CompactString/SmallMap clone occurs on append.
        let table = array_bytes::<Arc<RepositoryWorkspaceRecord>>(add(self.records.len(), 1)?)?;
        let record_metadata = add(
            add(table, table)?,
            add(
                array_bytes::<RepositoryWorkspaceRecord>(1)?,
                add(request.operation.metadata()?, result.metadata()?)?,
            )?,
        )?;
        retention.metadata_bytes = add(
            retention.metadata_bytes,
            add(scratch, add(state_metadata, record_metadata)?)?,
        )?;
        retention.check(budget, mode == RepositoryWorkspaceMode::Process)?;
        if mode == RepositoryWorkspaceMode::Process && retention.body_bytes > process_body_limit {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        Ok((retention, refs, scratch))
    }

    /// Conservative complete native lifetime guard, including the producer's
    /// retained construction scratch. Projected source allocations are reported
    /// separately; the borrowed walk reads lengths and never copies bodies.
    #[doc(hidden)]
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let owned = add(
            self.retention.metadata_bytes,
            usize::try_from(self.retention.body_bytes)
                .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        RepositoryWorkspaceStoragePartition::new(
            owned,
            read_views::source_storage_bytes(|visit| {
                self.visit_read_views(&mut |view| visit(view))
            })?,
        )
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.storage_partition()?.total_bytes()
    }
    pub fn visit_read_views(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        self.initial.visit_read_views(visit);
        for record in self.records.iter() {
            record.request.visit_read_views(visit);
            record.next.visit_read_views(visit);
        }
    }

    /// Bound a complete successor and its construction scratch before native
    /// admission. The old transcript and incoming buffers need their own live
    /// guards throughout completion. New body occurrences pay full Arc layout;
    /// actual canonicalization may reuse storage and retain less. This forecast
    /// does not substitute for completion's semantic or unique-body admission.
    #[doc(hidden)]
    pub fn completion_storage_partition(
        &self,
        request: &RepositoryWorkspaceRequest,
        result: &RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let (retention, _, _) = self.completion_metadata(
            request,
            result,
            mode,
            entries,
            self.reservation.limits,
            REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES,
        )?;
        let mut bytes = add(
            retention.metadata_bytes,
            usize::try_from(retention.body_bytes)
                .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        let mut charge = |body: &Arc<[u8]>| {
            bytes = add(bytes, array_bytes::<u8>(body.len())?)?;
            Ok(())
        };
        // bodies() is intentionally allocation-free; retain the first checked
        // arithmetic error rather than allocating an auxiliary body set.
        let mut failure = None;
        let mut visit = |body: &Arc<[u8]>| {
            if failure.is_none() {
                failure = charge(body).err();
            }
        };
        request.operation.bodies(&mut visit);
        result.bodies(&mut visit);
        for entry in entries {
            if let RepositoryWorkspaceNode::File(body) = entry.node() {
                visit(body);
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let next_sources = state_sources::BindingSelection::for_request(request, entries)?;
        let sources = read_views::source_storage_bytes(|visit| {
            self.visit_read_views(&mut |view| visit(view));
            request.visit_read_views(&mut |view| visit(view));
            next_sources.visit(&mut |view| visit(view));
        })?;
        RepositoryWorkspaceStoragePartition::new(bytes, sources)
    }
    pub fn completion_storage_bytes(
        &self,
        request: &RepositoryWorkspaceRequest,
        result: &RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<usize, RepositoryWorkspaceError> {
        self.completion_storage_partition(request, result, mode, entries)?
            .total_bytes()
    }

    /// Incoming snapshot/argv/source buffers MUST already be credited by their
    /// producer. This method forecasts its own retained arrays and scratch before
    /// allocation; failed admission never publishes a partial record. Body reuse
    /// follows complete byte comparison and releases equal foreign allocations.
    pub fn complete(
        &self,
        request: RepositoryWorkspaceRequest,
        result: RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: Vec<RepositoryWorkspaceEntry>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        self.complete_with_budget(
            request,
            result,
            mode,
            entries,
            self.reservation.limits,
            REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES,
        )
    }
    fn complete_with_budget(
        &self,
        mut request: RepositoryWorkspaceRequest,
        mut result: RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        mut entries: Vec<RepositoryWorkspaceEntry>,
        budget: RepositoryWorkspaceRetentionLimits,
        process_body_limit: u64,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if self.replay(&request)?.is_some() {
            return Err(RepositoryWorkspaceError::WrongIndex);
        }
        // Equality authenticates facts, not physical storage ownership. Transfer
        // the already charged current owners before retaining this request;
        // equal foreign predecessor bodies/descriptor guards must be released.
        request.reservation = self.reservation.clone();
        request.predecessor = self.current().clone();
        let (mut retention, refs, scratch) = self.completion_metadata(
            &request,
            &result,
            mode,
            &entries,
            budget,
            process_body_limit,
        )?;
        let mut pool: SmallMap<Arc<[u8]>, ()> = SmallMap::with_capacity(refs);
        let mut seen: SmallSet<(usize, usize)> = SmallSet::with_capacity(refs);
        self.visit_bodies(&mut |body| {
            if seen.insert((body.as_ptr() as usize, body.len())) {
                pool.insert(body.clone(), ());
            }
        });
        let actual = add(
            pool.retained_storage_bytes_upper_bound()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
            seen.retained_storage_bytes_upper_bound()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        if actual > scratch {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        let mut canonicalize = |body: &mut Arc<[u8]>| -> Result<(), RepositoryWorkspaceError> {
            if let Some((_, shared, _)) = pool.get_full(body.as_ref()) {
                *body = shared.clone();
            } else {
                retention.body_bytes = retention
                    .body_bytes
                    .checked_add(body.len() as u64)
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
                let owner = array_bytes::<u8>(body.len())?
                    .checked_sub(body.len())
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
                retention.metadata_bytes = add(retention.metadata_bytes, owner)?;
                retention.check(budget, mode == RepositoryWorkspaceMode::Process)?;
                if mode == RepositoryWorkspaceMode::Process
                    && retention.body_bytes > process_body_limit
                {
                    return Err(RepositoryWorkspaceError::RetentionLimit);
                }
                pool.insert(body.clone(), ());
            }
            Ok(())
        };
        request.operation.bodies_mut(&mut canonicalize)?;
        if let Some(body) = result.body_mut() {
            canonicalize(body)?;
        }
        for entry in &mut entries {
            if let Some(body) = entry.body_mut() {
                canonicalize(body)?;
            }
        }
        let sources = state_sources::BindingSelection::for_request(&request, &entries)?.into_vec();
        let next = Arc::new(RepositoryWorkspaceState::from_admitted_with_sources(
            self.reservation.clone(),
            mode,
            entries,
            sources,
        ));
        let mut records = Vec::with_capacity(self.records.len() + 1);
        records.extend(self.records.iter().cloned());
        records.push(Arc::new(RepositoryWorkspaceRecord {
            request,
            result,
            next,
        }));
        Ok(Self {
            reservation: self.reservation.clone(),
            initial: self.initial.clone(),
            records: records.into(),
            retention,
        })
    }
    /// Complete retained realization plus fixed digest construction scratch.
    /// Native producers reserve this before realization clones or digest output.
    pub fn realization_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        self.storage_partition()?
            .add_owned(std::mem::size_of::<RepositoryWorkspaceRealization>() + 256)
    }
    pub fn realization_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.realization_storage_partition()?.total_bytes()
    }
    pub fn realize(
        &self,
        consumed: usize,
    ) -> Result<RepositoryWorkspaceRealization, RepositoryWorkspaceError> {
        if consumed != self.records.len() {
            return Err(RepositoryWorkspaceError::UnusedRecords);
        }
        let source_identity =
            crate::source_preparation::workspace_identity::derive_workspace_source_identity(
                self.current(),
            );
        Ok(RepositoryWorkspaceRealization {
            transcript: self.clone(),
            source_identity,
        })
    }
}

fn validate_result(
    request: &RepositoryWorkspaceRequest,
    result: &RepositoryWorkspaceResult,
    next: &[RepositoryWorkspaceEntry],
) -> Result<(), RepositoryWorkspaceError> {
    let previous = request.predecessor.entries();
    let lookup = |path: &str| {
        next.binary_search_by(|entry| entry.path().cmp(path))
            .ok()
            .map(|i| &next[i])
    };
    match (&request.operation, result) {
        (RepositoryWorkspaceOperation::Execute(_), RepositoryWorkspaceResult::Execute(_)) => {
            result.metadata()?;
        }
        (
            RepositoryWorkspaceOperation::Archive(archive),
            RepositoryWorkspaceResult::Archive(digest),
        ) if archive
            .expected_sha256()
            .is_none_or(|expected| expected == *digest) => {}
        (RepositoryWorkspaceOperation::Read(path), RepositoryWorkspaceResult::Read(body)) => {
            if next != previous
                || request
                    .predecessor
                    .read_expectation(path)?
                    .validate(body)
                    .is_err()
            {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
        }
        (RepositoryWorkspaceOperation::Exists(path), RepositoryWorkspaceResult::Exists(exists)) => {
            if next != previous || *exists != request.predecessor.exists(path)? {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
        }
        (
            RepositoryWorkspaceOperation::Delete(path),
            RepositoryWorkspaceResult::Delete(removed),
        ) => {
            if *removed != request.predecessor.entry(path).is_some() {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
            let mut kept = previous.iter().filter(|entry| {
                !(entry.path() == path
                    || entry
                        .path()
                        .strip_prefix(path.as_str())
                        .is_some_and(|tail| tail.starts_with('/')))
            });
            for entry in next {
                if kept.next() != Some(entry) {
                    return Err(RepositoryWorkspaceError::InvalidResult);
                }
            }
            if kept.next().is_some() {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
        }
        (
            RepositoryWorkspaceOperation::Symlink { path, target, .. },
            RepositoryWorkspaceResult::Unit,
        ) => {
            if !matches!(lookup(path), Some(entry) if entry.mode()==0o777 && matches!(entry.node(),RepositoryWorkspaceNode::Link(actual) if Some(actual.as_str())==target.as_path().to_str()))
            {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
            for entry in previous {
                if entry.path() != path && lookup(entry.path()) != Some(entry) {
                    return Err(RepositoryWorkspaceError::InvalidResult);
                }
            }
            for entry in next {
                if entry.path() != path
                    && request.predecessor.entry(entry.path()).is_none()
                    && !(matches!(entry.node(), RepositoryWorkspaceNode::Directory)
                        && path
                            .strip_prefix(entry.path())
                            .is_some_and(|tail| tail.starts_with('/')))
                {
                    return Err(RepositoryWorkspaceError::InvalidResult);
                }
            }
        }
        (
            RepositoryWorkspaceOperation::File { path, body, mode }
            | RepositoryWorkspaceOperation::Template {
                path, body, mode, ..
            },
            RepositoryWorkspaceResult::Unit,
        ) => {
            if !matches!(lookup(path), Some(entry) if entry.mode() == *mode && matches!(entry.node(), RepositoryWorkspaceNode::File(actual) if actual == body))
            {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
            for entry in previous {
                if entry.path() != path && lookup(entry.path()) != Some(entry) {
                    return Err(RepositoryWorkspaceError::InvalidResult);
                }
            }
            for entry in next {
                if entry.path() != path
                    && request.predecessor.entry(entry.path()).is_none()
                    && !(matches!(entry.node(), RepositoryWorkspaceNode::Directory)
                        && path
                            .strip_prefix(entry.path())
                            .is_some_and(|tail| tail.starts_with('/')))
                {
                    return Err(RepositoryWorkspaceError::InvalidResult);
                }
            }
        }
        _ => return Err(RepositoryWorkspaceError::InvalidResult),
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceRealization {
    transcript: RepositoryWorkspaceTranscript,
    source_identity: Arc<str>,
}
impl RepositoryWorkspaceRealization {
    /// Exact comparison whose every state edge uses the caller's bounded
    /// comparator. No digest substitutes for structural equality.
    #[doc(hidden)]
    pub fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            transcript,
            source_identity,
        } = self;
        let Self {
            transcript: other_transcript,
            source_identity: other_identity,
        } = other;
        if source_identity != other_identity {
            return Ok(false);
        }
        transcript.try_eq_with_states(other_transcript, compare)
    }

    pub fn source_identity(&self) -> &Arc<str> {
        &self.source_identity
    }
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        self.transcript.storage_partition()?.add_owned(add(
            std::mem::size_of::<Self>(),
            array_bytes::<u8>(self.source_identity.len())?,
        )?)
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.storage_partition()?.total_bytes()
    }
    pub fn visit_read_views(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        self.transcript.visit_read_views(visit)
    }

    pub fn transcript(&self) -> &RepositoryWorkspaceTranscript {
        &self.transcript
    }
    pub fn final_state(&self) -> &Arc<RepositoryWorkspaceState> {
        self.transcript.current()
    }
    pub fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        self.transcript.reservation()
    }
}

/// Epoch is command-fresh EMPTY at begin, never constructed from a previous
/// transcript. Later completed updates retain exact current reservations only.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, dupe::Dupe)]
pub struct RepositoryWorkspaceEpoch {
    active: Option<ActiveWorkspaceEpoch>,
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative, dupe::Dupe)]
struct ActiveWorkspaceEpoch {
    workspace: NormalizedAbsolutePath,
    lease: u64,
    limits: RepositoryWorkspaceRetentionLimits,
    transcripts: Arc<[RepositoryWorkspaceTranscript]>,
    overhead: usize,
}
impl RepositoryWorkspaceEpoch {
    pub fn revoked() -> Self {
        Self { active: None }
    }
    pub fn fresh(
        workspace: NormalizedAbsolutePath,
        lease: u64,
        limits: RepositoryWorkspaceRetentionLimits,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let overhead = Self::fresh_metadata(&workspace, lease, limits)?;
        Ok(Self {
            active: Some(ActiveWorkspaceEpoch {
                workspace,
                lease,
                limits,
                transcripts: Arc::from([]),
                overhead,
            }),
        })
    }
    fn fresh_metadata(
        workspace: &NormalizedAbsolutePath,
        lease: u64,
        limits: RepositoryWorkspaceRetentionLimits,
    ) -> Result<usize, RepositoryWorkspaceError> {
        if lease == 0 {
            return Err(RepositoryWorkspaceError::InvalidReservation);
        }
        let overhead = add(
            std::mem::size_of::<Self>(),
            workspace
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        if overhead > limits.metadata_bytes {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        Ok(overhead)
    }
    /// Complete conservative native guard; semantic retention counters remain
    /// independently checked by every epoch update. The empty Arc header is
    /// included even before any transcript has been reserved.
    #[doc(hidden)]
    pub fn fresh_storage_partition(
        workspace: &NormalizedAbsolutePath,
        lease: u64,
        limits: RepositoryWorkspaceRetentionLimits,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        RepositoryWorkspaceStoragePartition::new(
            add(
                Self::fresh_metadata(workspace, lease, limits)?,
                array_bytes::<RepositoryWorkspaceTranscript>(0)?,
            )?,
            0,
        )
    }
    pub fn fresh_storage_bytes(
        workspace: &NormalizedAbsolutePath,
        lease: u64,
        limits: RepositoryWorkspaceRetentionLimits,
    ) -> Result<usize, RepositoryWorkspaceError> {
        Self::fresh_storage_partition(workspace, lease, limits)?.total_bytes()
    }
    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let Some(active) = &self.active else {
            return RepositoryWorkspaceStoragePartition::new(std::mem::size_of::<Self>(), 0);
        };
        let mut storage = RepositoryWorkspaceStoragePartition::new(
            add(
                active.overhead,
                array_bytes::<RepositoryWorkspaceTranscript>(0)?,
            )?,
            0,
        )?;
        for transcript in active.transcripts.iter() {
            storage = storage.add_partition(transcript.storage_partition()?)?
        }
        Ok(storage)
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        self.storage_partition()?.total_bytes()
    }
    pub fn visit_read_views(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        if let Some(active) = &self.active {
            for transcript in active.transcripts.iter() {
                transcript.visit_read_views(visit)
            }
        }
    }
    fn reservation_budget(
        &self,
        reservation: &RepositoryWorkspaceReservation,
    ) -> Result<RepositoryWorkspaceRetentionLimits, RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        if reservation.workspace != active.workspace
            || reservation.lease != active.lease
            || reservation.limits != active.limits
            || active.transcripts.iter().any(|t| {
                t.reservation.same_slot(reservation)
                    || t.reservation.instance == reservation.instance
                    || t.reservation.display_root == reservation.display_root
            })
        {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        if active.transcripts.len() >= active.limits.max_roots {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        Ok(self.remaining_budget(None)?.0)
    }
    #[doc(hidden)]
    pub fn reserve_storage_partition(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        root_mode: u16,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let budget = self.reservation_budget(reservation)?;
        let (_, retention) =
            RepositoryWorkspaceTranscript::initial_retention(reservation, root_mode, budget)?;
        let (_, total) =
            self.projected_retention(None, retention, RepositoryWorkspaceMode::Archive)?;
        RepositoryWorkspaceStoragePartition::new(
            Self::retention_storage(total)?,
            read_views::source_storage_bytes(|visit| {
                self.visit_read_views(&mut |view| visit(view))
            })?,
        )
    }
    pub fn reserve_storage_bytes(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        root_mode: u16,
    ) -> Result<usize, RepositoryWorkspaceError> {
        self.reserve_storage_partition(reservation, root_mode)?
            .total_bytes()
    }
    fn publish_slot(
        &self,
        transcript: &RepositoryWorkspaceTranscript,
    ) -> Result<usize, RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let slot = active
            .transcripts
            .iter()
            .position(|old| old.reservation == transcript.reservation)
            .ok_or(RepositoryWorkspaceError::ForeignReservation)?;
        let old = &active.transcripts[slot];
        if transcript.records.len() != add(old.records.len(), 1)? {
            return Err(RepositoryWorkspaceError::WrongIndex);
        }
        if transcript.initial != old.initial {
            return Err(RepositoryWorkspaceError::ForeignPredecessor);
        }
        if transcript.records[..old.records.len()] != *old.records {
            return Err(RepositoryWorkspaceError::ReplayConflict);
        }
        Ok(slot)
    }
    /// Native completion may publish exactly one already validated append.
    /// Matching only the final state cannot replace retained command history.
    /// This carries immutable facts, never physical or child authority.
    #[doc(hidden)]
    pub fn publish_completed(
        &self,
        transcript: RepositoryWorkspaceTranscript,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let slot = self.publish_slot(&transcript)?;
        self.update(Some(slot), transcript)
    }
    #[doc(hidden)]
    pub fn publish_storage_partition(
        &self,
        transcript: &RepositoryWorkspaceTranscript,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let slot = self.publish_slot(transcript)?;
        let (_, total) = self.projected_retention(
            Some(slot),
            transcript.retention,
            transcript.current().mode(),
        )?;
        let sources = read_views::source_storage_bytes(|visit| {
            if let Some(active) = &self.active {
                for (index, item) in active.transcripts.iter().enumerate() {
                    if index != slot {
                        item.visit_read_views(&mut |view| visit(view))
                    }
                }
            }
            transcript.visit_read_views(&mut |view| visit(view));
        })?;
        RepositoryWorkspaceStoragePartition::new(Self::retention_storage(total)?, sources)
    }
    pub fn publish_storage_bytes(
        &self,
        transcript: &RepositoryWorkspaceTranscript,
    ) -> Result<usize, RepositoryWorkspaceError> {
        self.publish_storage_partition(transcript)?.total_bytes()
    }
    fn retention_storage(
        total: RepositoryWorkspaceRetention,
    ) -> Result<usize, RepositoryWorkspaceError> {
        add(
            add(
                total.metadata_bytes,
                array_bytes::<RepositoryWorkspaceTranscript>(0)?,
            )?,
            usize::try_from(total.body_bytes)
                .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?,
        )
    }
    pub fn lease(&self) -> Option<u64> {
        self.active.as_ref().map(|a| a.lease)
    }
    pub fn workspace(&self) -> Option<&NormalizedAbsolutePath> {
        self.active.as_ref().map(|active| &active.workspace)
    }
    pub fn limits(&self) -> Option<RepositoryWorkspaceRetentionLimits> {
        self.active.as_ref().map(|active| active.limits)
    }
    pub fn transcripts(&self) -> Option<&[RepositoryWorkspaceTranscript]> {
        self.active.as_ref().map(|a| a.transcripts.as_ref())
    }
    pub fn reserve(
        &self,
        reservation: Arc<RepositoryWorkspaceReservation>,
        root_mode: u16,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let budget = self.reservation_budget(&reservation)?;
        let transcript =
            RepositoryWorkspaceTranscript::new_with_budget(reservation, root_mode, budget)?;
        self.update(None, transcript)
    }
    /// Use this command-level entrypoint before native execution. It additionally
    /// checks process-history admission across OTHER current process roots.
    pub fn request_at(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        index: usize,
        operation: RepositoryWorkspaceOperation,
    ) -> Result<RepositoryWorkspaceRequest, RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let slot = active
            .transcripts
            .iter()
            .position(|t| t.reservation.as_ref() == reservation)
            .ok_or(RepositoryWorkspaceError::ForeignReservation)?;
        let transcript = &active.transcripts[slot];
        if index == transcript.records.len()
            && matches!(&operation, RepositoryWorkspaceOperation::Execute(_))
        {
            self.check_new_execute()?;
            let (_, process_limit) = self.remaining_budget(Some(slot))?;
            if transcript.retention.body_bytes > process_limit {
                return Err(RepositoryWorkspaceError::RetentionLimit);
            }
        }
        transcript.request_at(index, operation)
    }
    pub fn complete(
        &self,
        request: RepositoryWorkspaceRequest,
        result: RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: Vec<RepositoryWorkspaceEntry>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let index = active
            .transcripts
            .iter()
            .position(|t| t.reservation == request.reservation)
            .ok_or(RepositoryWorkspaceError::ForeignReservation)?;
        if request.index == active.transcripts[index].records.len()
            && matches!(&request.operation, RepositoryWorkspaceOperation::Execute(_))
        {
            self.check_new_execute()?;
        }
        let (budget, process_limit) = self.remaining_budget(Some(index))?;
        self.update(
            Some(index),
            active.transcripts[index].complete_with_budget(
                request,
                result,
                mode,
                entries,
                budget,
                process_limit,
            )?,
        )
    }
    fn check_new_execute(&self) -> Result<(), RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let mut executes = 0usize;
        for transcript in active.transcripts.iter() {
            executes = add(executes, transcript.retention.executes)?;
        }
        if executes >= REPOSITORY_WORKSPACE_EXECUTES {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        Ok(())
    }
    fn remaining_budget(
        &self,
        index: Option<usize>,
    ) -> Result<(RepositoryWorkspaceRetentionLimits, u64), RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let count = add(active.transcripts.len(), usize::from(index.is_none()))?;
        if count > active.limits.max_roots {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        let table = array_bytes::<RepositoryWorkspaceTranscript>(count)?;
        let overhead = add(
            active.overhead,
            add(add(table, table)?, std::mem::size_of::<Self>())?,
        )?;
        let mut budget = active.limits;
        budget.metadata_bytes = budget
            .metadata_bytes
            .checked_sub(overhead)
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
        let mut process_limit = REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES;
        for (i, item) in active.transcripts.iter().enumerate() {
            if Some(i) == index {
                continue;
            }
            budget.metadata_bytes = budget
                .metadata_bytes
                .checked_sub(item.retention.metadata_bytes)
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            budget.archive_body_bytes = budget
                .archive_body_bytes
                .checked_sub(item.retention.body_bytes)
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            if item.current().mode() == RepositoryWorkspaceMode::Process {
                process_limit = process_limit
                    .checked_sub(item.retention.body_bytes)
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            }
        }
        Ok((budget, process_limit))
    }
    fn projected_retention(
        &self,
        index: Option<usize>,
        replacement: RepositoryWorkspaceRetention,
        mode: RepositoryWorkspaceMode,
    ) -> Result<(usize, RepositoryWorkspaceRetention), RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let count = add(active.transcripts.len(), usize::from(index.is_none()))?;
        if count > active.limits.max_roots {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        let table = array_bytes::<RepositoryWorkspaceTranscript>(count)?;
        let overhead = add(
            active.overhead,
            add(add(table, table)?, std::mem::size_of::<Self>())?,
        )?;
        let mut total = RepositoryWorkspaceRetention {
            metadata_bytes: overhead,
            ..Default::default()
        };
        let mut process_body = 0u64;
        for (item, mode) in active
            .transcripts
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != index)
            .map(|(_, t)| (t.retention, t.current().mode()))
            .chain(std::iter::once((replacement, mode)))
        {
            total.state_versions = add(total.state_versions, item.state_versions)?;
            total.executes = add(total.executes, item.executes)?;
            total.metadata_bytes = add(total.metadata_bytes, item.metadata_bytes)?;
            total.body_bytes = total
                .body_bytes
                .checked_add(item.body_bytes)
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            if mode == RepositoryWorkspaceMode::Process {
                process_body = process_body
                    .checked_add(item.body_bytes)
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            }
        }
        total.check(active.limits, false)?;
        if process_body > REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
        Ok((overhead, total))
    }
    fn update(
        &self,
        index: Option<usize>,
        transcript: RepositoryWorkspaceTranscript,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let (overhead, _) =
            self.projected_retention(index, transcript.retention, transcript.current().mode())?;
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let count = add(active.transcripts.len(), usize::from(index.is_none()))?;
        let mut transcripts = Vec::with_capacity(count);
        transcripts.extend(active.transcripts.iter().cloned());
        if let Some(index) = index {
            transcripts[index] = transcript;
        } else {
            transcripts.push(transcript);
        }
        Ok(Self {
            active: Some(ActiveWorkspaceEpoch {
                workspace: active.workspace.clone(),
                lease: active.lease,
                limits: active.limits,
                transcripts: transcripts.into(),
                overhead,
            }),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryWorkspaceEpochKey {
    workspace: NormalizedAbsolutePath,
}
impl RepositoryWorkspaceEpochKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}
impl fmt::Display for RepositoryWorkspaceEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repository-workspace-epoch:{}", self.workspace)
    }
}
impl InjectedKey for RepositoryWorkspaceEpochKey {
    type Value = RepositoryWorkspaceEpoch;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositoryWorkspaceObservationKey {
    workspace: NormalizedAbsolutePath,
}
impl RepositoryWorkspaceObservationKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}
impl fmt::Display for RepositoryWorkspaceObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-repository-workspace:{}", self.workspace)
    }
}
#[async_trait]
impl Key for RepositoryWorkspaceObservationKey {
    type Value = RepositoryWorkspaceEpoch;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&RepositoryWorkspaceEpochKey::new(self.workspace.clone()))
            .await
            .expect("Core must inject a fresh empty workspace epoch before every reuse path")
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
    fn validity(value: &Self::Value) -> bool {
        value.active.is_some()
    }
}

#[cfg(test)]
mod retention_floor_tests {
    use super::*;

    #[test]
    fn intrinsic_state_floor_checks_exact_plus_one_and_overflow() {
        let minimum = RepositoryWorkspaceState::minimum_metadata_bytes().unwrap();
        let limits = RepositoryWorkspaceRetentionLimits::new(1, minimum, 1).unwrap();
        let exact = RepositoryWorkspaceRetention {
            state_versions: 1,
            metadata_bytes: minimum,
            ..Default::default()
        };
        assert_eq!(exact.check(limits, false), Ok(()));
        assert_eq!(
            RepositoryWorkspaceRetention {
                metadata_bytes: minimum + 1,
                ..exact
            }
            .check(limits, false),
            Err(RepositoryWorkspaceError::RetentionLimit)
        );
        assert_eq!(
            RepositoryWorkspaceRetention {
                metadata_bytes: minimum - 1,
                ..exact
            }
            .check(limits, false),
            Err(RepositoryWorkspaceError::RetentionLimit)
        );
        assert_eq!(
            RepositoryWorkspaceRetention {
                state_versions: 2,
                ..exact
            }
            .check(limits, false),
            Err(RepositoryWorkspaceError::RetentionLimit)
        );
        assert_eq!(
            RepositoryWorkspaceRetention {
                state_versions: usize::MAX,
                metadata_bytes: usize::MAX,
                ..exact
            }
            .check(limits, false),
            Err(RepositoryWorkspaceError::RetentionLimit)
        );
        assert!(RepositoryWorkspaceRetentionLimits::new(1, minimum - 1, 1).is_err());
    }
}
