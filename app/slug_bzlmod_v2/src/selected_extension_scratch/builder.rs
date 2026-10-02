use std::mem::size_of;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorReservation;

use super::SelectedScratchResources;
use super::types::*;
use crate::HostSelectedExtensionOwner;
use crate::RepositoryEnvironmentEntry;
use crate::RepositoryEnvironmentSnapshot;
use crate::SelectedExtensionDownloadVerified;

/// A fresh recorder for one evaluator pass. Need discards it completely.
#[derive(Debug, Allocative)]
pub struct SelectedScratchPrefixBuilder {
    origin: BudgetedImageBlock<Origin>,
    operations: Vec<SelectedScratchOperation>,
    entries: Vec<SelectedScratchEntry>,
    // Drop vectors before returning their capacity/replacement credit.
    reservation: ImageDescriptorReservation,
    occurrence_bytes: usize,
    descriptor_bytes: usize,
    failed: Option<SelectedScratchError>,
}

fn add(
    left: usize,
    right: usize,
    limit: usize,
    kind: SelectedScratchLimit,
) -> Result<usize, SelectedScratchError> {
    left.checked_add(right)
        .filter(|v| *v <= limit)
        .ok_or(SelectedScratchError::LimitExceeded(kind))
}
fn slot<T>(
    values: &mut Vec<T>,
    credit: &mut ImageDescriptorReservation,
    ceiling: usize,
) -> Result<(), SelectedScratchError> {
    if values.len() == values.capacity() {
        let next = values.capacity().saturating_mul(2).max(4).min(ceiling);
        if next <= values.len() {
            return Err(SelectedScratchError::Allocation);
        }
        credit.try_grow(
            next.checked_mul(size_of::<T>())
                .ok_or(SelectedScratchError::Allocation)?,
        )?;
        values
            .try_reserve_exact(next - values.len())
            .map_err(|_| SelectedScratchError::Allocation)?;
    }
    Ok(())
}

impl SelectedScratchPrefixBuilder {
    pub fn new(
        workspace: ImageChargedPath,
        owner: Arc<HostSelectedExtensionOwner>,
        policy: SelectedScratchPolicy,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedScratchError> {
        if !workspace.same_budget(resources.descriptor_budget()) {
            return Err(SelectedScratchError::ForeignResources);
        }
        if GUEST_ROOT.len() > policy.limits().max_descriptor_bytes {
            return Err(SelectedScratchError::LimitExceeded(
                SelectedScratchLimit::DescriptorBytes,
            ));
        }
        let origin = resources
            .descriptor_budget()
            .try_reserve(0)?
            .freeze(Origin {
                workspace,
                owner,
                policy,
                resources: resources.dupe(),
            })?;
        let mut builder = Self {
            origin,
            operations: Vec::new(),
            entries: Vec::new(),
            reservation: resources.descriptor_budget().try_reserve(0)?,
            occurrence_bytes: 0,
            descriptor_bytes: GUEST_ROOT.len(),
            failed: None,
        };
        let root = builder.make_path("")?;
        slot(
            &mut builder.entries,
            &mut builder.reservation,
            policy.limits().max_entries,
        )?;
        builder
            .entries
            .push(SelectedScratchEntry::Directory { path: root });
        Ok(builder)
    }
    fn resources(&self) -> &SelectedScratchResources {
        &self.origin.get().resources
    }
    fn limits(&self) -> &SelectedScratchLimits {
        self.origin.get().policy.limits()
    }
    fn remember<T>(
        &mut self,
        result: Result<T, SelectedScratchError>,
    ) -> Result<T, SelectedScratchError> {
        if let Err(error) = result {
            self.failed.get_or_insert(error);
        }
        result
    }
    fn ready(&self) -> Result<(), SelectedScratchError> {
        self.failed.map_or(Ok(()), Err)
    }
    fn own_path(&self, path: &SelectedScratchPath) -> Result<(), SelectedScratchError> {
        if !std::ptr::eq(path.0.get().origin.get(), self.origin.get()) {
            return Err(SelectedScratchError::ForeignOrigin);
        }
        Ok(())
    }
    fn own_external(&self, path: &SelectedScratchExternalPath) -> Result<(), SelectedScratchError> {
        if !std::ptr::eq(path.0.get().origin.get(), self.origin.get()) {
            return Err(SelectedScratchError::ForeignOrigin);
        }
        match self.operations.get(path.event_index()) {
            Some(SelectedScratchOperation::ExternalPath { path: actual }) if actual == path => {
                Ok(())
            }
            _ => Err(SelectedScratchError::ForeignOrigin),
        }
    }
    fn descriptor(&self, extra: usize) -> Result<usize, SelectedScratchError> {
        add(
            self.descriptor_bytes,
            extra,
            self.limits().max_descriptor_bytes,
            SelectedScratchLimit::DescriptorBytes,
        )
    }
    fn event(&mut self, extra: usize) -> Result<(), SelectedScratchError> {
        self.ready()?;
        add(
            self.operations.len(),
            1,
            self.limits().max_operations,
            SelectedScratchLimit::Operations,
        )?;
        let descriptor = self.descriptor(extra)?;
        let ceiling = self.limits().max_operations;
        slot(&mut self.operations, &mut self.reservation, ceiling)?;
        self.descriptor_bytes = descriptor;
        Ok(())
    }
    fn check_relative(&self, relative: &str) -> Result<(), SelectedScratchError> {
        if !relative.is_empty()
            && (relative.contains(['\\', '\0'])
                || relative.starts_with('/')
                || relative
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || relative.split('/').next().is_some_and(|p| p.ends_with(':')))
        {
            return Err(SelectedScratchError::InvalidPath);
        }
        let depth = if relative.is_empty() {
            0
        } else {
            relative.bytes().filter(|b| *b == b'/').count() + 1
        };
        if depth > self.limits().max_depth {
            return Err(SelectedScratchError::LimitExceeded(
                SelectedScratchLimit::Depth,
            ));
        }
        self.check_spelling(GUEST_ROOT.len() + usize::from(!relative.is_empty()) + relative.len())
    }
    fn check_spelling(&self, length: usize) -> Result<(), SelectedScratchError> {
        if length > self.limits().max_path_bytes {
            return Err(SelectedScratchError::LimitExceeded(
                SelectedScratchLimit::PathBytes,
            ));
        }
        Ok(())
    }
    fn text(&self, value: &str) -> Result<SelectedScratchText, SelectedScratchError> {
        let reservation = self
            .resources()
            .descriptor_budget()
            .try_reserve(value.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| SelectedScratchError::Allocation)?;
        owned.push_str(value);
        Ok(SelectedScratchText(reservation.freeze(owned)?))
    }
    fn make_path(&self, relative: &str) -> Result<SelectedScratchPath, SelectedScratchError> {
        self.check_relative(relative)?;
        let length = GUEST_ROOT.len() + usize::from(!relative.is_empty()) + relative.len();
        let reservation = self
            .resources()
            .descriptor_budget()
            .try_reserve(relative.len() + length)?;
        let mut stored = String::new();
        stored
            .try_reserve_exact(relative.len())
            .map_err(|_| SelectedScratchError::Allocation)?;
        stored.push_str(relative);
        let mut guest = String::new();
        guest
            .try_reserve_exact(length)
            .map_err(|_| SelectedScratchError::Allocation)?;
        guest.push_str(GUEST_ROOT);
        if !relative.is_empty() {
            guest.push('/');
            guest.push_str(relative);
        }
        Ok(SelectedScratchPath(reservation.freeze(PathInner {
            origin: self.origin.dupe(),
            relative: stored,
            guest,
        })?))
    }
    pub fn path(&mut self, literal: &str) -> Result<SelectedScratchPath, SelectedScratchError> {
        let result = (|| {
            let relative = if literal == "." { "" } else { literal };
            self.check_relative(relative)?;
            let result_length =
                GUEST_ROOT.len() + usize::from(!relative.is_empty()) + relative.len();
            self.event(literal.len() + result_length)?;
            let path = self.make_path(relative)?;
            let literal = self.text(literal)?;
            self.operations.push(SelectedScratchOperation::Path {
                literal,
                result: path.dupe(),
            });
            Ok(path)
        })();
        self.remember(result)
    }
    pub fn parent(
        &mut self,
        path: &SelectedScratchPath,
    ) -> Result<SelectedScratchPath, SelectedScratchError> {
        let result = (|| {
            self.own_path(path)?;
            let relative = path.relative().rsplit_once('/').map_or("", |v| v.0);
            let length = GUEST_ROOT.len() + usize::from(!relative.is_empty()) + relative.len();
            self.event(path.guest_spelling().len() + length)?;
            let result = self.make_path(relative)?;
            self.operations.push(SelectedScratchOperation::Parent {
                from: path.dupe(),
                result: result.dupe(),
            });
            Ok(result)
        })();
        self.remember(result)
    }
    pub fn child(
        &mut self,
        path: &SelectedScratchPath,
        literal: &str,
    ) -> Result<SelectedScratchPath, SelectedScratchError> {
        let result = (|| {
            self.own_path(path)?;
            if literal.is_empty() {
                return Err(SelectedScratchError::InvalidPath);
            }
            self.check_relative(literal)?;
            let depth = |s: &str| {
                if s.is_empty() {
                    0
                } else {
                    s.bytes().filter(|b| *b == b'/').count() + 1
                }
            };
            if depth(path.relative()) + depth(literal) > self.limits().max_depth {
                return Err(SelectedScratchError::LimitExceeded(
                    SelectedScratchLimit::Depth,
                ));
            }
            let length =
                path.relative().len() + usize::from(!path.relative().is_empty()) + literal.len();
            self.check_spelling(GUEST_ROOT.len() + 1 + length)?;
            self.event(
                path.guest_spelling().len() + literal.len() + GUEST_ROOT.len() + 1 + length,
            )?;
            // Reserve the temporary join peak before allocating, retaining it until join drops.
            let _scratch = self.resources().descriptor_budget().try_reserve(length)?;
            let mut joined = String::new();
            joined
                .try_reserve_exact(length)
                .map_err(|_| SelectedScratchError::Allocation)?;
            joined.push_str(path.relative());
            if !joined.is_empty() {
                joined.push('/');
            }
            joined.push_str(literal);
            let result = self.make_path(&joined)?;
            let literal = self.text(literal)?;
            self.operations.push(SelectedScratchOperation::Child {
                from: path.dupe(),
                literal,
                result: result.dupe(),
            });
            Ok(result)
        })();
        self.remember(result)
    }
    pub fn record_external_path(
        &mut self,
        spelling: &str,
    ) -> Result<SelectedScratchExternalPath, SelectedScratchError> {
        let result = (|| {
            if spelling.contains('\0') {
                return Err(SelectedScratchError::InvalidPath);
            }
            self.check_spelling(spelling.len())?;
            self.event(spelling.len())?;
            let reservation = self.resources().descriptor_budget().try_reserve(0)?;
            let path = SelectedScratchExternalPath(reservation.freeze(ExternalInner {
                origin: self.origin.dupe(),
                event_index: self.operations.len(),
                spelling: self.text(spelling)?,
            })?);
            self.operations
                .push(SelectedScratchOperation::ExternalPath { path: path.dupe() });
            Ok(path)
        })();
        self.remember(result)
    }
    fn entry(&self, relative: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.path().relative() == relative)
    }
    fn admit_write(
        &mut self,
        path: &SelectedScratchPath,
        bytes: usize,
        extra: usize,
    ) -> Result<(), SelectedScratchError> {
        self.own_path(path)?;
        if bytes > self.limits().max_file_bytes {
            return Err(SelectedScratchError::LimitExceeded(
                SelectedScratchLimit::FileBytes,
            ));
        }
        let occurrence = add(
            self.occurrence_bytes,
            bytes,
            self.limits().max_occurrence_bytes,
            SelectedScratchLimit::OccurrenceBytes,
        )?;
        let mut missing = usize::from(self.entry(path.relative()).is_none());
        let mut descriptors = extra;
        if self
            .entry(path.relative())
            .is_some_and(|i| matches!(self.entries[i], SelectedScratchEntry::Directory { .. }))
        {
            return Err(SelectedScratchError::WrongKind);
        }
        for (index, _) in path.relative().match_indices('/') {
            let parent = &path.relative()[..index];
            match self.entry(parent) {
                Some(i) if matches!(self.entries[i], SelectedScratchEntry::File { .. }) => {
                    return Err(SelectedScratchError::WrongKind);
                }
                Some(_) => {}
                None => {
                    missing += 1;
                    descriptors += GUEST_ROOT.len() + 1 + parent.len();
                }
            }
        }
        add(
            self.entries.len(),
            missing,
            self.limits().max_entries,
            SelectedScratchLimit::Entries,
        )?;
        self.event(descriptors)?;
        self.occurrence_bytes = occurrence;
        Ok(())
    }
    fn install(
        &mut self,
        path: &SelectedScratchPath,
        blob: &SelectedScratchBlob,
        executable: bool,
    ) -> Result<(), SelectedScratchError> {
        let ceiling = self.limits().max_entries;
        for (index, _) in path.relative().match_indices('/') {
            let parent = &path.relative()[..index];
            if self.entry(parent).is_none() {
                let parent = self.make_path(parent)?;
                slot(&mut self.entries, &mut self.reservation, ceiling)?;
                self.entries
                    .push(SelectedScratchEntry::Directory { path: parent });
            }
        }
        let entry = SelectedScratchEntry::File {
            path: path.dupe(),
            blob: blob.dupe(),
            executable,
        };
        if let Some(index) = self.entry(path.relative()) {
            self.entries[index] = entry;
        } else {
            slot(&mut self.entries, &mut self.reservation, ceiling)?;
            self.entries.push(entry);
        }
        Ok(())
    }
    pub fn file(
        &mut self,
        path: &SelectedScratchPath,
        bytes: &[u8],
        executable: bool,
    ) -> Result<(), SelectedScratchError> {
        let result = (|| {
            self.admit_write(path, bytes.len(), path.guest_spelling().len())?;
            let descriptor = self.resources().descriptor_budget().try_reserve(0)?;
            let payload = self.resources().payload_budget().reserve(bytes.len())?;
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| SelectedScratchError::Allocation)?;
            owned.extend_from_slice(bytes);
            let blob = SelectedScratchBlob(descriptor.freeze(BlobInner {
                storage: BlobStorage::Owned(owned),
                payload,
            })?);
            self.install(path, &blob, executable)?;
            self.operations.push(SelectedScratchOperation::File {
                path: path.dupe(),
                blob,
                executable,
            });
            Ok(())
        })();
        self.remember(result)
    }
    pub fn download(
        &mut self,
        path: &SelectedScratchPath,
        verified: &Arc<SelectedExtensionDownloadVerified>,
    ) -> Result<(), SelectedScratchError> {
        let result = (|| {
            let request = verified.request();
            if request.workspace() != self.origin.get().workspace.normalized_ref()
                || request.owner() != &self.origin.get().owner
                || request.output().as_path().to_str() != Some(path.relative())
            {
                return Err(SelectedScratchError::DownloadMismatch);
            }
            if self.operations.iter().any(|op| matches!(op, SelectedScratchOperation::Download {blob, ..} if blob.download().unwrap().request() != request)) {
                return Err(SelectedScratchError::Unsupported(SelectedScratchUnsupported::DistinctDownloads));
            }
            self.admit_write(
                path,
                verified.bytes().len(),
                path.guest_spelling().len() + request.url().len() + 64,
            )?;
            let existing = self.operations.iter().find_map(|op| match op {
                SelectedScratchOperation::Download { blob, .. }
                    if Arc::ptr_eq(blob.download().unwrap(), verified) =>
                {
                    Some(blob.dupe())
                }
                _ => None,
            });
            let blob = match existing {
                Some(blob) => blob,
                None => {
                    let descriptor = self.resources().descriptor_budget().try_reserve(0)?;
                    let payload = self
                        .resources()
                        .payload_budget()
                        .reserve(verified.bytes().len())?;
                    SelectedScratchBlob(descriptor.freeze(BlobInner {
                        storage: BlobStorage::Download(verified.dupe()),
                        payload,
                    })?)
                }
            };
            self.install(path, &blob, request.executable())?;
            self.operations.push(SelectedScratchOperation::Download {
                path: path.dupe(),
                blob,
            });
            Ok(())
        })();
        self.remember(result)
    }

    pub fn finish(
        mut self,
        call: SelectedScratchProcessSpec<'_>,
        inherited: &RepositoryEnvironmentSnapshot,
    ) -> Result<SelectedScratchPrefix, SelectedScratchError> {
        self.ready()?;
        if call.arguments.is_empty()
            || call.timeout_seconds == 0
            || !(call.cwd == GUEST_ROOT || call.cwd.starts_with("/__slug_module/"))
        {
            return Err(SelectedScratchError::InvalidArgument);
        }
        if call.cwd.ends_with('/') {
            return Err(SelectedScratchError::InvalidArgument);
        }
        self.check_relative(
            call.cwd
                .strip_prefix(GUEST_ROOT)
                .unwrap()
                .strip_prefix('/')
                .unwrap_or(""),
        )?;
        add(
            0,
            call.arguments.len(),
            self.limits().max_argv_entries,
            SelectedScratchLimit::ArgvEntries,
        )?;
        add(
            inherited.len(),
            call.environment.len(),
            self.limits().max_environment_entries,
            SelectedScratchLimit::EnvironmentEntries,
        )?;
        let mut argv_bytes = 0;
        for arg in call.arguments {
            let spelling = match arg {
                SelectedScratchOperandSpec::Literal(s) => *s,
                SelectedScratchOperandSpec::Scratch(p) => {
                    self.own_path(p)?;
                    p.guest_spelling()
                }
                SelectedScratchOperandSpec::ExternalPath(p) => {
                    self.own_external(p)?;
                    p.spelling()
                }
            };
            if spelling.contains('\0') {
                return Err(SelectedScratchError::InvalidArgument);
            }
            argv_bytes = add(
                argv_bytes,
                spelling.len(),
                self.limits().max_argv_bytes,
                SelectedScratchLimit::ArgvBytes,
            )?;
        }
        let mut environment_bytes = 0;
        for entry in inherited.iter() {
            environment_bytes = add(
                environment_bytes,
                entry.name().len() + entry.value().len(),
                self.limits().max_environment_bytes,
                SelectedScratchLimit::EnvironmentBytes,
            )?;
        }
        let inherited_bytes = environment_bytes;
        for (name, value) in call.environment {
            if name.is_empty()
                || name.contains(['=', '\0'])
                || value.is_some_and(|v| v.contains('\0'))
            {
                return Err(SelectedScratchError::InvalidArgument);
            }
            environment_bytes = add(
                environment_bytes,
                name.len() + value.map_or(0, str::len),
                self.limits().max_environment_bytes,
                SelectedScratchLimit::EnvironmentBytes,
            )?;
        }
        self.event(argv_bytes + environment_bytes + call.cwd.len())?;
        let mut reservation = self.resources().descriptor_budget().try_reserve(
            inherited_bytes + inherited.len() * size_of::<RepositoryEnvironmentEntry>(),
        )?;
        let mut arguments = Vec::new();
        for arg in call.arguments {
            let arg = match arg {
                SelectedScratchOperandSpec::Literal(s) => {
                    SelectedScratchOperand::Literal(self.text(s)?)
                }
                SelectedScratchOperandSpec::Scratch(p) => {
                    SelectedScratchOperand::Scratch((*p).dupe())
                }
                SelectedScratchOperandSpec::ExternalPath(p) => {
                    SelectedScratchOperand::ExternalPath((*p).dupe())
                }
            };
            slot(
                &mut arguments,
                &mut reservation,
                self.limits().max_argv_entries,
            )?;
            arguments.push(arg);
        }
        let mut environment = Vec::new();
        for (name, value) in call.environment {
            let change = SelectedScratchEnvironmentChange {
                name: self.text(name)?,
                value: value.map(|s| self.text(s)).transpose()?,
            };
            slot(
                &mut environment,
                &mut reservation,
                self.limits().max_environment_entries,
            )?;
            environment.push(change);
        }
        let process = SelectedScratchProcessCall(reservation.freeze(ProcessInner {
            arguments,
            cwd: self.text(call.cwd)?,
            environment,
            inherited: inherited.dupe(),
            timeout: call.timeout_seconds,
            quiet: call.quiet,
        })?);
        self.entries
            .sort_unstable_by(|a, b| a.path().relative().cmp(b.path().relative()));
        Ok(SelectedScratchPrefix(self.reservation.freeze(
            PrefixInner {
                origin: self.origin,
                operations: self.operations,
                entries: self.entries,
                process,
                occurrence_bytes: self.occurrence_bytes,
                descriptor_bytes: self.descriptor_bytes,
            },
        )?))
    }
}
