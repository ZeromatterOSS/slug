//! Additive native control forecasts. These pure values prove neither body
//! coverage nor IO authority. Callers must separately retain exact allocation
//! owners; uncovered/foreign values still require the original full forecasts.
use super::*;

fn without_payload(
    storage: RepositoryWorkspaceStoragePartition,
    payload: usize,
) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
    RepositoryWorkspaceStoragePartition::new(
        storage
            .owned_bytes()
            .checked_sub(payload)
            .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        storage.source_state_bytes(),
    )
}

fn payload_bytes(
    mut walk: impl FnMut(
        &mut dyn FnMut(&Arc<[u8]>) -> Result<(), RepositoryWorkspaceError>,
    ) -> Result<(), RepositoryWorkspaceError>,
) -> Result<usize, RepositoryWorkspaceError> {
    let mut bytes = 0;
    walk(&mut |body| {
        bytes = add(bytes, body.len())?;
        Ok(())
    })?;
    Ok(bytes)
}

fn retained_payload(
    transcript: &RepositoryWorkspaceTranscript,
) -> Result<usize, RepositoryWorkspaceError> {
    usize::try_from(transcript.retention.body_bytes)
        .map_err(|_| RepositoryWorkspaceError::RetentionLimit)
}

impl RepositoryWorkspaceOperation {
    /// Visits only payloads charged by the canonical body ledger. Execute
    /// outputs and external read-view states remain in their original partitions.
    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::File { body, .. } => visit(body)?,
            Self::Template {
                body, source_body, ..
            } => {
                visit(body)?;
                visit(source_body)?;
            }
            Self::Symlink { .. }
            | Self::Read(_)
            | Self::Exists(_)
            | Self::Archive(_)
            | Self::Delete(_)
            | Self::Execute(_) => {}
        }
        Ok(())
    }
}

impl RepositoryWorkspaceResult {
    /// Execute stdout/stderr stay fully charged as metadata, including their
    /// payloads. They are intentionally absent from this visitor.
    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Read(body) => visit(body)?,
            Self::Unit
            | Self::Exists(_)
            | Self::Archive(_)
            | Self::Delete(_)
            | Self::Execute(_) => {}
        }
        Ok(())
    }
}

impl RepositoryWorkspaceState {
    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        for entry in self.entries() {
            if let RepositoryWorkspaceNode::File(body) = entry.node() {
                visit(body)?;
            }
        }
        Ok(())
    }

    /// Requires independent coverage of every visited actual allocation.
    /// Conservative Arc headers/padding and all source partitions stay charged.
    #[doc(hidden)]
    pub fn body_covered_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let full = self.storage_partition()?;
        let payload = payload_bytes(|visit| self.try_visit_body_allocations(&mut |b| visit(b)))?;
        without_payload(full, payload)
    }
}

impl RepositoryWorkspaceRequest {
    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.predecessor.try_visit_body_allocations(visit)?;
        self.operation.try_visit_body_allocations(visit)
    }

    /// Requires actual allocation coverage, independently of request equality.
    #[doc(hidden)]
    pub fn body_covered_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let full = self.storage_partition()?;
        let payload = payload_bytes(|visit| self.try_visit_body_allocations(&mut |b| visit(b)))?;
        without_payload(full, payload)
    }
}

impl RepositoryWorkspaceTranscript {
    /// Visits all retained canonical-body positions, including deleted history.
    /// Occurrences may repeat; this is not an allocation-identity certificate.
    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.initial.try_visit_body_allocations(visit)?;
        for record in self.records.iter() {
            record.next.try_visit_body_allocations(visit)?;
        }
        for record in self.records.iter() {
            record.request.operation.try_visit_body_allocations(visit)?;
            record.result.try_visit_body_allocations(visit)?;
        }
        Ok(())
    }

    /// Removes only the existing exact canonical history payload counter.
    /// Headers, full metadata and all historical construction forecasts remain.
    #[doc(hidden)]
    pub fn body_covered_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        without_payload(self.storage_partition()?, retained_payload(self)?)
    }

    /// The protocol forecast must also succeed before using this supplied-graph
    /// visitor. Foreign incoming request guards remain independently required.
    #[doc(hidden)]
    pub fn try_visit_completion_body_allocations<E>(
        &self,
        request: &RepositoryWorkspaceRequest,
        result: &RepositoryWorkspaceResult,
        entries: &[RepositoryWorkspaceEntry],
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.try_visit_body_allocations(visit)?;
        request.operation.try_visit_body_allocations(visit)?;
        result.try_visit_body_allocations(visit)?;
        for entry in entries {
            if let RepositoryWorkspaceNode::File(body) = entry.node() {
                visit(body)?;
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn completion_body_covered_storage_partition(
        &self,
        request: &RepositoryWorkspaceRequest,
        result: &RepositoryWorkspaceResult,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let full = self.completion_storage_partition(request, result, mode, entries)?;
        let mut payload = retained_payload(self)?;
        let mut visit = |body: &Arc<[u8]>| -> Result<(), RepositoryWorkspaceError> {
            payload = add(payload, body.len())?;
            Ok(())
        };
        request.operation.try_visit_body_allocations(&mut visit)?;
        result.try_visit_body_allocations(&mut visit)?;
        for entry in entries {
            if let RepositoryWorkspaceNode::File(body) = entry.node() {
                visit(body)?;
            }
        }
        without_payload(full, payload)
    }

    #[doc(hidden)]
    pub fn realization_body_covered_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        without_payload(
            self.realization_storage_partition()?,
            retained_payload(self)?,
        )
    }
}

impl RepositoryWorkspaceRealization {
    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.transcript.try_visit_body_allocations(visit)
    }

    #[doc(hidden)]
    pub fn body_covered_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        without_payload(
            self.storage_partition()?,
            retained_payload(&self.transcript)?,
        )
    }
}

impl RepositoryWorkspaceEpoch {
    fn body_payload_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut payload = 0;
        if let Some(active) = &self.active {
            for transcript in active.transcripts.iter() {
                payload = add(payload, retained_payload(transcript)?)?;
            }
        }
        Ok(payload)
    }

    #[doc(hidden)]
    pub fn try_visit_body_allocations<E>(
        &self,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(active) = &self.active {
            for transcript in active.transcripts.iter() {
                transcript.try_visit_body_allocations(visit)?;
            }
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn body_covered_storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        without_payload(self.storage_partition()?, self.body_payload_bytes()?)
    }

    #[doc(hidden)]
    pub fn reserve_body_covered_storage_partition(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        root_mode: u16,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        without_payload(
            self.reserve_storage_partition(reservation, root_mode)?,
            self.body_payload_bytes()?,
        )
    }

    /// Validates the same reservation forecast before visiting its unchanged
    /// history. The new genesis contributes no body allocation.
    #[doc(hidden)]
    pub fn try_visit_reserve_body_allocations<E>(
        &self,
        reservation: &RepositoryWorkspaceReservation,
        root_mode: u16,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<Result<(), RepositoryWorkspaceError>, E> {
        if let Err(error) = self.reserve_storage_partition(reservation, root_mode) {
            return Ok(Err(error));
        }
        self.try_visit_body_allocations(visit)?;
        Ok(Ok(()))
    }

    #[doc(hidden)]
    pub fn publish_body_covered_storage_partition(
        &self,
        transcript: &RepositoryWorkspaceTranscript,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let full = self.publish_storage_partition(transcript)?;
        let slot = self.publish_slot(transcript)?;
        let mut payload = retained_payload(transcript)?;
        if let Some(active) = &self.active {
            for (index, old) in active.transcripts.iter().enumerate() {
                if index != slot {
                    payload = add(payload, retained_payload(old)?)?;
                }
            }
        }
        without_payload(full, payload)
    }

    #[doc(hidden)]
    pub fn try_visit_publish_body_allocations<E>(
        &self,
        transcript: &RepositoryWorkspaceTranscript,
        visit: &mut impl FnMut(&Arc<[u8]>) -> Result<(), E>,
    ) -> Result<Result<(), RepositoryWorkspaceError>, E> {
        if let Err(error) = self.publish_storage_partition(transcript) {
            return Ok(Err(error));
        }
        let slot = match self.publish_slot(transcript) {
            Ok(slot) => slot,
            Err(error) => return Ok(Err(error)),
        };
        if let Some(active) = &self.active {
            for (index, old) in active.transcripts.iter().enumerate() {
                if index != slot {
                    old.try_visit_body_allocations(visit)?;
                }
            }
        }
        transcript.try_visit_body_allocations(visit)?;
        Ok(Ok(()))
    }
}
