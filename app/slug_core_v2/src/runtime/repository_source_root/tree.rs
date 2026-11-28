//! Bounded nofollow digest capture. Optional copying writes directly to the
//! selected backing; no file payload Vec/Arc is retained by the tree.
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use super::*;

pub(in crate::runtime::repository_physical_root) struct CapturedTree {
    entries: Vec<Entry>,
    stats: Vec<libc::stat>,
    _credit: ResourceCredit,
}
pub(super) struct RegularFileFrames {
    frames: Vec<(usize, libc::stat)>,
    _credit: ResourceCredit,
}

impl RegularFileFrames {
    pub(super) fn get(&self, entry: usize) -> Option<&libc::stat> {
        let index = self
            .frames
            .binary_search_by_key(&entry, |(index, _)| *index)
            .ok()?;
        Some(&self.frames[index].1)
    }
}

impl CapturedTree {
    pub(super) fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub(super) fn same_host_capture(&self, other: &Self) -> bool {
        self.entries == other.entries
            && self.stats.len() == other.stats.len()
            && self
                .stats
                .iter()
                .zip(&other.stats)
                .all(|(a, b)| crate::runtime::repository_root_io::unchanged(a, b))
    }
    pub(super) fn retain_regular_frames(
        &self,
        resources: &Arc<RepositoryRootResources>,
    ) -> Option<RegularFileFrames> {
        let count = self
            .entries
            .iter()
            .filter(|entry| matches!(entry.node(), Node::File(_)))
            .count();
        if count == 0 {
            return None;
        }
        let allocation = count
            .checked_mul(std::mem::size_of::<(usize, libc::stat)>())?
            .checked_add(64)?;
        // The complete capture remains paid during fitting and copying. A
        // refused table leaves the sealed owner and ordinary observation intact.
        let credit = resources.reserve(allocation, 0, 0).ok()?;
        let mut frames = Vec::new();
        frames.try_reserve_exact(count).ok()?;
        if frames.capacity() != count {
            return None;
        }
        frames.extend(self.entries.iter().zip(&self.stats).enumerate().filter_map(
            |(index, (entry, frame))| {
                matches!(entry.node(), Node::File(_)).then_some((index, *frame))
            },
        ));
        Some(RegularFileFrames {
            frames,
            _credit: credit,
        })
    }
    pub(in crate::runtime::repository_physical_root) fn into_state(
        self,
        reservation: Arc<Reservation>,
        resources: &Arc<RepositoryRootResources>,
    ) -> Result<(Arc<State>, ResourceCredit), PhysicalRootError> {
        let bytes = State::retained_storage_bytes_for(&reservation, &self.entries)
            .map_err(PhysicalRootError::Protocol)?;
        let credit = resources.reserve(add(bytes, arc_bytes::<State>()?)?, 0, 0)?;
        let state =
            Arc::new(State::new(reservation, self.entries).map_err(PhysicalRootError::Protocol)?);
        Ok((state, credit))
    }
}
pub(super) fn capture(
    source: &RootIo<'_>,
    reservation: &Arc<Reservation>,
    destination: Option<&RootIo<'_>>,
    resources: &Arc<RepositoryRootResources>,
    scope: &Arc<SourceServiceScope>,
    active: &dyn Fn() -> bool,
) -> Result<CapturedTree, PhysicalRootError> {
    check(scope, active)?;
    let credit = resources.reserve(8 * 1024 * 1024, 0, 0)?;
    let _scratch = resources.reserve(2 * IO_SCRATCH, 8, 0)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(8192)
        .map_err(|_| PhysicalRootError::Resource)?;
    let mut stats = Vec::new();
    stats
        .try_reserve_exact(8192)
        .map_err(|_| PhysicalRootError::Resource)?;
    let root = source.root_stat()?;
    if root.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Err(PhysicalRootError::Snapshot);
    }
    entries.push(
        Entry::new(
            CompactString::new(""),
            (root.st_mode & 0o7777) as u16,
            Node::Directory,
        )
        .map_err(PhysicalRootError::Protocol)?,
    );
    stats.push(root);
    let running = || active() && scope.check().is_ok();
    let mut total = 0u64;
    let mut names = 0usize;
    let mut index = 0;
    while index < entries.len() {
        check(scope, active)?;
        let framed = stats[index];
        match entries[index].node() {
            Node::Directory => {
                // Reading this directory appends to the same entry vector.
                let path = entries[index].path().to_owned();
                if let Some(target) = destination {
                    target.ensure_directory(Path::new(&path), 0o755)?;
                }
                source.read_directory(Path::new(&path), &running, |name, stat| {
                    if entries.len() == 8192 {
                        return Err(io::Error::other("source entry limit"));
                    }
                    let name = name
                        .to_str()
                        .ok_or_else(|| io::Error::other("source path is not UTF-8"))?;
                    let joined = if path.is_empty() {
                        name.to_owned()
                    } else {
                        format!("{path}/{name}")
                    };
                    names = names
                        .checked_add(joined.len())
                        .filter(|n| *n <= 1024 * 1024)
                        .ok_or_else(|| io::Error::other("source path limit"))?;
                    let node = match stat.st_mode & libc::S_IFMT {
                        libc::S_IFDIR => Node::Directory,
                        libc::S_IFREG
                            if stat.st_nlink == 1
                                && stat.st_size >= 0
                                && stat.st_size as u64 <= 64 * 1024 * 1024 =>
                        {
                            total = total
                                .checked_add(stat.st_size as u64)
                                .filter(|n| *n <= 256 * 1024 * 1024)
                                .ok_or_else(|| io::Error::other("source byte limit"))?;
                            Node::File(
                                FileContentDigest::new([0; 32], stat.st_size as u64)
                                    .ok_or_else(|| io::Error::other("source size"))?,
                            )
                        }
                        libc::S_IFLNK if stat.st_nlink == 1 => {
                            Node::Link(CompactString::new("pending"))
                        }
                        _ => return Err(io::Error::other("source kind or hardlink refused")),
                    };
                    entries.push(
                        Entry::new(
                            CompactString::new(joined),
                            (stat.st_mode & 0o7777) as u16,
                            node,
                        )
                        .map_err(|_| io::Error::other("source path refused"))?,
                    );
                    stats.push(*stat);
                    Ok(())
                })?;
            }
            Node::File(_) => {
                let entry = &mut entries[index];
                let path = Path::new(entry.path());
                let mut output = destination
                    .map(|d| {
                        d.open_write_regular(
                            path,
                            super::super::super::repository_root_io::WriteMode::CreateNew,
                        )
                    })
                    .transpose()?;
                let captured = source.read_file(path, &running, |bytes| {
                    if let Some(file) = &mut output {
                        crate::runtime::repository_io::source_service::write_counted(
                            file,
                            bytes,
                            &running,
                            Some(scope),
                        )?;
                    }
                    Ok(())
                })?;
                if !super::super::seal::unchanged(&framed, &captured.metadata) {
                    return Err(PhysicalRootError::Snapshot);
                }
                stats[index] = captured.metadata;
                if let (Some(target), Some(file)) = (destination, output.as_ref()) {
                    target.chmod_regular(file, (framed.st_mode & 0o7777) as u32)?;
                }
                entry.replace_node(Node::File(
                    FileContentDigest::new(captured.digest, framed.st_size as u64)
                        .ok_or(PhysicalRootError::Snapshot)?,
                ));
            }
            Node::Link(_) => {
                let entry = &mut entries[index];
                let path = Path::new(entry.path());
                let mut buffer = [0u8; 257];
                let target = source.read_link(path, &mut buffer)?;
                let target =
                    std::str::from_utf8(target).map_err(|_| PhysicalRootError::Snapshot)?;
                names = add(names, target.len())?;
                if names > 1024 * 1024 {
                    return Err(PhysicalRootError::Resource);
                }
                if let Some(destination) = destination {
                    destination.symlink_relative(path, Path::new(target))?;
                }
                if !super::super::seal::unchanged(&framed, &source.lstat(path)?) {
                    return Err(PhysicalRootError::Snapshot);
                }
                entry.replace_node(Node::Link(CompactString::new(target)));
            }
        }
        index += 1;
    }
    if let Some(target) = destination {
        for entry in entries.iter().rev() {
            if matches!(entry.node(), Node::Directory) {
                target.set_mode(Path::new(entry.path()), entry.mode() as u32)?;
            }
        }
    }
    // Preserve canonical path/frame association for the Local Host rewalk and
    // the native sealed owner. Frames never enter the semantic Source state.
    let mut order = Vec::new();
    order
        .try_reserve_exact(entries.len())
        .map_err(|_| PhysicalRootError::Resource)?;
    order.extend(0..entries.len());
    order.sort_unstable_by(|a, b| entries[*a].path().cmp(entries[*b].path()));
    let mut ordered_stats = Vec::new();
    ordered_stats
        .try_reserve_exact(stats.len())
        .map_err(|_| PhysicalRootError::Resource)?;
    ordered_stats.extend(order.into_iter().map(|index| stats[index]));
    entries.sort_unstable_by(|a, b| a.path().cmp(b.path()));
    State::retained_storage_bytes_for(reservation, &entries)
        .map_err(PhysicalRootError::Protocol)?;
    check(scope, active)?;
    Ok(CapturedTree {
        entries,
        stats: ordered_stats,
        _credit: credit,
    })
}
pub(super) fn observation_storage(
    epoch: &PathObservationEpoch,
) -> Result<usize, PhysicalRootError> {
    epoch
        .observations()
        .iter()
        .try_fold(4096usize, |n, (d, _)| {
            add(n, add(d.path().as_path().as_os_str().len(), 1024)?)
        })
}
