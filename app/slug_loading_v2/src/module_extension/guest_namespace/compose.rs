//! Full source occurrences are admitted before physical node deduplication.
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::ImageDescriptorReservation;
use slug_workspace_v2::InputImageEntryKind;
use slug_workspace_v2::ObservedInputImage;

use super::*;

#[derive(Clone, Copy)]
enum ScopeKind {
    Scratch,
    Tool(usize),
    Runtime(usize),
}
#[derive(Clone, Copy)]
struct Scope<'a> {
    root: &'a str,
    image: Option<&'a ObservedInputImage>,
    kind: ScopeKind,
    duplicate: bool,
}
#[derive(Clone, Copy)]
enum Data<'a> {
    Directory,
    Digest(FileContentDigest),
    Bytes(&'a [u8]),
}
struct Row<'a> {
    root: &'a str,
    suffix: &'a str,
    data: Data<'a>,
    mode: u32,
    binding: GuestNamespaceBinding,
    read_only: bool,
}
fn active(check: &dyn Fn() -> bool) -> Result<(), GuestNamespaceFactsError> {
    if check() {
        Ok(())
    } else {
        Err(GuestNamespaceFactsError::Operational)
    }
}
fn scopes(
    profile: &ObservedRuntimeSourceProfile,
) -> Result<([Option<Scope<'_>>; 68], usize), GuestNamespaceFactsError> {
    let mut rows: [Option<Scope<'_>>; 68] = [None; 68];
    rows[0] = Some(Scope {
        root: profile
            .tool_sources()
            .prefix()
            .prefix()
            .policy()
            .guest_root(),
        image: None,
        kind: ScopeKind::Scratch,
        duplicate: false,
    });
    let mut length = 1;
    for (role, source) in profile.tool_sources().sources().enumerate() {
        let Some(metadata) = source.label_metadata() else {
            continue;
        };
        let image = metadata.image();
        let root = image
            .source_root()
            .as_path()
            .to_str()
            .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
        rows[length] = Some(Scope {
            root,
            image: Some(image),
            kind: ScopeKind::Tool(role),
            duplicate: false,
        });
        length += 1;
    }
    for (declaration, (request, image)) in profile
        .requests()
        .requests()
        .zip(profile.images())
        .enumerate()
    {
        rows[length] = Some(Scope {
            root: request.guest_prefix().as_str(),
            image: Some(image),
            kind: ScopeKind::Runtime(declaration),
            duplicate: false,
        });
        length += 1;
    }
    for i in 0..length {
        let current = rows[i].unwrap();
        let mut duplicate = current.duplicate;
        for previous in rows[..i].iter().flatten() {
            if Path::new(current.root).starts_with(previous.root)
                || Path::new(previous.root).starts_with(current.root)
            {
                if let (ScopeKind::Runtime(declaration), ScopeKind::Tool(previous_role)) =
                    (current.kind, previous.kind)
                {
                    if let Some(crate::module_extension::runtime_sources::RuntimeSourceImageBinding::CoveredTool { role, .. }) = profile.image_binding(declaration) {
                        let certified = profile.tool_sources().sources().nth(role)
                            .and_then(|s| s.label_metadata()).map(|m| m.image());
                        if (role == previous_role || certified == previous.image)
                            && Path::new(current.root).starts_with(previous.root)
                            && current.image.is_some_and(|image| image.source_root().as_path() == Path::new(current.root)) {
                            duplicate = true;
                            continue;
                        }
                    }
                }
                if matches!(current.kind, ScopeKind::Tool(_))
                    && matches!(previous.kind, ScopeKind::Tool(_))
                    && current.root == previous.root
                    && current.image == previous.image
                {
                    duplicate = true;
                    break;
                }
                return Err(GuestNamespaceFactsError::Collision);
            }
        }
        rows[i].as_mut().unwrap().duplicate = duplicate;
    }
    Ok((rows, length))
}
fn visit(
    scope: Scope<'_>,
    profile: &ObservedRuntimeSourceProfile,
    visitor: &mut dyn FnMut(Row<'_>) -> Result<(), GuestNamespaceFactsError>,
) -> Result<(), GuestNamespaceFactsError> {
    if let Some(image) = scope.image {
        for (entry, item) in image.entries().iter().enumerate() {
            let suffix = item
                .path()
                .as_path()
                .strip_prefix(image.source_root().as_path())
                .map_err(|_| GuestNamespaceFactsError::UnsupportedSource)?
                .to_str()
                .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
            let data = match item.kind() {
                InputImageEntryKind::Directory => Data::Directory,
                InputImageEntryKind::RegularFile(d) => Data::Digest(d),
                _ => return Err(GuestNamespaceFactsError::UnsupportedSource),
            };
            let binding = match scope.kind {
                ScopeKind::Tool(role) => GuestNamespaceBinding::ToolImage { role, entry },
                ScopeKind::Runtime(declaration) => {
                    GuestNamespaceBinding::RuntimeImage { declaration, entry }
                }
                _ => unreachable!(),
            };
            visitor(Row {
                root: scope.root,
                suffix,
                data,
                mode: item.mode(),
                binding,
                read_only: true,
            })?;
        }
    } else {
        for (entry, item) in profile
            .tool_sources()
            .prefix()
            .prefix()
            .entries()
            .iter()
            .enumerate()
        {
            let data = item
                .blob()
                .map_or(Data::Directory, |b| Data::Bytes(b.bytes()));
            let read_only = profile.tool_sources().sources().any(|s| {
                s.scratch_result()
                    .is_some_and(|s| s.request().entry_index() == entry)
            });
            visitor(Row {
                root: scope.root,
                suffix: item.path().relative(),
                data,
                mode: item.mode(),
                binding: GuestNamespaceBinding::ScratchEntry { entry },
                read_only,
            })?;
        }
    }
    Ok(())
}
fn parents(
    root: &str,
    f: &mut dyn FnMut(&str) -> Result<(), GuestNamespaceFactsError>,
) -> Result<(), GuestNamespaceFactsError> {
    let mut current = root;
    while current != "/" {
        let (parent, _) = current
            .rsplit_once('/')
            .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
        current = if parent.is_empty() { "/" } else { parent };
        f(current)?;
    }
    Ok(())
}
#[derive(Default)]
struct Totals {
    entries: usize,
    paths: usize,
    bytes: u64,
}
impl Totals {
    fn add(
        &mut self,
        root: &str,
        suffix: &str,
        bytes: u64,
        policy: GuestNamespacePolicy,
    ) -> Result<(), GuestNamespaceFactsError> {
        let len = root
            .len()
            .checked_add(suffix.len())
            .and_then(|n| n.checked_add(usize::from(!suffix.is_empty())))
            .ok_or(GuestNamespaceFactsError::Limit)?;
        let depth = root.split('/').filter(|s| !s.is_empty()).count()
            + suffix.split('/').filter(|s| !s.is_empty()).count();
        self.entries = self
            .entries
            .checked_add(1)
            .ok_or(GuestNamespaceFactsError::Limit)?;
        self.paths = self
            .paths
            .checked_add(len)
            .ok_or(GuestNamespaceFactsError::Limit)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(GuestNamespaceFactsError::Limit)?;
        let limits = policy.limits();
        if len > limits.max_path_bytes
            || depth > limits.max_depth
            || self.entries > limits.max_entries
            || self.paths > limits.max_descriptor_bytes
            || self.bytes > limits.max_total_file_bytes
        {
            return Err(GuestNamespaceFactsError::Limit);
        }
        Ok(())
    }
}
fn size(data: Data<'_>) -> u64 {
    match data {
        Data::Directory => 0,
        Data::Digest(d) => d.size_bytes(),
        Data::Bytes(b) => b.len() as u64,
    }
}
fn reserve<T>(
    credit: &mut ImageDescriptorReservation,
    n: usize,
) -> Result<Vec<T>, GuestNamespaceFactsError> {
    credit
        .try_grow(
            n.checked_mul(std::mem::size_of::<T>())
                .and_then(|n| n.checked_add(256))
                .ok_or(GuestNamespaceFactsError::Operational)?,
        )
        .map_err(|_| GuestNamespaceFactsError::Operational)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| GuestNamespaceFactsError::Operational)?;
    Ok(v)
}
fn append(
    nodes: &mut Vec<GuestNamespaceNode>,
    credit: &mut ImageDescriptorReservation,
    row: Row<'_>,
    policy: GuestNamespacePolicy,
    check: &dyn Fn() -> bool,
) -> Result<(), GuestNamespaceFactsError> {
    active(check)?;
    let length = row.root.len() + row.suffix.len() + usize::from(!row.suffix.is_empty());
    credit
        .try_grow(length + 128)
        .map_err(|_| GuestNamespaceFactsError::Operational)?;
    let mut path = String::new();
    path.try_reserve_exact(length)
        .map_err(|_| GuestNamespaceFactsError::Operational)?;
    path.push_str(row.root);
    if !row.suffix.is_empty() {
        path.push('/');
        path.push_str(row.suffix)
    }
    let kind = match row.data {
        Data::Directory => GuestNamespaceNodeKind::Directory,
        Data::Digest(d) => GuestNamespaceNodeKind::RegularFile(d),
        Data::Bytes(bytes) => {
            let mut hash = Sha256::new();
            for chunk in bytes.chunks(64 * 1024) {
                active(check)?;
                hash.update(chunk)
            }
            GuestNamespaceNodeKind::RegularFile(
                FileContentDigest::new(hash.finalize().into(), bytes.len() as u64)
                    .ok_or(GuestNamespaceFactsError::Limit)?,
            )
        }
    };
    let size = match kind {
        GuestNamespaceNodeKind::Directory => 4096,
        GuestNamespaceNodeKind::RegularFile(d) => d.size_bytes(),
    };
    let metadata = GuestNamespaceMetadata {
        device: policy.device(),
        inode: 0,
        nlink: 1,
        mode: row.mode
            | match kind {
                GuestNamespaceNodeKind::Directory => 0o040000,
                _ => 0o100000,
            },
        uid: policy.owner_uid(),
        gid: policy.owner_gid(),
        size,
        block_size: 4096,
        blocks: size.div_ceil(512),
        epoch_seconds: policy.epoch_seconds(),
        epoch_nanoseconds: policy.epoch_nanoseconds(),
    };
    nodes.push(GuestNamespaceNode {
        index: 0,
        path,
        kind,
        mode: row.mode,
        binding: row.binding,
        read_only: row.read_only,
        parent: None,
        first_child: 0,
        child_count: 0,
        metadata,
    });
    Ok(())
}
fn scaffold(path: &str, policy: GuestNamespacePolicy) -> Row<'_> {
    Row {
        root: path,
        suffix: "",
        data: Data::Directory,
        mode: policy.scaffold_mode(),
        binding: GuestNamespaceBinding::Scaffold,
        read_only: true,
    }
}
fn link(
    nodes: &mut [GuestNamespaceNode],
    credit: &mut ImageDescriptorReservation,
    check: &dyn Fn() -> bool,
) -> Result<Vec<usize>, GuestNamespaceFactsError> {
    let mut edges = reserve::<(usize, usize)>(credit, nodes.len())?;
    for i in 0..nodes.len() {
        active(check)?;
        nodes[i].index = i;
        nodes[i].metadata.inode = i as u64 + 1;
        if nodes[i].path == "/" {
            continue;
        }
        let parent = nodes[i]
            .path
            .rsplit_once('/')
            .ok_or(GuestNamespaceFactsError::Collision)?
            .0;
        let parent = if parent.is_empty() { "/" } else { parent };
        let index = nodes
            .binary_search_by(|n| n.path.as_str().cmp(parent))
            .map_err(|_| GuestNamespaceFactsError::Collision)?;
        if nodes[index].kind != GuestNamespaceNodeKind::Directory {
            return Err(GuestNamespaceFactsError::Collision);
        }
        nodes[i].parent = Some(index);
        edges.push((index, i));
    }
    edges.sort_unstable();
    let mut children = reserve::<usize>(credit, edges.len())?;
    for (parent, child) in edges {
        if nodes[parent].child_count == 0 {
            nodes[parent].first_child = children.len()
        }
        nodes[parent].child_count += 1;
        children.push(child);
    }
    for i in 0..nodes.len() {
        if nodes[i].kind == GuestNamespaceNodeKind::Directory {
            let n = children[nodes[i].first_child..nodes[i].first_child + nodes[i].child_count]
                .iter()
                .filter(|child| nodes[**child].kind == GuestNamespaceNodeKind::Directory)
                .count();
            nodes[i].metadata.nlink = 2 + n as u64;
        }
    }
    Ok(children)
}

pub(super) fn build(
    key: &SelectedGuestNamespaceFactsKey,
    profile: ObservedRuntimeSourceProfile,
    data_sources: Option<ObservedSelectedDataLabelSources>,
    policy: GuestNamespacePolicy,
    resources: &SelectedScratchResources,
    check: &dyn Fn() -> bool,
) -> Result<ObservedGuestNamespaceFacts, GuestNamespaceFactsError> {
    active(check)?;
    let mut credit = resources
        .descriptor_budget()
        .try_reserve(
            std::mem::size_of::<FactsInner>()
                + std::mem::size_of::<[Option<Scope<'_>>; 68]>()
                + 512,
        )
        .map_err(|_| GuestNamespaceFactsError::Operational)?;
    let (mounts, length) = scopes(&profile)?;
    let mut totals = Totals::default();
    for scope in mounts[..length].iter().flatten() {
        visit(*scope, &profile, &mut |r| {
            active(check)?;
            if r.mode & !0o777 != 0 {
                return Err(GuestNamespaceFactsError::UnsupportedSource);
            }
            totals.add(r.root, r.suffix, size(r.data), policy)
        })?;
        parents(scope.root, &mut |p| totals.add(p, "", 0, policy))?;
    }
    if let Some(data) = &data_sources {
        preflight_data(data, &mounts[..length])?;
        visit_data(data, &mut |r| {
            active(check)?;
            if r.mode & !0o777 != 0 {
                return Err(GuestNamespaceFactsError::UnsupportedSource);
            }
            totals.add(r.root, "", size(r.data), policy)?;
            parents(r.root, &mut |p| totals.add(p, "", 0, policy))
        })?;
    }
    let mut nodes = reserve::<GuestNamespaceNode>(&mut credit, totals.entries)?;
    for scope in mounts[..length].iter().flatten() {
        if !scope.duplicate {
            visit(*scope, &profile, &mut |r| {
                append(&mut nodes, &mut credit, r, policy, check)
            })?;
        }
        parents(scope.root, &mut |p| {
            append(&mut nodes, &mut credit, scaffold(p, policy), policy, check)
        })?;
    }
    if let Some(data) = &data_sources {
        visit_data(data, &mut |r| {
            let root = r.root;
            append(&mut nodes, &mut credit, r, policy, check)?;
            parents(root, &mut |p| {
                let mut row = scaffold(p, policy);
                row.binding = GuestNamespaceBinding::DataScaffold;
                append(&mut nodes, &mut credit, row, policy, check)
            })
        })?;
    }
    // Prefer genuine complete source ownership, then data-required sparse parents.
    nodes.sort_unstable_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| priority(a.binding).cmp(&priority(b.binding)))
            .then_with(|| match (a.binding, b.binding) {
                (
                    GuestNamespaceBinding::DataLeaf { occurrence: a },
                    GuestNamespaceBinding::DataLeaf { occurrence: b },
                ) => a.cmp(&b),
                _ => std::cmp::Ordering::Equal,
            })
    });
    for pair in nodes.windows(2) {
        if pair[0].path == pair[1].path
            && !is_scaffold(pair[0].binding)
            && !is_scaffold(pair[1].binding)
            && (pair[0].kind != pair[1].kind
                || pair[0].mode != pair[1].mode
                || pair[0].binding != pair[1].binding)
        {
            // Full-equal new leaf duplicates were checked before allocation; retain first occurrence binding.
            if !matches!(
                (pair[0].binding, pair[1].binding),
                (
                    GuestNamespaceBinding::DataLeaf { .. },
                    GuestNamespaceBinding::DataLeaf { .. }
                )
            ) {
                return Err(GuestNamespaceFactsError::Collision);
            }
        }
    }
    nodes.dedup_by(|a, b| a.path == b.path);
    let children = link(&mut nodes, &mut credit, check)?;
    active(check)?;
    credit
        .freeze(FactsInner {
            key: key.clone(),
            profile,
            data_sources,
            policy,
            nodes,
            children,
            lookup_budget: resources.descriptor_budget().dupe(),
        })
        .map(ObservedGuestNamespaceFacts)
        .map_err(|_| GuestNamespaceFactsError::Operational)
}

fn is_scaffold(b: GuestNamespaceBinding) -> bool {
    matches!(
        b,
        GuestNamespaceBinding::Scaffold | GuestNamespaceBinding::DataScaffold
    )
}
fn priority(b: GuestNamespaceBinding) -> u8 {
    match b {
        GuestNamespaceBinding::Scaffold => 2,
        GuestNamespaceBinding::DataScaffold => 1,
        _ => 0,
    }
}
fn visit_data(
    data: &ObservedSelectedDataLabelSources,
    visitor: &mut dyn FnMut(Row<'_>) -> Result<(), GuestNamespaceFactsError>,
) -> Result<(), GuestNamespaceFactsError> {
    use super::super::selected_label_data::SelectedDataLabelBinding as Binding;
    for (request, source) in data.requests().requests().zip(data.sources()) {
        let (image, index) = data
            .member(source)
            .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
        let node = image
            .entries()
            .get(index)
            .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
        let root = request
            .selection()
            .selected_file()
            .as_path()
            .to_str()
            .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
        let kind = match node.kind() {
            InputImageEntryKind::Directory => Data::Directory,
            InputImageEntryKind::RegularFile(d) => Data::Digest(d),
            _ => return Err(GuestNamespaceFactsError::UnsupportedSource),
        };
        let binding = match source.binding() {
            Binding::Leaf { .. } => GuestNamespaceBinding::DataLeaf {
                occurrence: request.occurrence_index(),
            },
            Binding::ToolMember { role, entry } => GuestNamespaceBinding::ToolImage {
                role: *role,
                entry: *entry,
            },
            Binding::RuntimeMember { declaration, entry } => GuestNamespaceBinding::RuntimeImage {
                declaration: *declaration,
                entry: *entry,
            },
        };
        visitor(Row {
            root,
            suffix: "",
            data: kind,
            mode: node.mode(),
            binding,
            read_only: true,
        })?;
    }
    Ok(())
}
fn preflight_data(
    data: &ObservedSelectedDataLabelSources,
    mounts: &[Option<Scope<'_>>],
) -> Result<(), GuestNamespaceFactsError> {
    use super::super::selected_label_data::SelectedDataLabelBinding as Binding;
    for (request, source) in data.requests().requests().zip(data.sources()) {
        let path = request.selection().selected_file().as_path();
        for scope in mounts.iter().flatten() {
            if path.starts_with(scope.root) || Path::new(scope.root).starts_with(path) {
                let owned = match (source.binding(), scope.kind) {
                    (Binding::ToolMember { role, .. }, ScopeKind::Tool(r)) => {
                        *role == r
                            || data
                                .member(source)
                                .is_some_and(|(image, _)| scope.image == Some(image))
                    }
                    (Binding::RuntimeMember { declaration, .. }, ScopeKind::Runtime(r)) => {
                        *declaration == r
                    }
                    _ => false,
                };
                // A relocated runtime leaf is outside its guest tree; overlap is not a second spelling grant.
                if !owned || !path.starts_with(scope.root) {
                    return Err(GuestNamespaceFactsError::Collision);
                }
                let (image, entry) = data
                    .member(source)
                    .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
                let item = image
                    .entries()
                    .get(entry)
                    .ok_or(GuestNamespaceFactsError::UnsupportedSource)?;
                let suffix = item
                    .path()
                    .as_path()
                    .strip_prefix(image.source_root().as_path())
                    .map_err(|_| GuestNamespaceFactsError::Collision)?;
                if path.strip_prefix(scope.root).ok() != Some(suffix) {
                    return Err(GuestNamespaceFactsError::Collision);
                }
            }
        }
        for (old_request, old_source) in data
            .requests()
            .requests()
            .zip(data.sources())
            .take(request.occurrence_index())
        {
            let old = old_request.selection().selected_file().as_path();
            if path == old {
                if matches!(
                    (source.binding(), old_source.binding()),
                    (Binding::Leaf { .. }, Binding::Leaf { .. })
                ) {
                    if request.selection() != old_request.selection()
                        || source.leaf_image() != old_source.leaf_image()
                    {
                        return Err(GuestNamespaceFactsError::Collision);
                    }
                } else if source.binding() != old_source.binding() {
                    return Err(GuestNamespaceFactsError::Collision);
                }
            } else if path.starts_with(old) || old.starts_with(path) {
                // Covered directories already belong to a full real image. All other file ancestors refuse.
                let (a, ai) = data
                    .member(source)
                    .ok_or(GuestNamespaceFactsError::Collision)?;
                let (b, bi) = data
                    .member(old_source)
                    .ok_or(GuestNamespaceFactsError::Collision)?;
                if a != b
                    || (!matches!(a.entries()[ai].kind(), InputImageEntryKind::Directory)
                        && !matches!(b.entries()[bi].kind(), InputImageEntryKind::Directory))
                {
                    return Err(GuestNamespaceFactsError::Collision);
                }
            }
        }
    }
    Ok(())
}
