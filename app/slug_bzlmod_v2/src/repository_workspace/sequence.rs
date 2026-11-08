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
    metadata_bytes: usize,
    body_bytes: u64,
}
impl RepositoryWorkspaceRetention {
    pub fn state_versions(self) -> usize {
        self.state_versions
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
        if self.state_versions > limits.max_state_versions
            || self.metadata_bytes > limits.metadata_bytes
            || self.body_bytes > limits.archive_body_bytes
            || (process && self.body_bytes > REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES)
        {
            return Err(RepositoryWorkspaceError::RetentionLimit);
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
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut bytes = add(
            std::mem::size_of::<Self>(),
            add(
                self.reservation.retained_storage_bytes()?,
                add(
                    self.predecessor.retained_storage_bytes()?,
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
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceRecord {
    request: RepositoryWorkspaceRequest,
    result: RepositoryWorkspaceResult,
    next: Arc<RepositoryWorkspaceState>,
}
impl RepositoryWorkspaceRecord {
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
                >= 128
            {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
            RepositoryWorkspaceState::validate(
                &self.reservation,
                RepositoryWorkspaceMode::Process,
                predecessor.entries(),
            )?;
            // Check ALL retained history, not just the latest body lengths.
            self.retention.check(self.reservation.limits, true)?;
        }
        Ok(RepositoryWorkspaceRequest {
            reservation: self.reservation.clone(),
            index,
            predecessor: predecessor.clone(),
            operation,
        })
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
        let state_metadata = RepositoryWorkspaceState::validate(&self.reservation, mode, entries)?;
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
    /// retained construction scratch. No allocation or body traversal is needed.
    #[doc(hidden)]
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        add(
            self.retention.metadata_bytes,
            usize::try_from(self.retention.body_bytes)
                .map_err(|_| RepositoryWorkspaceError::RetentionLimit)?,
        )
    }

    /// Bound a complete successor and its construction scratch before native
    /// admission. The old transcript and incoming buffers need their own live
    /// guards throughout completion. New body occurrences pay full Arc layout;
    /// actual canonicalization may reuse storage and retain less. This forecast
    /// does not substitute for completion's semantic or unique-body admission.
    #[doc(hidden)]
    pub fn completion_storage_bytes(
        &self,
        request: &RepositoryWorkspaceRequest,
        result: &RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<usize, RepositoryWorkspaceError> {
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
        Ok(bytes)
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
        let next = Arc::new(RepositoryWorkspaceState::from_admitted(
            self.reservation.clone(),
            mode,
            entries,
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
    pub fn realize(
        &self,
        consumed: usize,
    ) -> Result<RepositoryWorkspaceRealization, RepositoryWorkspaceError> {
        if consumed != self.records.len() {
            return Err(RepositoryWorkspaceError::UnusedRecords);
        }
        Ok(RepositoryWorkspaceRealization {
            transcript: self.clone(),
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
        ) if archive.sha256() == *digest => {}
        (RepositoryWorkspaceOperation::Read(path), RepositoryWorkspaceResult::Read(body)) => {
            if next != previous
                || !matches!(request.predecessor.entry(path).map(|e| e.node()), Some(RepositoryWorkspaceNode::File(actual)) if actual == body)
            {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
        }
        (RepositoryWorkspaceOperation::Exists(path), RepositoryWorkspaceResult::Exists(exists)) => {
            if next != previous || *exists != request.predecessor.entry(path).is_some() {
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
}
impl RepositoryWorkspaceRealization {
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
    pub fn lease(&self) -> Option<u64> {
        self.active.as_ref().map(|a| a.lease)
    }
    pub fn transcripts(&self) -> Option<&[RepositoryWorkspaceTranscript]> {
        self.active.as_ref().map(|a| a.transcripts.as_ref())
    }
    pub fn reserve(
        &self,
        reservation: Arc<RepositoryWorkspaceReservation>,
        root_mode: u16,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        if reservation.workspace != active.workspace
            || reservation.lease != active.lease
            || reservation.limits != active.limits
        {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        if active.transcripts.iter().any(|t| {
            t.reservation.same_slot(&reservation)
                || t.reservation.instance == reservation.instance
                || t.reservation.display_root == reservation.display_root
        }) {
            return Err(RepositoryWorkspaceError::ForeignReservation);
        }
        let (budget, _) = self.remaining_budget(None)?;
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
    fn remaining_budget(
        &self,
        index: Option<usize>,
    ) -> Result<(RepositoryWorkspaceRetentionLimits, u64), RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let count = add(active.transcripts.len(), usize::from(index.is_none()))?;
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
            budget.max_state_versions = budget
                .max_state_versions
                .checked_sub(item.retention.state_versions)
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
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
    fn update(
        &self,
        index: Option<usize>,
        transcript: RepositoryWorkspaceTranscript,
    ) -> Result<Self, RepositoryWorkspaceError> {
        let active = self
            .active
            .as_ref()
            .ok_or(RepositoryWorkspaceError::Revoked)?;
        let count = add(active.transcripts.len(), usize::from(index.is_none()))?;
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
        for item in active
            .transcripts
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != index)
            .map(|(_, t)| t)
            .chain(std::iter::once(&transcript))
        {
            total.state_versions = add(total.state_versions, item.retention.state_versions)?;
            total.metadata_bytes = add(total.metadata_bytes, item.retention.metadata_bytes)?;
            total.body_bytes = total
                .body_bytes
                .checked_add(item.retention.body_bytes)
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            if item.current().mode() == RepositoryWorkspaceMode::Process {
                process_body = process_body
                    .checked_add(item.retention.body_bytes)
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?;
            }
        }
        total.check(active.limits, false)?;
        if process_body > REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES {
            return Err(RepositoryWorkspaceError::RetentionLimit);
        }
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
