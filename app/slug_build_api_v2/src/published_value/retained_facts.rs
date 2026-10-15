//! Canonical retained records, independent of the producer's inspection history.
use super::*;

#[path = "retained_facts/checked_backing.rs"]
mod checked_backing;
#[path = "retained_facts/metadata.rs"]
mod metadata;
#[path = "retained_facts/sources.rs"]
mod sources;
pub(super) use checked_backing::CertificationPhase;
pub(super) use checked_backing::checked_local_visits;
pub(super) use checked_backing::prepare_checked_draft;

/// Successful canonical local contributions, not a physical allocation partition.
#[derive(Clone, Copy, Debug)]
pub(super) struct LocalRetainedFacts {
    pub(super) total: usize,
    pub(super) source: usize,
    pub(super) native: usize,
}
impl LocalRetainedFacts {
    pub(super) fn other(self) -> usize {
        // Each subset is committed only after its disjoint successful total add.
        self.total - self.source - self.native
    }
}

struct Meter<'a> {
    work: &'a mut usize,
    bytes: &'a mut usize,
    scratch: &'a mut usize,
    failed: &'a mut bool,
    retained: usize,
    source: usize,
    native: usize,
}
impl Meter<'_> {
    fn work(&mut self) -> Result<(), PublishedValueError> {
        if *self.failed || *self.work >= NODE_LIMIT {
            *self.failed = true;
            return Err(PublishedValueError::Limit("retained certificate work"));
        }
        *self.work += 1;
        Ok(())
    }
    fn add(&mut self, count: usize) -> Result<(), PublishedValueError> {
        match self
            .retained
            .checked_add(count)
            .filter(|n| *n <= RETAINED_FACT_LIMIT)
        {
            Some(next) if !*self.failed => {
                self.retained = next;
                Ok(())
            }
            _ => {
                *self.failed = true;
                Err(PublishedValueError::Limit("retained facts"))
            }
        }
    }
    fn record(&mut self) -> Result<(), PublishedValueError> {
        self.work()?;
        self.add(1)
    }
    fn bytes(&mut self, count: usize) -> Result<(), PublishedValueError> {
        self.byte_parts(count, 0)
    }
    fn byte_parts(&mut self, retained: usize, scratch: usize) -> Result<(), PublishedValueError> {
        let next = retained
            .checked_add(scratch)
            .and_then(|count| self.bytes.checked_add(count))
            .filter(|n| *n <= BYTE_LIMIT)
            .zip(self.scratch.checked_add(scratch));
        match next {
            Some((bytes, scratch)) if !*self.failed => {
                *self.bytes = bytes;
                *self.scratch = scratch;
                Ok(())
            }
            _ => {
                *self.failed = true;
                Err(PublishedValueError::Limit("retained certificate bytes"))
            }
        }
    }
    /// Immutable array metadata supplies the FULL record count. The new
    /// constant operation is paid even for empty arrays, before reading len.
    fn count_records<T>(&mut self, records: &[T]) -> Result<(), PublishedValueError> {
        self.work()?;
        self.bytes(std::mem::size_of::<usize>())?;
        self.add(records.len())
    }
    fn native(&mut self, count: usize) -> Result<(), PublishedValueError> {
        self.work()?; // Read a private, successful complete strict-audit certificate.
        self.bytes(std::mem::size_of::<NativeCertificate>())?;
        self.add(count)?;
        self.native += count; // Bounded by the successful total <= F.
        Ok(())
    }
    fn source(&mut self, count: usize) -> Result<(), PublishedValueError> {
        self.add(count)?;
        self.source += count; // Bounded by the successful total <= F.
        Ok(())
    }
}

struct Draft<'a> {
    rows: &'a [Option<PublishedNode>],
    edges: &'a [PublishedEdge],
    imports: &'a [PublishedValueArena],
}
impl Draft<'_> {
    fn edge(&self, edge: PublishedEdge, meter: &mut Meter<'_>) -> Result<(), PublishedValueError> {
        meter.record()?;
        match edge {
            PublishedEdge::Local(id) if id.index() < self.rows.len() => Ok(()),
            PublishedEdge::Imported { arena, node } => self
                .imports
                .get(arena.index())
                .ok_or(PublishedValueError::Invalid("retained import"))?
                .node(node)
                .map(|_| ()),
            _ => Err(PublishedValueError::Invalid("retained edge")),
        }
    }
    fn range(&self, range: PublishedEdgeRange) -> Result<(), PublishedValueError> {
        let end = (range.start as usize)
            .checked_add(range.len as usize)
            .ok_or(PublishedValueError::Invalid("retained range"))?;
        if end > self.edges.len() {
            return Err(PublishedValueError::Invalid("retained range"));
        }
        Ok(())
    }
    fn optional_edge(
        &self,
        edge: Option<PublishedEdge>,
        meter: &mut Meter<'_>,
    ) -> Result<(), PublishedValueError> {
        if let Some(edge) = edge {
            self.edge(edge, meter)?;
        }
        Ok(())
    }
}

/// The only source-table insertion paths are local code set_node and prior-arena
/// closure audit. Both fields stay private. Failed owner/import walks poison
/// sealing; successful walks retain that prior DAG. Thus dropping an imported-
/// only convenience guard cannot discard its last owner. No second DFS is needed.
#[allow(clippy::too_many_arguments)]
pub(super) fn local_visits(
    rows: &[Option<PublishedNode>],
    edges: &[PublishedEdge],
    slots: &[Option<PublishedRootSlot>],
    imports: &[PublishedValueArena],
    owner_count: usize,
    root_count: usize,
    work: &mut usize,
    bytes: &mut usize,
    scratch: &mut usize,
    failed: &mut bool,
) -> Result<(LocalRetainedFacts, Vec<CallableSourceAuthority>), PublishedValueError> {
    let mut phase = CertificationPhase::SourceSetup;
    local_visits_with::<checked_backing::Raw>(
        Draft {
            rows,
            edges,
            imports,
        },
        slots,
        owner_count,
        root_count,
        work,
        bytes,
        scratch,
        failed,
        &mut phase,
    )
}

#[allow(clippy::too_many_arguments)]
fn local_visits_with<B: checked_backing::Backing>(
    draft: Draft<'_>,
    slots: &[Option<PublishedRootSlot>],
    owner_count: usize,
    root_count: usize,
    work: &mut usize,
    bytes: &mut usize,
    scratch: &mut usize,
    failed: &mut bool,
    phase: &mut CertificationPhase,
) -> Result<(LocalRetainedFacts, Vec<CallableSourceAuthority>), PublishedValueError> {
    let mut meter = Meter {
        work,
        bytes,
        scratch,
        failed,
        retained: 0,
        source: 0,
        native: 0,
    };
    let result = (|| {
        let Draft {
            rows,
            edges: _,
            imports,
        } = draft;
        *phase = CertificationPhase::SourceSetup;
        let mut sources = sources::LocalSources::new(rows.len(), &mut meter)?;
        *phase = CertificationPhase::Rows;
        for row in rows {
            meter.record()?;
            let row = row
                .as_ref()
                .ok_or(PublishedValueError::Invalid("incomplete row"))?;
            metadata::row(row, &draft, &mut sources, &mut meter)?;
        }
        // Both paths retain the FULL backing, including unused entries.
        *phase = CertificationPhase::BackingEdges;
        B::count(&draft, &mut meter)?;
        *phase = CertificationPhase::RootSlots;
        if slots.len() != rows.len() {
            return Err(PublishedValueError::Invalid("retained root slots"));
        }
        B::slots(slots, root_count, &mut meter)?;
        // These immutable length fields certify stored reference counts; there
        // is no per-reference inspection in these former counting-only loops.
        // Payloads remain covered by the guarded owner/arena walk and source
        // certificates above. Keep their FULL canonical retained counts.
        *phase = CertificationPhase::StoredReferences;
        let references = imports
            .len()
            .checked_add(owner_count)
            .and_then(|n| n.checked_add(root_count))
            .ok_or(PublishedValueError::Limit("retained reference count"))?;
        if references != 0 {
            meter.work()?;
            meter.bytes(3 * std::mem::size_of::<usize>())?;
            meter.add(references)?;
        }
        *phase = CertificationPhase::SourceCount;
        let source_count = sources.values().len();
        if source_count != 0 {
            meter.work()?;
            meter.bytes(std::mem::size_of::<usize>())?;
            meter.add(source_count)?;
        }
        Ok((
            LocalRetainedFacts {
                total: meter.retained,
                source: meter.source,
                native: meter.native,
            },
            sources.finish(),
        ))
    })();
    if result.is_err() {
        *meter.failed = true;
    }
    result
}

#[cfg(test)]
#[path = "retained_facts/tests.rs"]
pub(super) mod tests;
