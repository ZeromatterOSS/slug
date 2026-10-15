use super::*;
impl Bank<'_> {
    pub(in crate::source_table) fn prepay_finish(
        &self,
        intrinsic: PublicationShape,
        work: &mut Budget,
    ) -> Result<(), String> {
        work.charge(4, std::mem::size_of::<PublicationShape>())?;
        let records = self
            .used
            // Two extra intrinsic scalar facts and four two-scalar transfers
            // per COPY cache: completion/clone, Option, flatten and final Arc.
            .checked_mul(22)
            .and_then(|n| {
                self.entries
                    .len()
                    .checked_mul(2)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| self.rows.checked_mul(4).and_then(|m| n.checked_add(m)))
            .and_then(|n| n.checked_add(8))
            .ok_or("mapping transfer work")?;
        work.charge(records, 0)?;
        if self.uses.len() != self.rows {
            return Err("mapping row completion".into());
        }
        let mut total = intrinsic;
        for storage in [self.storage()?, declaration_storage(self.rows)?] {
            total.nodes = total
                .nodes
                .checked_add(storage.nodes)
                .ok_or("mapping storage nodes")?;
            total.edges = total
                .edges
                .checked_add(storage.edges)
                .ok_or("mapping storage edges")?;
            total.bytes = total
                .bytes
                .checked_add(storage.bytes)
                .ok_or("mapping storage bytes")?;
            total.visits = total
                .visits
                .checked_add(storage.visits)
                .ok_or("mapping storage facts")?;
        }
        for (position, (_, used)) in self.entries.iter().enumerate() {
            if !used {
                continue;
            }
            let child = self.shape_at(position)?;
            total.nodes = total
                .nodes
                .checked_add(child.nodes)
                .ok_or("mapping union nodes")?;
            total.edges = total
                .edges
                .checked_add(child.edges)
                .ok_or("mapping union edges")?;
            total.bytes = total
                .bytes
                .checked_add(child.bytes)
                .ok_or("mapping union bytes")?;
            total.visits = total
                .visits
                .checked_add(child.visits)
                .ok_or("mapping union facts")?;
        }
        use slug_build_api_v2::published_value::BYTE_LIMIT;
        use slug_build_api_v2::published_value::EDGE_LIMIT;
        use slug_build_api_v2::published_value::NODE_LIMIT;
        if total.nodes > NODE_LIMIT
            || total.edges > EDGE_LIMIT
            || total.bytes > BYTE_LIMIT
            || total.visits > NODE_LIMIT
        {
            return Err("source mapping complete union limit".into());
        }
        Ok(())
    }
    pub(in crate::source_table) fn finish(
        self,
    ) -> Result<(Arc<[MappingPair]>, Vec<SharedPublicationInput>), String> {
        let completed = self
            .entries
            .into_iter()
            .map(|(entry, used)| {
                if !used {
                    return Ok(None);
                }
                match entry {
                    Entry::Inherited(pair) => Ok(Some(pair.clone())),
                    Entry::New { raw, prepared } => prepared.finish(raw).map(Some),
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        let children = self
            .uses
            .into_iter()
            .map(|p| completed[p].as_ref().expect("used mapping").input().clone())
            .collect::<Vec<_>>();
        let pairs = completed.into_iter().flatten().collect::<Vec<_>>();
        Ok((pairs.into(), children))
    }
}
