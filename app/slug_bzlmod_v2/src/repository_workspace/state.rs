use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub enum RepositoryWorkspaceMode {
    Archive,
    Process,
}

/// Literal symlink text is preserved. External targets require an exact Source
/// binding in the same immutable state; resolution handles internal links,
/// one-way Source transitions, missing targets and cycles separately.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryWorkspaceNode {
    Directory,
    File(Arc<[u8]>),
    Link(CompactString),
}
impl fmt::Debug for RepositoryWorkspaceNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory => f.write_str("Directory"),
            Self::File(body) => f.debug_tuple("FileBytes").field(&body.len()).finish(),
            Self::Link(target) => f.debug_tuple("LinkBytes").field(&target.len()).finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceEntry {
    path: CompactString,
    mode: u16,
    node: RepositoryWorkspaceNode,
}
impl RepositoryWorkspaceEntry {
    /// Transfers already-budgeted snapshot data; does not copy its body/path.
    pub fn new(
        path: CompactString,
        mode: u16,
        node: RepositoryWorkspaceNode,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if !valid_relative(&path, true) || mode & !0o7777 != 0 {
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
    pub fn node(&self) -> &RepositoryWorkspaceNode {
        &self.node
    }
    pub(super) fn body_mut(&mut self) -> Option<&mut Arc<[u8]>> {
        match &mut self.node {
            RepositoryWorkspaceNode::File(body) => Some(body),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceState {
    reservation: Arc<RepositoryWorkspaceReservation>,
    mode: RepositoryWorkspaceMode,
    entries: Arc<[RepositoryWorkspaceEntry]>,
    source_bindings: Arc<[RepositoryWorkspaceReadView]>,
}
impl RepositoryWorkspaceState {
    pub fn reservation(&self) -> &Arc<RepositoryWorkspaceReservation> {
        &self.reservation
    }
    pub fn mode(&self) -> RepositoryWorkspaceMode {
        self.mode
    }
    pub fn entries(&self) -> &[RepositoryWorkspaceEntry] {
        &self.entries
    }
    pub fn entry(&self, path: &str) -> Option<&RepositoryWorkspaceEntry> {
        self.entries
            .binary_search_by(|entry| entry.path().cmp(path))
            .ok()
            .map(|i| &self.entries[i])
    }

    pub fn source_bindings(&self) -> &[RepositoryWorkspaceReadView] {
        &self.source_bindings
    }
    pub fn visit_read_views(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        for view in self.source_bindings.iter() {
            visit(view);
        }
    }

    /// Complete state/entry/path/link and reservation guard, excluding the
    /// allocations owned by File bodies. Borrows existing storage only.
    #[doc(hidden)]
    pub fn retained_metadata_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        add(
            self.reservation.metadata()?,
            add(
                Self::validate_base(&self.reservation, self.mode, &self.entries)?,
                state_sources::BindingSelection::for_state(self)?.owned_bytes()?,
            )?,
        )
    }

    /// Complete conservative retained state guard. Every File occurrence pays
    /// its full Arc allocation header, padding and payload, even when shared.
    #[doc(hidden)]
    pub fn retained_storage_bytes(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut bytes = self.retained_metadata_bytes()?;
        for entry in self.entries.iter() {
            if let RepositoryWorkspaceNode::File(body) = entry.node() {
                bytes = add(bytes, array_bytes::<u8>(body.len())?)?;
            }
        }
        add(
            bytes,
            read_views::source_storage_bytes(|visit| self.visit_read_views(&mut |v| visit(v)))?,
        )
    }

    pub fn storage_partition(
        &self,
    ) -> Result<RepositoryWorkspaceStoragePartition, RepositoryWorkspaceError> {
        let mut owned = self.retained_metadata_bytes()?;
        for entry in self.entries.iter() {
            if let RepositoryWorkspaceNode::File(body) = entry.node() {
                owned = add(owned, array_bytes::<u8>(body.len())?)?;
            }
        }
        RepositoryWorkspaceStoragePartition::new(
            owned,
            read_views::source_storage_bytes(|visit| self.visit_read_views(&mut |v| visit(v)))?,
        )
    }

    pub(super) fn from_admitted(
        reservation: Arc<RepositoryWorkspaceReservation>,
        mode: RepositoryWorkspaceMode,
        entries: Vec<RepositoryWorkspaceEntry>,
    ) -> Self {
        Self {
            reservation,
            mode,
            entries: entries.into(),
            source_bindings: Arc::from([]),
        }
    }
    pub(super) fn from_admitted_with_sources(
        reservation: Arc<RepositoryWorkspaceReservation>,
        mode: RepositoryWorkspaceMode,
        entries: Vec<RepositoryWorkspaceEntry>,
        source_bindings: Vec<RepositoryWorkspaceReadView>,
    ) -> Self {
        Self {
            reservation,
            mode,
            entries: entries.into(),
            source_bindings: source_bindings.into(),
        }
    }
    pub(super) fn validate(
        reservation: &RepositoryWorkspaceReservation,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<usize, RepositoryWorkspaceError> {
        add(
            Self::validate_base(reservation, mode, entries)?,
            state_sources::BindingSelection::new(reservation, entries, std::iter::empty())?
                .owned_bytes()?,
        )
    }
    pub(super) fn validate_base(
        reservation: &RepositoryWorkspaceReservation,
        mode: RepositoryWorkspaceMode,
        entries: &[RepositoryWorkspaceEntry],
    ) -> Result<usize, RepositoryWorkspaceError> {
        let process = mode == RepositoryWorkspaceMode::Process;
        let (max_entries, max_names, max_body) = if process {
            (
                REPOSITORY_WORKSPACE_PROCESS_ENTRIES,
                REPOSITORY_WORKSPACE_PROCESS_NAMES,
                REPOSITORY_WORKSPACE_PROCESS_BODY_BYTES,
            )
        } else {
            (131_072, 4 * 1024 * 1024, 1024 * 1024 * 1024)
        };
        if entries.is_empty() || entries.len() > max_entries {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        if entries[0].path() != "" || entries[0].node != RepositoryWorkspaceNode::Directory {
            return Err(RepositoryWorkspaceError::MissingRoot);
        }
        let root = reservation
            .display_root
            .as_path()
            .to_str()
            .ok_or(RepositoryWorkspaceError::InvalidPath)?;
        let mut names = 0usize;
        let mut bodies = 0u64;
        let mut metadata = add(
            add(
                std::mem::size_of::<Self>(),
                2 * std::mem::size_of::<usize>(),
            )?,
            array_bytes::<RepositoryWorkspaceEntry>(entries.len())?,
        )?;
        for (index, entry) in entries.iter().enumerate() {
            let path = entry.path();
            if !valid_relative(path, true) || entry.mode & !0o7777 != 0 {
                return Err(RepositoryWorkspaceError::InvalidPath);
            }
            if index > 0 {
                if entries[index - 1].path() >= path {
                    return Err(RepositoryWorkspaceError::NonCanonicalEntries);
                }
                let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                let parent_entry = entries[..index]
                    .binary_search_by(|entry| entry.path().cmp(parent))
                    .ok()
                    .map(|i| &entries[i]);
                if parent_entry.is_none_or(|entry| entry.node != RepositoryWorkspaceNode::Directory)
                {
                    return Err(RepositoryWorkspaceError::MissingParent);
                }
            }
            let absolute_len = add(
                root.len(),
                if path.is_empty() {
                    0
                } else {
                    add(1, path.len())?
                },
            )?;
            if absolute_len > 4096 || (process && !path.is_empty() && path.split('/').count() > 64)
            {
                return Err(RepositoryWorkspaceError::StateLimit);
            }
            names = add(names, path.len())?;
            metadata = add(metadata, entry.path.capacity())?;
            match &entry.node {
                RepositoryWorkspaceNode::Directory => {}
                RepositoryWorkspaceNode::File(body) => {
                    bodies = bodies
                        .checked_add(body.len() as u64)
                        .ok_or(RepositoryWorkspaceError::StateLimit)?;
                }
                RepositoryWorkspaceNode::Link(target) => {
                    if target.is_empty() || target.len() > 4096 || target.contains(['\0', '\\']) {
                        return Err(RepositoryWorkspaceError::InvalidPath);
                    }
                    let relative = if target.starts_with('/') {
                        target
                            .strip_prefix(root)
                            .and_then(|tail| {
                                if tail.is_empty() {
                                    Some("")
                                } else {
                                    tail.strip_prefix('/')
                                }
                            })
                            .unwrap_or("")
                    } else {
                        target.as_str()
                    };
                    let mut depth = if target.starts_with('/') {
                        0
                    } else {
                        path.rsplit_once('/')
                            .map_or(0, |(parent, _)| parent.split('/').count())
                    };
                    for part in relative.split('/') {
                        match part {
                            "" | "." => {}
                            ".." => {
                                depth = depth
                                    .checked_sub(1)
                                    .ok_or(RepositoryWorkspaceError::InvalidPath)?;
                            }
                            _ => {
                                depth = add(depth, 1)?;
                            }
                        }
                    }
                    names = add(names, target.len())?;
                    metadata = add(metadata, target.capacity())?;
                }
            }
        }
        if names > max_names || bodies > max_body {
            return Err(RepositoryWorkspaceError::StateLimit);
        }
        Ok(metadata)
    }
}
