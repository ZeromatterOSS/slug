use super::*;
#[derive(Debug)]
pub enum RepositoryWorkspaceBodyBuildError<E> {
    Body(Error),
    Admission(E),
}
pub struct RepositoryWorkspaceBodyBuilder {
    expected: usize,
    appended: usize,
    blocks: Vec<Block>,
    routing: StarlarkHasher,
    // Vec headers are covered by storage_bytes(sizeofSelf); their bounded heap
    // storage belongs to the separately prepaid encoding scratch allowance.
    workspace: codec::Workspace,
}
impl RepositoryWorkspaceBodyBuilder {
    pub fn storage_bytes(expected: usize) -> Result<usize, Error> {
        let count = expected.checked_add(CHUNK - 1).ok_or(Error::Storage)? / CHUNK;
        let table = table_bytes(count)?;
        add(
            add(
                size_of::<Self>(),
                table.checked_mul(2).ok_or(Error::Storage)?,
            )?,
            arc_layout(Layout::new::<PackedData>())?
                .checked_mul(2)
                .ok_or(Error::Storage)?,
        )
    }
    pub fn encoding_scratch_bytes() -> usize {
        codec::scratch_bound()
    }
    pub fn new(expected: usize) -> Result<Self, Error> {
        Self::storage_bytes(expected)?;
        let count = expected.checked_add(CHUNK - 1).ok_or(Error::Storage)? / CHUNK;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        if blocks.capacity() > count {
            return Err(Error::Storage);
        }
        let mut routing = StarlarkHasher::new();
        expected.hash(&mut routing);
        Ok(Self {
            expected,
            appended: 0,
            blocks,
            routing,
            workspace: codec::Workspace::default(),
        })
    }
    pub fn append<E>(
        &mut self,
        raw: &[u8],
        admit: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), RepositoryWorkspaceBodyBuildError<E>> {
        use RepositoryWorkspaceBodyBuildError as B;
        let remaining = self.expected - self.appended;
        if raw.is_empty() || raw.len() != remaining.min(CHUNK) {
            return Err(B::Body(Error::Length));
        }
        // Framing is checked before lazy scratch allocation. A refused append
        // may keep scratch, but every retry reruns encode/decode/full comparison.
        let encoded = self.workspace.encode_and_validate(raw).map_err(B::Body)?;
        let compressed = encoded.len() < raw.len();
        let selected = if compressed { encoded } else { raw };
        admit(arc_bytes(selected.len()).map_err(B::Body)?).map_err(B::Admission)?;
        // Table was prepaid/reserved exactly before any block allocation.
        self.blocks.push(Block {
            bytes: Arc::from(selected),
            compressed,
        });
        self.routing.write(raw);
        self.appended += raw.len();
        Ok(())
    }
    pub fn finish(self) -> Result<RepositoryWorkspaceBody, Error> {
        if self.appended != self.expected {
            return Err(Error::Length);
        }
        let mut control = add(
            arc_layout(Layout::new::<PackedData>())?,
            table_bytes(self.blocks.len())?,
        )?;
        let mut payload = 0;
        for block in &self.blocks {
            payload = add(payload, block.bytes.len())?;
            control = add(
                control,
                arc_bytes(block.bytes.len())?
                    .checked_sub(block.bytes.len())
                    .ok_or(Error::Storage)?,
            )?;
        }
        let native = add(control, payload)?;
        let retained = add(control, self.expected)?;
        // No working slice escapes into PackedData; retire both buffers before
        // constructing the immutable owner and before native scratch can retire.
        drop(self.workspace);
        let data = PackedData {
            logical: self.expected,
            routing: self.routing.finish_small(),
            control,
            native,
            retained,
            blocks: self.blocks.into_boxed_slice(),
        };
        Ok(RepositoryWorkspaceBody(Repr::Packed(Arc::new(data))))
    }
}
