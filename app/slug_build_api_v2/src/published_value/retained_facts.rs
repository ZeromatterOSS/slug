//! Canonical retained records, independent of the producer's inspection history.
use super::*;

#[path = "retained_facts/metadata.rs"]
mod metadata;
#[path = "retained_facts/sources.rs"]
mod sources;

struct Meter<'a> {
    work: &'a mut usize,
    bytes: &'a mut usize,
    scratch: &'a mut usize,
    failed: &'a mut bool,
    retained: usize,
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
    fn native(&mut self, count: usize) -> Result<(), PublishedValueError> {
        self.work()?; // Read a private, successful complete strict-audit certificate.
        self.bytes(std::mem::size_of::<NativeCertificate>())?;
        self.add(count)
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
) -> Result<(usize, Vec<CallableSourceAuthority>), PublishedValueError> {
    let mut meter = Meter {
        work,
        bytes,
        scratch,
        failed,
        retained: 0,
    };
    let result = (|| {
        let draft = Draft {
            rows,
            edges,
            imports,
        };
        let mut sources = sources::LocalSources::new(rows.len(), &mut meter)?;
        for row in rows {
            meter.record()?;
            let row = row
                .as_ref()
                .ok_or(PublishedValueError::Invalid("incomplete row"))?;
            metadata::row(row, &draft, &mut sources, &mut meter)?;
        }
        // Count the entire backing, including unused edges and None root slots.
        for edge in edges {
            draft.edge(*edge, &mut meter)?;
        }
        if slots.len() != rows.len() {
            return Err(PublishedValueError::Invalid("retained root slots"));
        }
        for slot in slots {
            meter.record()?;
            if slot.is_some_and(|slot| slot.index() >= root_count) {
                return Err(PublishedValueError::Invalid("original slot"));
            }
        }
        // These immutable length fields certify stored reference counts; there
        // is no per-reference inspection in these former counting-only loops.
        // Payloads remain covered by the guarded owner/arena walk and source
        // certificates above. Keep their FULL canonical retained counts.
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
        let source_count = sources.values().len();
        if source_count != 0 {
            meter.work()?;
            meter.bytes(std::mem::size_of::<usize>())?;
            meter.add(source_count)?;
        }
        Ok((meter.retained, sources.finish()))
    })();
    if result.is_err() {
        *meter.failed = true;
    }
    result
}

#[cfg(test)]
#[path = "retained_facts/tests.rs"]
mod tests;
