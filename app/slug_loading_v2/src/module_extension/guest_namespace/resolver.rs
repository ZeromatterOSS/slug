//! Borrowed sequential path resolution over a complete normal namespace table.
use super::*;

impl ObservedGuestNamespaceFacts {
    pub fn lookup(
        &self,
        path: &str,
        operation: GuestNamespaceOperation,
    ) -> Result<&GuestNamespaceNode, GuestLookupError> {
        self.lookup_with(path, operation, &mut |_, _| true)
    }

    pub fn lookup_with(
        &self,
        path: &str,
        operation: GuestNamespaceOperation,
        check: &mut dyn FnMut(&GuestNamespaceNode, GuestDirectoryCheck) -> bool,
    ) -> Result<&GuestNamespaceNode, GuestLookupError> {
        if !path.starts_with('/') || path.as_bytes().contains(&0) {
            return Err(GuestLookupError::InvalidPath);
        }
        if path.len() > 4096 || path.len() > self.policy().limits().max_path_bytes {
            return Err(GuestLookupError::PathLimit);
        }
        if matches!(operation, GuestNamespaceOperation::Unsupported)
            || matches!(operation, GuestNamespaceOperation::Access(mask) if mask > 7)
        {
            return Err(GuestLookupError::Unsupported);
        }
        // Physical scratch belongs to the actual service, independently of
        // semantic path admission. RAII also covers callback panic unwinding.
        let _credit = self
            .0
            .get()
            .lookup_budget
            .try_reserve(4352)
            .map_err(|_| GuestLookupError::ObservationRejected)?;
        let mut buffer = [0u8; 4096];
        buffer[..path.len()].copy_from_slice(path.as_bytes());
        // The copied input is already UTF8. Preserve raw components rather than
        // normalizing away file/., file/.. or permission-before-missing.
        let raw = std::str::from_utf8(&buffer[..path.len()])
            .map_err(|_| GuestLookupError::InvalidPath)?;
        let mut current = self
            .node(0)
            .filter(|node| node.path() == "/")
            .ok_or(GuestLookupError::Unowned)?;
        for component in raw.split('/').filter(|component| !component.is_empty()) {
            self.search_directory(current, check)?;
            match component {
                "." => {}
                ".." => {
                    current = self
                        .node(current.parent.unwrap_or(current.index()))
                        .ok_or(GuestLookupError::Unowned)?;
                }
                name => {
                    checked(current, GuestDirectoryCheck::Membership, check)?;
                    let children = self.children(current.index());
                    let position = children.binary_search_by(|index| {
                        let child = &self.nodes()[*index];
                        child
                            .path()
                            .rsplit('/')
                            .next()
                            .unwrap_or("")
                            .as_bytes()
                            .cmp(name.as_bytes())
                    });
                    current = match position {
                        Ok(position) => &self.nodes()[children[position]],
                        Err(_) if current.is_owned_tree() => return Err(GuestLookupError::Missing),
                        Err(_) => return Err(GuestLookupError::Unowned),
                    };
                }
            }
        }
        // A final slash requires a directory search; plain stat("/dir") does
        // not require final-directory R/X permission. Root has no parent edge.
        if raw.ends_with('/') && raw.bytes().any(|byte| byte != b'/') {
            self.search_directory(current, check)?;
        }
        self.finish_lookup(current, operation, check)
    }

    fn search_directory(
        &self,
        node: &GuestNamespaceNode,
        check: &mut dyn FnMut(&GuestNamespaceNode, GuestDirectoryCheck) -> bool,
    ) -> Result<(), GuestLookupError> {
        if node.kind() != GuestNamespaceNodeKind::Directory {
            checked(node, GuestDirectoryCheck::Metadata, check)?;
            return Err(GuestLookupError::NotDirectory);
        }
        checked(node, GuestDirectoryCheck::Traverse, check)?;
        self.permission(node, 1)
    }

    fn permission(&self, node: &GuestNamespaceNode, mask: u8) -> Result<(), GuestLookupError> {
        let policy = self.policy();
        let metadata = node.metadata();
        let shift = if policy.caller_uid() == metadata.uid {
            6
        } else if policy.caller_gid() == metadata.gid {
            3
        } else {
            0
        };
        let available = ((node.mode() >> shift) & 7) as u8;
        if available & mask != mask {
            Err(GuestLookupError::PermissionDenied)
        } else {
            Ok(())
        }
    }

    fn finish_lookup<'a>(
        &self,
        node: &'a GuestNamespaceNode,
        operation: GuestNamespaceOperation,
        check: &mut dyn FnMut(&GuestNamespaceNode, GuestDirectoryCheck) -> bool,
    ) -> Result<&'a GuestNamespaceNode, GuestLookupError> {
        // A backed final node must still match the certificate before even a
        // certified errno can be returned. Metadata checks apply to both file
        // and directory nodes, including denied reads/listings.
        checked(node, GuestDirectoryCheck::Metadata, check)?;
        let directory = node.kind() == GuestNamespaceNodeKind::Directory;
        if matches!(operation, GuestNamespaceOperation::ListDirectory) {
            if !directory {
                return Err(GuestLookupError::NotDirectory);
            }
            self.permission(node, 5)?;
            checked(node, GuestDirectoryCheck::Listing, check)?;
            return Ok(node);
        }
        match operation {
            GuestNamespaceOperation::Lstat | GuestNamespaceOperation::Stat => Ok(node),
            GuestNamespaceOperation::Access(mask) => {
                self.permission(node, mask)?;
                if mask & 2 != 0 && node.read_only() {
                    return Err(GuestLookupError::ReadOnly);
                }
                Ok(node)
            }
            GuestNamespaceOperation::ReadRegular => {
                if directory {
                    return Err(GuestLookupError::IsDirectory);
                }
                self.permission(node, 4)?;
                Ok(node)
            }
            GuestNamespaceOperation::ReadLink => Err(GuestLookupError::NotSymlink),
            GuestNamespaceOperation::ListDirectory | GuestNamespaceOperation::Unsupported => {
                Err(GuestLookupError::Unsupported)
            }
        }
    }
}

fn checked(
    node: &GuestNamespaceNode,
    kind: GuestDirectoryCheck,
    check: &mut dyn FnMut(&GuestNamespaceNode, GuestDirectoryCheck) -> bool,
) -> Result<(), GuestLookupError> {
    if check(node, kind) {
        Ok(())
    } else {
        Err(GuestLookupError::ObservationRejected)
    }
}
