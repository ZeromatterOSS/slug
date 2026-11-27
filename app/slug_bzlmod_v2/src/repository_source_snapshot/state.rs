use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum RepositorySourceSnapshotNode {
    Directory,
    File(FileContentDigest),
    Link(CompactString),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RepositorySourceSnapshotEntry {
    path: CompactString,
    mode: u16,
    node: RepositorySourceSnapshotNode,
}
fn relative(path: &str) -> bool {
    path.is_empty()
        || (path.len() <= REPOSITORY_SOURCE_SNAPSHOT_PATH
            && !path.contains(['\0', '\\'])
            && path
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".." && p.len() <= 255)
            && path.split('/').count() <= REPOSITORY_SOURCE_SNAPSHOT_DEPTH)
}
impl RepositorySourceSnapshotEntry {
    pub fn new(
        path: CompactString,
        mode: u16,
        node: RepositorySourceSnapshotNode,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if !relative(&path) || mode & !0o7777 != 0 {
            return Err(RepositoryWorkspaceError::InvalidPath);
        }
        Ok(Self { path, mode, node })
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn mode(&self) -> u16 {
        self.mode
    }
    pub fn node(&self) -> &RepositorySourceSnapshotNode {
        &self.node
    }

    /// Replace the discovered node with the captured contents while keeping the
    /// validated path and mode. State construction still validates tree rules.
    #[doc(hidden)]
    pub fn replace_node(&mut self, node: RepositorySourceSnapshotNode) {
        self.node = node;
    }

    fn metadata_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let link = match &self.node {
            RepositorySourceSnapshotNode::Link(target) => target.capacity(),
            _ => 0,
        };
        add(self.path.capacity(), link)
    }
}

/// A complete captured source image. File sizes constrain physical admission;
/// only digest/tree metadata is retained in this pure allocation.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositorySourceSnapshotState {
    reservation: Arc<RepositorySourceSnapshotReservation>,
    entries: Arc<[RepositorySourceSnapshotEntry]>,
}
impl RepositorySourceSnapshotState {
    pub fn new(
        reservation: Arc<RepositorySourceSnapshotReservation>,
        entries: Vec<RepositorySourceSnapshotEntry>,
    ) -> Result<Self, RepositoryWorkspaceError> {
        Self::retained_storage_bytes_for(&reservation, &entries)?;
        Ok(Self {
            reservation,
            entries: entries.into(),
        })
    }
    pub fn reservation(&self) -> &Arc<RepositorySourceSnapshotReservation> {
        &self.reservation
    }
    pub fn entries(&self) -> &[RepositorySourceSnapshotEntry] {
        &self.entries
    }
    pub fn entry(&self, path: &str) -> Option<&RepositorySourceSnapshotEntry> {
        self.entries
            .binary_search_by(|e| e.path().cmp(path))
            .ok()
            .map(|i| &self.entries[i])
    }
    pub fn file_bytes(&self) -> u64 {
        self.entries
            .iter()
            .map(|e| match e.node {
                RepositorySourceSnapshotNode::File(d) => d.size_bytes(),
                _ => 0,
            })
            .sum()
    }
    /// Borrowed preflight before allocating the immutable result. Input Vec
    /// capacity and conversion peak must independently remain prepaid by caller.
    pub fn retained_storage_bytes_for(
        reservation: &RepositorySourceSnapshotReservation,
        entries: &[RepositorySourceSnapshotEntry],
    ) -> Result<usize, RepositoryWorkspaceError> {
        let tree = Self::validate_entries(reservation, entries)?;
        add(tree, reservation.retained_storage_bytes()?)
    }
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        add(
            self.tree_storage_bytes()?,
            self.reservation.retained_storage_bytes()?,
        )
    }
    /// The entire tree allocation, excluding its independently covered source
    /// request/reservation. No file payload allocation exists in this state.
    pub fn tree_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        // Once published, entries are immutable and have passed full tree
        // admission. Recount their actual allocation capacities for each owner.
        self.entries
            .iter()
            .try_fold(Self::metadata_base(self.entries.len())?, |bytes, entry| {
                add(bytes, entry.metadata_bytes()?)
            })
    }

    fn metadata_base(entries: usize) -> Result<usize, RepositoryWorkspaceError> {
        add(
            array::<Self>(1)?,
            array::<RepositorySourceSnapshotEntry>(entries)?,
        )
    }
    fn validate_entries(
        reservation: &RepositorySourceSnapshotReservation,
        entries: &[RepositorySourceSnapshotEntry],
    ) -> Result<usize, RepositoryWorkspaceError> {
        if entries.is_empty() || entries.len() > REPOSITORY_SOURCE_SNAPSHOT_ENTRIES {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        if entries[0].path() != "" || entries[0].node != RepositorySourceSnapshotNode::Directory {
            return Err(RepositoryWorkspaceError::MissingRoot);
        }
        let mut metadata = Self::metadata_base(entries.len())?;
        let mut names = 0usize;
        let mut file_bytes = 0u64;
        let display_len = reservation.display_root().as_path().as_os_str().len();
        for (i, e) in entries.iter().enumerate() {
            // Entry construction owns the intrinsic path and mode checks.
            if add(
                add(display_len, usize::from(!e.path.is_empty()))?,
                e.path.len(),
            )? > 4096
            {
                return Err(RepositoryWorkspaceError::InvalidPath);
            }
            if i > 0 {
                if entries[i - 1].path() >= e.path() {
                    return Err(RepositoryWorkspaceError::NonCanonicalEntries);
                }
                let parent = e.path().rsplit_once('/').map_or("", |(p, _)| p);
                if entries[..i]
                    .binary_search_by(|v| v.path().cmp(parent))
                    .ok()
                    .is_none_or(|p| entries[p].node != RepositorySourceSnapshotNode::Directory)
                {
                    return Err(RepositoryWorkspaceError::MissingParent);
                }
            }
            names = add(names, e.path.len())?;
            match &e.node {
                RepositorySourceSnapshotNode::Directory => {}
                RepositorySourceSnapshotNode::File(d) => {
                    if d.size_bytes() > REPOSITORY_SOURCE_SNAPSHOT_FILE_BYTES {
                        return Err(RepositoryWorkspaceError::StateLimit);
                    }
                    file_bytes = file_bytes
                        .checked_add(d.size_bytes())
                        .filter(|n| *n <= REPOSITORY_SOURCE_SNAPSHOT_TOTAL_BYTES)
                        .ok_or(RepositoryWorkspaceError::StateLimit)?;
                }
                RepositorySourceSnapshotNode::Link(target) => {
                    if target.is_empty()
                        || target.starts_with('/')
                        || target.len() > REPOSITORY_SOURCE_SNAPSHOT_PATH
                        || target.contains(['\0', '\\'])
                    {
                        return Err(RepositoryWorkspaceError::InvalidPath);
                    }
                    let mut depth = e
                        .path()
                        .rsplit_once('/')
                        .map_or(0, |(p, _)| p.split('/').count());
                    for component in target.split('/') {
                        match component {
                            "" | "." => {}
                            ".." => {
                                depth = depth
                                    .checked_sub(1)
                                    .ok_or(RepositoryWorkspaceError::InvalidPath)?
                            }
                            _ => depth = add(depth, 1)?,
                        }
                        if depth > REPOSITORY_SOURCE_SNAPSHOT_DEPTH {
                            return Err(RepositoryWorkspaceError::StateLimit);
                        }
                    }
                    names = add(names, target.len())?;
                }
            }
            metadata = add(metadata, e.metadata_bytes()?)?;
            if names > REPOSITORY_SOURCE_SNAPSHOT_NAMES
                || metadata > REPOSITORY_SOURCE_SNAPSHOT_METADATA
            {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
        }
        Ok(metadata)
    }
}
