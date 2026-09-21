use super::*;
use crate::AnalysisArtifact;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcFeature {
    pub name: CompactString,
    pub enabled: bool,
    pub flag_sets: Box<[CcFlagSet]>,
    pub env_sets: Box<[CcEnvSet]>,
    pub requires: Box<[CcSet<CompactString>]>,
    pub implies: Box<[CompactString]>,
    pub provides: Box<[CompactString]>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcActionConfig {
    pub action_name: CompactString,
    pub enabled: bool,
    pub tools: Box<[CcTool]>,
    pub flag_sets: Box<[CcFlagSet]>,
    pub implies: Box<[CompactString]>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcFlagSet {
    pub actions: CcSet<CompactString>,
    pub with_features: CcSet<CcWithFeatureSet>,
    pub flag_groups: Box<[CcFlagGroup]>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcWithFeatureSet {
    pub features: CcSet<CompactString>,
    pub not_features: CcSet<CompactString>,
}
/// Native WithFeatureSet equality ignores each predicate set's order. Deduplicate
/// on that semantic comparison, retaining the first predicate's ordered payload
/// for complete publication identity (which is deliberately a separate relation).
pub fn cc_with_features_set(
    values: impl IntoIterator<Item = CcWithFeatureSet>,
) -> CcSet<CcWithFeatureSet> {
    let mut result = CcSet::new();
    for value in values {
        if !result.iter().any(|prior: &CcWithFeatureSet| {
            prior.features.0 == value.features.0 && prior.not_features.0 == value.not_features.0
        }) {
            result.insert(value);
        }
    }
    result
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcFlagGroup {
    pub expandables: Box<[CcExpandable]>,
    pub iterate_over: Option<CompactString>,
    pub expand_if_available: Option<CompactString>,
    pub expand_if_not_available: Option<CompactString>,
    pub expand_if_true: Option<CompactString>,
    pub expand_if_false: Option<CompactString>,
    pub expand_if_equal: Option<CcVariableWithValue>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum CcExpandable {
    Flag(Box<[CcStringChunk]>),
    Group(CcFlagGroup),
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcVariableWithValue {
    pub name: CompactString,
    pub value: CompactString,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcEnvSet {
    pub actions: CcSet<CompactString>,
    pub env_entries: Box<[CcEnvEntry]>,
    pub with_features: CcSet<CcWithFeatureSet>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcEnvEntry {
    pub key: CompactString,
    pub value: Box<[CcStringChunk]>,
    pub expand_if_available: Option<CompactString>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum CcStringChunk {
    Literal(CompactString),
    Variable(CompactString),
    RelativePath(CompactString),
}

/// Native StringValueParser chunk boundaries, including percent escapes and path chunks.
/// Error offsets use UTF-8 bytes (the explicit Slug-native valid-Unicode boundary).
pub fn parse_cc_string(value: &str) -> Result<Vec<CcStringChunk>, CcFeatureError> {
    let mut chunks = Vec::new();
    let mut current = 0;
    let bytes = value.as_bytes();
    while current < bytes.len() {
        if bytes[current] == b'%' && bytes.get(current + 1) != Some(&b'%') {
            current += 1;
            if bytes.get(current) != Some(&b'{') {
                return Err(invalid("expected '{' while parsing a flag"));
            }
            current += 1;
            let end = value[current..]
                .find('}')
                .map(|offset| current + offset)
                .ok_or_else(|| invalid("expected '}' while parsing a flag"))?;
            if end == current {
                return Err(invalid("expected variable name while parsing a flag"));
            }
            let name = &value[current..end];
            if let Some(path) = name.strip_prefix("path:") {
                if path.is_empty() {
                    return Err(invalid("expected path after 'path:'"));
                }
                let path = normalize_posix_path(path)?;
                if path.starts_with('/') {
                    return Err(invalid("expected relative Unix-style path after 'path:'"));
                }
                chunks.push(CcStringChunk::RelativePath(path));
            } else {
                chunks.push(CcStringChunk::Variable(name.into()));
            }
            current = end + 1;
        } else {
            if bytes[current] == b'%' {
                current += 1;
            }
            let start = current;
            // The next byte is safe to search as raw bytes; only '%' ends a chunk.
            current = bytes[current + 1..]
                .iter()
                .position(|b| *b == b'%')
                .map_or(bytes.len(), |offset| start + 1 + offset);
            chunks.push(CcStringChunk::Literal(value[start..current].into()));
        }
    }
    Ok(chunks)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum CcExecOs {
    Linux,
    Darwin,
    Freebsd,
    Openbsd,
}
impl CcExecOs {
    pub fn parse(value: &str) -> Result<Self, CcFeatureError> {
        match value {
            "LINUX" => Ok(Self::Linux),
            "DARWIN" => Ok(Self::Darwin),
            "FREEBSD" => Ok(Self::Freebsd),
            "OPENBSD" => Ok(Self::Openbsd),
            _ => Err(invalid(format!(
                "unsupported C++ feature execution OS '{value}'"
            ))),
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum CcToolPathOrigin {
    FilesystemRoot,
    CrosstoolPackage,
    WorkspaceRoot,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcTool {
    path: CompactString,
    origin: CcToolPathOrigin,
    artifact: Option<AnalysisArtifact>,
    pub with_features: CcSet<CcWithFeatureSet>,
    pub execution_requirements: CcSet<CompactString>,
    exec_os: CcExecOs,
}
impl CcTool {
    pub fn new(
        path: Option<&str>,
        artifact: Option<AnalysisArtifact>,
        with_features: CcSet<CcWithFeatureSet>,
        execution_requirements: CcSet<CompactString>,
        exec_os: CcExecOs,
    ) -> Result<Self, CcFeatureError> {
        let (path, origin) = match (path, artifact.as_ref()) {
            (Some(path), None) => {
                let path = normalize_posix_path(path)?;
                if path.is_empty() {
                    return Err(invalid(
                        "The 'path' field of tool must be a nonempty string.",
                    ));
                }
                let origin = if path.starts_with('/') {
                    CcToolPathOrigin::FilesystemRoot
                } else {
                    CcToolPathOrigin::CrosstoolPackage
                };
                (path, origin)
            }
            (None, Some(artifact)) => (
                artifact.path().as_ref().into(),
                CcToolPathOrigin::WorkspaceRoot,
            ),
            _ => {
                return Err(invalid(
                    "tool must specify exactly one of 'path' and 'tool'",
                ));
            }
        };
        Ok(Self {
            path,
            origin,
            artifact,
            with_features: cc_with_features_set(with_features),
            execution_requirements,
            exec_os,
        })
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn origin(&self) -> CcToolPathOrigin {
        self.origin
    }
    pub fn artifact(&self) -> Option<&AnalysisArtifact> {
        self.artifact.as_ref()
    }
}

/// POSIX PathFragment normalization. Windows semantics are deliberately not inferred.
pub(super) fn normalize_posix_path(path: &str) -> Result<CompactString, CcFeatureError> {
    if path.contains('\0') {
        return Err(invalid("NUL in C++ toolchain path is unsupported"));
    }
    let absolute = path.starts_with('/');
    let mut segments = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            ".." if absolute => {}
            part => segments.push(part),
        }
    }
    let mut result = if absolute {
        String::from("/")
    } else {
        String::new()
    };
    result.push_str(&segments.join("/"));
    Ok(result.into())
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcArtifactNamePattern {
    pub category: CompactString,
    pub prefix: CompactString,
    pub extension: CompactString,
}
impl CcArtifactNamePattern {
    pub fn new(category: &str, prefix: &str, extension: &str) -> Result<Self, CcFeatureError> {
        let result = Self {
            category: category.into(),
            prefix: prefix.into(),
            extension: extension.into(),
        };
        result.validate()?;
        Ok(result)
    }
    pub(super) fn validate(&self) -> Result<(), CcFeatureError> {
        let (_, extensions) = category_spec(&self.category)?;
        if !extensions.contains(&self.extension.as_str()) {
            return Err(invalid(format!(
                "invalid extension '{}' for category '{}'",
                self.extension, self.category
            )));
        }
        Ok(())
    }
    pub(super) fn is_default(&self) -> bool {
        category_spec(&self.category).is_ok_and(|(prefix, extensions)| {
            self.prefix == prefix && self.extension == extensions[0]
        })
    }
}
fn category_spec(
    category: &str,
) -> Result<(&'static str, &'static [&'static str]), CcFeatureError> {
    Ok(match category {
        "static_library" => ("lib", &[".a", ".lib"]),
        "alwayslink_static_library" => ("lib", &[".lo", ".lo.lib"]),
        "dynamic_library" => ("lib", &[".so", ".dylib", ".dll", ".pyd", ".wasm"]),
        "executable" => ("", &["", ".exe", ".wasm"]),
        "interface_library" => ("lib", &[".ifso", ".tbd", ".if.lib", ".lib"]),
        "pic_file" => ("", &[".pic"]),
        "included_file_list" => ("", &[".d"]),
        "serialized_diagnostics_file" => ("", &[".dia"]),
        "object_file" => ("", &[".o", ".obj"]),
        "pic_object_file" => ("", &[".pic.o"]),
        "cpp_module" => ("", &[".pcm", ".gcm", ".ifc"]),
        "cpp_modules_info" => ("", &[".CXXModules.json"]),
        "cpp_modules_ddi" => ("", &[".ddi"]),
        "cpp_modules_modmap" => ("", &[".modmap"]),
        "cpp_modules_modmap_input" => ("", &[".modmap.input"]),
        "generated_assembly" => ("", &[".s", ".asm"]),
        "processed_header" => ("", &[".processed"]),
        "generated_header" => ("", &[".h"]),
        "preprocessed_c_source" => ("", &[".i"]),
        "preprocessed_cpp_source" => ("", &[".ii"]),
        "coverage_data_file" => ("", &[".gcno"]),
        "clif_output_proto" => ("", &[".opb"]),
        _ => return Err(invalid(format!("unknown artifact category '{category}'"))),
    })
}
pub(super) fn validate_name(name: &str, enabled: bool) -> Result<(), CcFeatureError> {
    if (name.is_empty() && !enabled)
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_+-.".contains(&c))
    {
        return Err(invalid(format!(
            "invalid feature or action config name '{name}'"
        )));
    }
    Ok(())
}
pub(super) fn validate_feature(feature: &CcFeature) -> Result<(), CcFeatureError> {
    for flags in &feature.flag_sets {
        if flags.actions.is_empty() {
            return Err(invalid("A flag_set in a feature must specify actions."));
        }
        validate_flag_set(flags)?;
    }
    for env in &feature.env_sets {
        if env.actions.is_empty() {
            return Err(invalid("An env_set must specify actions."));
        }
        for entry in &env.env_entries {
            if entry.key.is_empty() || entry.value.is_empty() {
                return Err(invalid("env_entry key and value must be nonempty"));
            }
        }
    }
    Ok(())
}
pub(super) fn validate_action(action: &CcActionConfig) -> Result<(), CcFeatureError> {
    for flags in &action.flag_sets {
        if flags.actions.len() != 1 || !flags.actions.contains(action.action_name.as_str()) {
            return Err(invalid(
                "action config flag_set must contain only its normalized action name",
            ));
        }
        validate_flag_set(flags)?;
    }
    Ok(())
}
fn validate_flag_set(flags: &CcFlagSet) -> Result<(), CcFeatureError> {
    let mut pending: Vec<_> = flags.flag_groups.iter().collect();
    while let Some(group) = pending.pop() {
        if group.expandables.is_empty() {
            return Err(invalid("flag_group must contain flags or flag_groups"));
        }
        let flags = matches!(group.expandables[0], CcExpandable::Flag(_));
        for expandable in &group.expandables {
            if matches!(expandable, CcExpandable::Flag(_)) != flags {
                return Err(invalid(
                    "flag_group must not contain both flags and flag_groups",
                ));
            }
            if let CcExpandable::Group(group) = expandable {
                pending.push(group);
            }
        }
        if let Some(equal) = &group.expand_if_equal {
            if equal.name.is_empty() || equal.value.is_empty() {
                return Err(invalid(
                    "variable_with_value name and value must be nonempty",
                ));
            }
        }
    }
    Ok(())
}
