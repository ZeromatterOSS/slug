//! Shared borrowed test-only materialization identity formatting.
use std::fmt;
use std::os::unix::ffi::OsStrExt;

use slug_bzlmod_v2::RepositoryMaterializationKind;

// Same streaming byte grammar as the original RegistryHex formatter.
pub(super) struct BorrowedHex<'a>(pub(super) Option<&'a [u8]>);
impl fmt::Display for BorrowedHex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(bytes) = self.0 else {
            return formatter.write_str("none");
        };
        if bytes.is_empty() {
            return formatter.write_str("-");
        }
        for byte in bytes {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub(super) const MATERIALIZATION_FIELD_BYTES: usize = 1024;
pub(super) const MATERIALIZATION_AGGREGATE_BYTES: usize = 4096;
pub(super) const MATERIALIZATION_MARKER_BYTES: usize = 10240;
pub(super) struct MaterializationDiagnostic<'a> {
    pub(super) kind: &'static str,
    pub(super) reason: &'static str,
    pub(super) fields: [Option<&'a [u8]>; 7],
}
impl<'a> MaterializationDiagnostic<'a> {
    pub(super) fn new(
        request: &'a slug_bzlmod_v2::RepositoryMaterializationRequest,
        reason: &'static str,
    ) -> Self {
        let (kind, logical_root) = match &request.kind {
            RepositoryMaterializationKind::Local { logical_root } => {
                ("local", Some(logical_root.as_path().as_os_str().as_bytes()))
            }
            RepositoryMaterializationKind::Immutable => ("immutable", None),
            RepositoryMaterializationKind::GeneratedFileEffects(_) => {
                ("generated_file_effects", None)
            }
        };
        let label = &request.repo_spec.rule_id.bzl_file;
        Self {
            kind,
            reason,
            fields: [
                Some(request.id.workspace.as_path().as_os_str().as_bytes()),
                Some(request.id.canonical_repo.as_str().as_bytes()),
                Some(label.package().repo().as_str().as_bytes()),
                Some(label.package().package().as_str().as_bytes()),
                Some(label.target().as_str().as_bytes()),
                Some(request.repo_spec.rule_id.rule_name.as_bytes()),
                logical_root,
            ],
        }
    }
    pub(super) fn captured(&self) -> bool {
        self.fields
            .iter()
            .flatten()
            .try_fold(0usize, |sum, bytes| {
                (bytes.len() <= MATERIALIZATION_FIELD_BYTES).then_some(())?;
                sum.checked_add(bytes.len())
                    .filter(|sum| *sum <= MATERIALIZATION_AGGREGATE_BYTES)
            })
            .is_some()
    }
}
pub(super) struct MaterializationWriter<'a, W> {
    pub(super) writer: &'a mut W,
    pub(super) bytes: usize,
    pub(super) limit: usize,
}
impl<W: fmt::Write> fmt::Write for MaterializationWriter<'_, W> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .filter(|n| *n < self.limit)
            .ok_or(fmt::Error)?;
        self.writer.write_str(text)
    }
}
impl MaterializationDiagnostic<'_> {
    pub(super) fn write_marker(
        &self,
        formatter: &mut fmt::Formatter<'_>,
        marker: &str,
    ) -> fmt::Result {
        use fmt::Write;
        let captured = self.captured();
        let mut writer = MaterializationWriter {
            writer: formatter,
            bytes: 0,
            limit: MATERIALIZATION_MARKER_BYTES,
        };
        write!(
            writer,
            "{marker} capture={} kind={} reason={}",
            if captured {
                "captured"
            } else {
                "bounds_refused"
            },
            self.kind,
            self.reason
        )?;
        for (name, bytes) in [
            "workspace",
            "canonical",
            "rule_repo",
            "rule_package",
            "rule_target",
            "rule_name",
            "logical_root",
        ]
        .into_iter()
        .zip(self.fields)
        {
            match bytes {
                Some(bytes) => write!(
                    writer,
                    " {name}_len={} {name}_hex={}",
                    bytes.len(),
                    BorrowedHex(captured.then_some(bytes))
                )?,
                None => write!(writer, " {name}_len=none {name}_hex=none")?,
            }
        }
        Ok(())
    }
}
impl fmt::Display for MaterializationDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_marker(formatter, "WP982_MATERIALIZATION")
    }
}
