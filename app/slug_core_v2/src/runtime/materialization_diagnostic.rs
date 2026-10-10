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

// This view never increments the request or attribute Arc counts.
pub(super) struct ImmutableSpecDiagnostic<'a> {
    pub(super) request: &'a slug_bzlmod_v2::RepositoryMaterializationRequest,
}
struct DiagnosticSink;
impl fmt::Write for DiagnosticSink {
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }
}
struct ImmutableSpecWriter<'a, W> {
    writer: &'a mut W,
    raw: usize,
    nodes: usize,
}
impl<W: fmt::Write> ImmutableSpecWriter<'_, W> {
    fn bytes(&mut self, bytes: Option<&[u8]>) -> fmt::Result {
        let Some(bytes) = bytes else {
            return self.writer.write_str("null");
        };
        self.raw = self
            .raw
            .checked_add(bytes.len())
            .filter(|sum| {
                *sum <= MATERIALIZATION_AGGREGATE_BYTES
                    && bytes.len() <= MATERIALIZATION_FIELD_BYTES
            })
            .ok_or(fmt::Error)?;
        write!(self.writer, "[{},\"", bytes.len())?;
        if !bytes.is_empty() {
            write!(self.writer, "{}", BorrowedHex(Some(bytes)))?;
        }
        self.writer.write_str("\"]")
    }
    fn label(&mut self, label: &slug_identity_v2::CanonicalLabel) -> fmt::Result {
        self.writer.write_str("[\"l\",")?;
        for bytes in [
            label.package().repo().as_str().as_bytes(),
            label.package().package().as_str().as_bytes(),
            label.target().as_str().as_bytes(),
        ] {
            self.bytes(Some(bytes))?;
            self.writer.write_str(",")?;
        }
        self.bytes(label.mapping_id().map(|id| id.as_str().as_bytes()))?;
        self.writer.write_str("]")
    }
    fn node(&mut self, depth: usize) -> fmt::Result {
        self.nodes = self
            .nodes
            .checked_add(1)
            .filter(|n| *n <= 128 && depth <= 8)
            .ok_or(fmt::Error)?;
        Ok(())
    }
    fn string(&mut self, value: &str) -> fmt::Result {
        self.writer.write_str("[\"s\",")?;
        self.bytes(Some(value.as_bytes()))?;
        self.writer.write_str("]")
    }
    fn key(&mut self, key: &slug_bzlmod_v2::OverrideAttributeKey, depth: usize) -> fmt::Result {
        self.node(depth)?;
        match key {
            slug_bzlmod_v2::OverrideAttributeKey::String(value) => self.string(value.as_str()),
            slug_bzlmod_v2::OverrideAttributeKey::Label(value) => self.label(value),
        }
    }
    fn value(
        &mut self,
        value: &slug_bzlmod_v2::OverrideAttributeValue,
        depth: usize,
    ) -> fmt::Result {
        use slug_bzlmod_v2::OverrideAttributeValue as V;
        self.node(depth)?;
        let child_depth = depth.checked_add(1).ok_or(fmt::Error)?;
        match value {
            V::None => self.writer.write_str("[\"n\"]"),
            V::Bool(value) => write!(self.writer, "[\"b\",{value}]"),
            V::Int(value) => write!(self.writer, "[\"i\",{value}]"),
            V::String(value) => self.string(value.as_str()),
            V::Label(value) => self.label(value),
            V::Iterable(values) => {
                if values.len() > 128 - self.nodes {
                    return Err(fmt::Error);
                }
                self.writer.write_str("[\"v\",[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.writer.write_str(",")?;
                    }
                    self.value(value, child_depth)?;
                }
                self.writer.write_str("]]")
            }
            V::Map(values) => {
                if values.len().checked_mul(2).ok_or(fmt::Error)? > 128 - self.nodes {
                    return Err(fmt::Error);
                }
                self.writer.write_str("[\"m\",[")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index != 0 {
                        self.writer.write_str(",")?;
                    }
                    self.writer.write_str("[")?;
                    self.key(key, child_depth)?;
                    self.writer.write_str(",")?;
                    self.value(value, child_depth)?;
                    self.writer.write_str("]")?;
                }
                self.writer.write_str("]]")
            }
        }
    }
}
impl ImmutableSpecDiagnostic<'_> {
    fn write_captured(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        let attributes = &self.request.repo_spec.attributes;
        if attributes.len() > 32 {
            return Err(fmt::Error);
        }
        let mut out = ImmutableSpecWriter {
            writer,
            raw: 0,
            nodes: 0,
        };
        out.writer.write_str("WP1079_IMMUTABLE_SPEC {\"capture\":\"captured\",\"kind\":\"immutable\",\"reason\":\"immutable_kind\",\"identity\":[")?;
        for (index, bytes) in MaterializationDiagnostic::new(self.request, "immutable_kind")
            .fields
            .into_iter()
            .enumerate()
        {
            if index != 0 {
                out.writer.write_str(",")?;
            }
            out.bytes(bytes)?;
        }
        out.writer.write_str("],\"rule_mapping\":")?;
        out.bytes(
            self.request
                .repo_spec
                .rule_id
                .bzl_file
                .mapping_id()
                .map(|id| id.as_str().as_bytes()),
        )?;
        out.writer.write_str(",\"attributes\":[")?;
        for (index, (name, value)) in attributes.iter().enumerate() {
            if index != 0 {
                out.writer.write_str(",")?;
            }
            out.writer.write_str("[")?;
            out.bytes(Some(name.as_bytes()))?;
            out.writer.write_str(",")?;
            out.value(value, 1)?;
            out.writer.write_str("]")?;
        }
        out.writer.write_str("]}")
    }
}
impl fmt::Display for ImmutableSpecDiagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let captured = self
            .write_captured(&mut MaterializationWriter {
                writer: &mut DiagnosticSink,
                bytes: 0,
                limit: MATERIALIZATION_MARKER_BYTES,
            })
            .is_ok();
        let mut writer = MaterializationWriter {
            writer: formatter,
            bytes: 0,
            limit: MATERIALIZATION_MARKER_BYTES,
        };
        if captured {
            self.write_captured(&mut writer)
        } else {
            fmt::Write::write_str(
                &mut writer,
                "WP1079_IMMUTABLE_SPEC {\"capture\":\"bounds_refused\",\"kind\":\"immutable\",\"reason\":\"immutable_kind\",\"identity\":null,\"rule_mapping\":null,\"attributes\":null}",
            )
        }
    }
}
