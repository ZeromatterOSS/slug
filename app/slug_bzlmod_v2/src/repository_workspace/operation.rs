use starlark_map::small_map::SmallMap;

use super::*;
use crate::GeneratedRepositoryArchiveEffect;
use crate::HostRepositoryLabelPathValue;
use crate::RepositoryEnvironmentSnapshot;
use crate::RepositoryExecuteResult;
use crate::RepositoryLabelPathAddress;

/// Every variant is allowed in every argv position. Path and Label retain their
/// tracked resolution provenance; this type does not perform source resolution.
#[derive(Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryWorkspaceArgument {
    String(CompactString),
    Path(HostRepositoryLabelPathValue),
    Label {
        address: RepositoryLabelPathAddress,
        resolved: HostRepositoryLabelPathValue,
    },
}
impl fmt::Debug for RepositoryWorkspaceArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::String(_) => "String(<redacted>)",
            Self::Path(_) => "Path(<redacted>)",
            Self::Label { .. } => "Label(<redacted>)",
        })
    }
}
impl RepositoryWorkspaceArgument {
    pub fn text(&self) -> &str {
        match self {
            Self::String(s) => s,
            Self::Path(p) | Self::Label { resolved: p, .. } => p.path_str(),
        }
    }
    fn validate(&self) -> Result<(), RepositoryWorkspaceError> {
        if self.text().contains('\0') {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        }
        if let Self::Path(path) | Self::Label { resolved: path, .. } = self {
            if let slug_workspace_v2::PathObservationNamespace::Materialization(id) =
                path.namespace()
            {
                if id.value() == 0 {
                    return Err(RepositoryWorkspaceError::InvalidOperation);
                }
            }
        }
        Ok(())
    }
    fn metadata(&self) -> Result<usize, RepositoryWorkspaceError> {
        match self {
            Self::String(value) => Ok(value.capacity()),
            Self::Path(path) => path
                .path()
                .retained_storage_bytes()
                .ok_or(RepositoryWorkspaceError::RetentionLimit),
            Self::Label { address, resolved } => add(
                allocative::size_of_unique_allocated_data(address),
                resolved
                    .path()
                    .retained_storage_bytes()
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
            ),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryWorkspaceEnvironmentValue {
    Override(CompactString),
    Remove,
}
impl fmt::Debug for RepositoryWorkspaceEnvironmentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Override(_) => "Override(<redacted>)",
            Self::Remove => "Remove",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryWorkspaceExecute {
    argv: Arc<[RepositoryWorkspaceArgument]>,
    environment: RepositoryEnvironmentSnapshot,
    overrides: SmallMap<CompactString, RepositoryWorkspaceEnvironmentValue>,
    timeout_seconds: i32,
    quiet: bool,
    working_directory: HostRepositoryLabelPathValue,
    read_views: RepositoryWorkspaceReadViewManifest,
}
impl RepositoryWorkspaceExecute {
    /// Exact structural comparison with caller-bounded source-state equality.
    /// This read-only hook grants no native authority and changes no derived Eq.
    #[doc(hidden)]
    pub fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let Self {
            argv,
            environment,
            overrides,
            timeout_seconds,
            quiet,
            working_directory,
            read_views,
        } = self;
        let Self {
            argv: other_argv,
            environment: other_environment,
            overrides: other_overrides,
            timeout_seconds: other_timeout,
            quiet: other_quiet,
            working_directory: other_cwd,
            read_views: other_views,
        } = other;
        if argv != other_argv
            || environment != other_environment
            || overrides != other_overrides
            || timeout_seconds != other_timeout
            || quiet != other_quiet
            || working_directory != other_cwd
        {
            return Ok(false);
        }
        read_views.try_eq_with_states(other_views, compare)
    }
    /// Called after Loading validates Starlark types in pinned argv/timeout/env
    /// order and resolves cwd. Requested600 remains600, independently of the
    /// native30s operational cap. This constructor admits no runtime authority.
    pub fn new(
        argv: Arc<[RepositoryWorkspaceArgument]>,
        environment: RepositoryEnvironmentSnapshot,
        overrides: SmallMap<CompactString, RepositoryWorkspaceEnvironmentValue>,
        timeout_seconds: i32,
        quiet: bool,
        working_directory: HostRepositoryLabelPathValue,
        read_views: RepositoryWorkspaceReadViewManifest,
    ) -> Result<Self, RepositoryWorkspaceError> {
        if argv.is_empty() {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        }
        for arg in argv.iter() {
            arg.validate()?;
        }
        if timeout_seconds <= 0 {
            return Err(RepositoryWorkspaceError::InvalidOperation);
        }
        // Validate ALL names before inspecting values, matching the bound API.
        for (name, _) in overrides.iter() {
            if name.contains(['\0', '=']) {
                return Err(RepositoryWorkspaceError::InvalidOperation);
            }
        }
        for entry in environment.iter() {
            if entry.name().contains(['\0', '=']) || entry.value().contains('\0') {
                return Err(RepositoryWorkspaceError::InvalidOperation);
            }
        }
        for (_, value) in overrides.iter() {
            if let RepositoryWorkspaceEnvironmentValue::Override(value) = value {
                if value.contains('\0') {
                    return Err(RepositoryWorkspaceError::InvalidOperation);
                }
            }
        }
        RepositoryWorkspaceArgument::Path(working_directory.clone()).validate()?;
        Ok(Self {
            argv,
            environment,
            overrides,
            timeout_seconds,
            quiet,
            working_directory,
            read_views,
        })
    }
    pub fn argv(&self) -> &[RepositoryWorkspaceArgument] {
        &self.argv
    }
    pub fn environment(&self) -> &RepositoryEnvironmentSnapshot {
        &self.environment
    }
    pub fn overrides(&self) -> &SmallMap<CompactString, RepositoryWorkspaceEnvironmentValue> {
        &self.overrides
    }
    pub fn timeout_seconds(&self) -> i32 {
        self.timeout_seconds
    }
    pub fn quiet(&self) -> bool {
        self.quiet
    }
    pub fn working_directory(&self) -> &HostRepositoryLabelPathValue {
        &self.working_directory
    }
    pub fn read_views(&self) -> &RepositoryWorkspaceReadViewManifest {
        &self.read_views
    }
    /// Pure rebuild for native canonicalization. No capability is granted;
    /// callers must compare the complete old/new projection before acceptance.
    pub fn with_read_views(
        mut self,
        read_views: RepositoryWorkspaceReadViewManifest,
    ) -> Result<Self, RepositoryWorkspaceError> {
        read_views.owned_metadata_bytes()?;
        self.read_views = read_views;
        Ok(self)
    }
    pub fn effective_value(&self, name: &str) -> Option<&str> {
        match self.overrides.get(name) {
            Some(RepositoryWorkspaceEnvironmentValue::Remove) => None,
            Some(RepositoryWorkspaceEnvironmentValue::Override(value)) => Some(value),
            None => self.environment.get(name).map(|value| value.as_ref()),
        }
    }
    fn metadata(&self) -> Result<usize, RepositoryWorkspaceError> {
        let mut size = array_bytes::<RepositoryWorkspaceArgument>(self.argv.len())?;
        size = add(
            size,
            self.overrides
                .retained_storage_bytes_upper_bound()
                .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
        )?;
        for arg in self.argv.iter() {
            size = add(size, arg.metadata()?)?;
        }
        size = add(
            size,
            array_bytes::<crate::RepositoryEnvironmentEntry>(self.environment.len())?,
        )?;
        for entry in self.environment.iter() {
            size = add(
                size,
                add(
                    allocative::size_of_unique_allocated_data(entry),
                    array_bytes::<u8>(entry.value().len())?,
                )?,
            )?;
        }
        for (name, value) in self.overrides.iter() {
            size = add(size, name.capacity())?;
            if let RepositoryWorkspaceEnvironmentValue::Override(value) = value {
                size = add(size, value.capacity())?;
            }
        }
        add(
            add(
                size,
                self.working_directory
                    .path()
                    .retained_storage_bytes()
                    .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
            )?,
            self.read_views.owned_metadata_bytes()?,
        )
    }
}

/// Template carries the exact rendered effect PLUS literal source/substitution
/// inputs selected by Loading, so protocol equality cannot erase those inputs.
/// Read/Exists paths here are already resolved root-relative workspace paths;
/// Host read/watch observations keep their existing tracked source owner.
#[derive(Clone, Eq, Allocative)]
pub enum RepositoryWorkspaceOperation {
    File {
        path: CompactString,
        body: RepositoryWorkspaceBody,
        mode: u16,
    },
    Template {
        path: CompactString,
        body: RepositoryWorkspaceBody,
        mode: u16,
        source: HostRepositoryLabelPathValue,
        source_body: RepositoryWorkspaceBody,
        substitutions: SmallMap<CompactString, CompactString>,
    },
    Symlink {
        path: CompactString,
        target: NormalizedAbsolutePath,
        source: Option<RepositoryWorkspaceReadView>,
    },
    Read(CompactString),
    Exists(CompactString),
    Archive(GeneratedRepositoryArchiveEffect),
    Delete(CompactString),
    Execute(RepositoryWorkspaceExecute),
}
impl PartialEq for RepositoryWorkspaceOperation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::File { path, body, mode },
                Self::File {
                    path: other_path,
                    body: other_body,
                    mode: other_mode,
                },
            ) => path == other_path && body == other_body && mode == other_mode,
            (
                Self::Template {
                    path,
                    body,
                    mode,
                    source,
                    source_body,
                    substitutions,
                },
                Self::Template {
                    path: other_path,
                    body: other_body,
                    mode: other_mode,
                    source: other_source,
                    source_body: other_source_body,
                    substitutions: other_substitutions,
                },
            ) => {
                path == other_path
                    && body == other_body
                    && mode == other_mode
                    && source == other_source
                    && source_body == other_source_body
                    // Literal replacement order is an input even when the
                    // selected source happens to render equal output bytes.
                    && substitutions.eq_ordered(other_substitutions)
            }
            (
                Self::Symlink {
                    path,
                    target,
                    source,
                },
                Self::Symlink {
                    path: p,
                    target: t,
                    source: s,
                },
            ) => path == p && target == t && source == s,
            (Self::Read(path), Self::Read(other))
            | (Self::Exists(path), Self::Exists(other))
            | (Self::Delete(path), Self::Delete(other)) => path == other,
            (Self::Archive(archive), Self::Archive(other)) => archive == other,
            (Self::Execute(execute), Self::Execute(other)) => execute == other,
            _ => false,
        }
    }
}
impl fmt::Debug for RepositoryWorkspaceOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::File { .. } => "File(<redacted>)",
            Self::Template { .. } => "Template(<redacted>)",
            Self::Symlink { .. } => "Symlink(<redacted>)",
            Self::Read(_) => "Read(<redacted>)",
            Self::Exists(_) => "Exists(<redacted>)",
            Self::Archive(_) => "Archive(<redacted>)",
            Self::Delete(_) => "Delete(<redacted>)",
            Self::Execute(_) => "Execute(<redacted>)",
        })
    }
}
impl RepositoryWorkspaceOperation {
    pub(super) fn try_eq_with_states<E>(
        &self,
        other: &Self,
        compare: &mut impl FnMut(
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
            crate::RepositoryWorkspaceReadViewStateRef<'_>,
        ) -> Result<bool, E>,
    ) -> Result<bool, E> {
        match (self, other) {
            (Self::Execute(left), Self::Execute(right)) => left.try_eq_with_states(right, compare),
            (
                Self::Symlink {
                    path,
                    target,
                    source,
                },
                Self::Symlink {
                    path: p,
                    target: t,
                    source: s,
                },
            ) => {
                if path != p || target != t {
                    return Ok(false);
                }
                match (source, s) {
                    (None, None) => Ok(true),
                    (Some(a), Some(b)) => a.try_eq_with_states(b, compare),
                    _ => Ok(false),
                }
            }
            _ => Ok(self == other),
        }
    }
    pub(super) fn validate(&self) -> Result<(), RepositoryWorkspaceError> {
        match self {
            Self::File { path, mode, .. } | Self::Template { path, mode, .. } => {
                if !valid_relative(path, false) || mode & !0o7777 != 0 {
                    return Err(RepositoryWorkspaceError::InvalidOperation);
                }
            }
            Self::Symlink {
                path,
                target,
                source,
            } => {
                if !valid_relative(path, false)
                    || target
                        .as_path()
                        .to_str()
                        .is_none_or(|s| s.len() > 4096 || s.contains(['\0', '\\']))
                    || source.as_ref().is_some_and(|v| v.source_state().is_none())
                {
                    return Err(RepositoryWorkspaceError::InvalidOperation);
                }
            }
            Self::Read(path) | Self::Exists(path) => {
                if !valid_relative(path, true) {
                    return Err(RepositoryWorkspaceError::InvalidOperation);
                }
            }
            Self::Delete(path) => {
                if !crate::generated_repository_file_effect::valid_delete_path(path) {
                    return Err(RepositoryWorkspaceError::InvalidOperation);
                }
            }
            Self::Archive(_) | Self::Execute(_) => {}
        }
        Ok(())
    }
    pub(super) fn visit_read_views(&self, visit: &mut impl FnMut(&RepositoryWorkspaceReadView)) {
        if let Self::Symlink {
            source: Some(source),
            ..
        } = self
        {
            visit(source);
        }
        if let Self::Execute(execute) = self {
            for view in execute.read_views.views() {
                visit(view)
            }
        }
    }
    pub(super) fn direct_source_candidates(
        &self,
    ) -> impl Iterator<Item = &RepositoryWorkspaceReadView> {
        let views: &[RepositoryWorkspaceReadView] = match self {
            Self::Execute(execute) => execute.read_views().views(),
            Self::Symlink {
                source: Some(view), ..
            } => std::slice::from_ref(view),
            _ => &[],
        };
        views.iter().filter(|view| view.source_state().is_some())
    }
    pub(super) fn bodies(&self, visit: &mut impl FnMut(&RepositoryWorkspaceBody)) {
        match self {
            Self::File { body, .. } => visit(body),
            Self::Template {
                body, source_body, ..
            } => {
                visit(body);
                visit(source_body);
            }
            _ => {}
        }
    }
    pub(super) fn bodies_mut(
        &mut self,
        visit: &mut impl FnMut(&mut RepositoryWorkspaceBody) -> Result<(), RepositoryWorkspaceError>,
    ) -> Result<(), RepositoryWorkspaceError> {
        match self {
            Self::File { body, .. } => visit(body)?,
            Self::Template {
                body, source_body, ..
            } => {
                visit(body)?;
                visit(source_body)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn metadata(&self) -> Result<usize, RepositoryWorkspaceError> {
        match self {
            Self::File { path, .. }
            | Self::Read(path)
            | Self::Exists(path)
            | Self::Delete(path) => Ok(path.capacity()),
            Self::Template {
                path,
                source,
                substitutions,
                ..
            } => {
                let mut size = add(
                    path.capacity(),
                    source
                        .path()
                        .retained_storage_bytes()
                        .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
                )?;
                size = add(
                    size,
                    substitutions
                        .retained_storage_bytes_upper_bound()
                        .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
                )?;
                for (key, value) in substitutions.iter() {
                    size = add(size, add(key.capacity(), value.capacity())?)?;
                }
                Ok(size)
            }
            Self::Symlink {
                path,
                target,
                source,
            } => {
                let mut bytes = add(
                    path.capacity(),
                    target
                        .retained_storage_bytes()
                        .ok_or(RepositoryWorkspaceError::RetentionLimit)?,
                )?;
                if let Some(view) = source {
                    bytes = add(bytes, view.storage_partition()?.owned_bytes())?;
                }
                Ok(bytes)
            }
            Self::Execute(execute) => execute.metadata(),
            Self::Archive(archive) => {
                let mut size = add(
                    add(
                        std::mem::size_of::<GeneratedRepositoryArchiveEffect>(),
                        allocative::size_of_unique_allocated_data(archive),
                    )?,
                    array_bytes::<CompactString>(archive.urls().len())?,
                )?;
                for url in archive.urls() {
                    size = add(size, url.capacity())?;
                }
                Ok(size)
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub enum RepositoryWorkspaceResult {
    Unit,
    Read(RepositoryWorkspaceBody),
    Exists(bool),
    Archive([u8; 32]),
    Delete(bool),
    Execute(RepositoryExecuteResult),
}
impl fmt::Debug for RepositoryWorkspaceResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(body) => f.debug_tuple("ReadBytes").field(&body.len()).finish(),
            Self::Execute(result) => fmt::Debug::fmt(result, f),
            Self::Unit => f.write_str("Unit"),
            Self::Exists(value) => f.debug_tuple("Exists").field(value).finish(),
            Self::Delete(value) => f.debug_tuple("Delete").field(value).finish(),
            Self::Archive(digest) => f.debug_tuple("Archive").field(digest).finish(),
        }
    }
}
impl RepositoryWorkspaceResult {
    pub(super) fn bodies(&self, visit: &mut impl FnMut(&RepositoryWorkspaceBody)) {
        if let Self::Read(body) = self {
            visit(body);
        }
    }
    pub(super) fn body_mut(&mut self) -> Option<&mut RepositoryWorkspaceBody> {
        if let Self::Read(body) = self {
            Some(body)
        } else {
            None
        }
    }
    pub(super) fn metadata(&self) -> Result<usize, RepositoryWorkspaceError> {
        if let Self::Execute(result) = self {
            if !(0..=255).contains(&result.return_code())
                || result.stdout().len() > 1024 * 1024
                || result.stderr().len() > 1024 * 1024
            {
                return Err(RepositoryWorkspaceError::InvalidResult);
            }
            add(
                array_bytes::<u8>(result.stdout().len())?,
                array_bytes::<u8>(result.stderr().len())?,
            )
        } else {
            Ok(0)
        }
    }
}
