//! Slug-native structural configuration identity.
//!
//! This deliberately does not reproduce Bazel's configuration checksum or
//! output-directory bytes.  It records the typed configuration values and
//! projects them through a versioned, domain-separated Slug format instead.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_identity_v2::OptionLabelContext;
use strong_hash::StrongHash;

use super::convert::ConvertError;
use super::defaults::materialize_default;
use super::host::AutoCpuToken;
use super::host::HostConversionInputs;
use super::host::HostPathFlavor;
use super::label_convert;
use super::label_convert::LabelConvertError;
use super::label_convert::LabelValue;
use super::label_convert::MixedValue;
use super::registry::NATIVE_OPTION_DESCRIPTORS;
use super::registry::NativeOptionDescriptor;
use super::value::*;

const PROJECTION_CONTEXT: &str = "slug.build/configuration-projection/v1";
const PROJECTION_MAGIC: &[u8] = b"slugcfg\0";
const PROJECTION_VERSION: u16 = 1;

/// The semantic role of a configuration.  Its byte spelling is Slug-native.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum SlugConfigurationKind {
    Target,
    Exec,
    HostLike,
}

impl SlugConfigurationKind {
    fn tag(self) -> u16 {
        match self {
            Self::Target => 0x0020,
            Self::Exec => 0x0021,
            Self::HostLike => 0x0022,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Exec => "exec",
            Self::HostLike => "host-like",
        }
    }
}

impl fmt::Display for SlugConfigurationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The packet-fixed root string build setting, `@@//:setting`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct RootStringSettingValue(CompactString);

impl RootStringSettingValue {
    pub fn new(value: impl Into<CompactString>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Full, domain-separated configuration digest.  It is neither a Bazel
/// checksum nor a REAPI/CAS digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct SlugConfigurationProjection([u8; 32]);

impl SlugConfigurationProjection {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Stable display token for diagnostics only.
    pub fn display_token(&self) -> String {
        format!("slugcfg-v1:{}", hex::encode(self.0))
    }

    /// Versioned, namespaced component for Slug-owned output paths.
    pub fn path_component(&self) -> String {
        format!("slugcfg-v1-{}", hex::encode(self.0))
    }
}

impl fmt::Display for SlugConfigurationProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display_token())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative)]
pub enum SlugConfigurationError {
    MissingAutoCpu,
    MissingPathFlavor,
    UnsupportedDescriptor { ordinal: u32 },
    InvalidDefault { ordinal: u32 },
    InvalidLabelDefault { ordinal: u32 },
    UnexpectedDescriptorCount { actual: usize },
    DuplicateDescriptor { ordinal: u32 },
    DescriptorOrdinalOverflow,
}

impl fmt::Display for SlugConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAutoCpu => formatter.write_str("native configuration requires AutoCPU"),
            Self::MissingPathFlavor => {
                formatter.write_str("native configuration requires a host path flavor")
            }
            Self::UnsupportedDescriptor { ordinal } => {
                write!(
                    formatter,
                    "unsupported native configuration descriptor {ordinal}"
                )
            }
            Self::InvalidDefault { ordinal } => {
                write!(
                    formatter,
                    "invalid native configuration default at descriptor {ordinal}"
                )
            }
            Self::InvalidLabelDefault { ordinal } => {
                write!(
                    formatter,
                    "invalid label configuration default at descriptor {ordinal}"
                )
            }
            Self::UnexpectedDescriptorCount { actual } => {
                write!(formatter, "expected 341 native descriptors, found {actual}")
            }
            Self::DuplicateDescriptor { ordinal } => {
                write!(
                    formatter,
                    "duplicate native descriptor at ordinal {ordinal}"
                )
            }
            Self::DescriptorOrdinalOverflow => {
                formatter.write_str("native descriptor ordinal does not fit u32")
            }
        }
    }
}

impl std::error::Error for SlugConfigurationError {}

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
enum OptionValue {
    Native(NativeOccurrence),
    Label(Option<LabelValue>),
    Mixed(Option<MixedValue>),
}

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
struct OptionRecord {
    ordinal: u32,
    class_name: &'static str,
    canonical_name: &'static str,
    value: OptionValue,
}

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
struct SlugConfigurationData {
    kind: SlugConfigurationKind,
    options: Arc<[OptionRecord]>,
    root_string_setting: Option<RootStringSettingValue>,
    canonical_bytes: Arc<[u8]>,
    projection: SlugConfigurationProjection,
}

/// Immutable typed configuration.  The `Arc` makes cloning a graph identity a
/// cheap refcount increment, while equality remains fully structural.
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub struct SlugConfiguration(Arc<SlugConfigurationData>);

impl SlugConfiguration {
    pub fn new_default(
        kind: SlugConfigurationKind,
        host: &HostConversionInputs,
    ) -> Result<Self, SlugConfigurationError> {
        let auto_cpu = host
            .auto_cpu()
            .ok_or(SlugConfigurationError::MissingAutoCpu)?;
        let path_flavor = host
            .path_flavor()
            .ok_or(SlugConfigurationError::MissingPathFlavor)?;
        if NATIVE_OPTION_DESCRIPTORS.len() != 341 {
            return Err(SlugConfigurationError::UnexpectedDescriptorCount {
                actual: NATIVE_OPTION_DESCRIPTORS.len(),
            });
        }
        let mut seen = BTreeSet::new();
        let mut options = Vec::with_capacity(NATIVE_OPTION_DESCRIPTORS.len());
        for (index, descriptor) in NATIVE_OPTION_DESCRIPTORS.iter().enumerate() {
            let ordinal = u32::try_from(index)
                .map_err(|_| SlugConfigurationError::DescriptorOrdinalOverflow)?;
            if !seen.insert((descriptor.class_name, descriptor.canonical_name)) {
                return Err(SlugConfigurationError::DuplicateDescriptor { ordinal });
            }
            options.push(OptionRecord {
                ordinal,
                class_name: descriptor.class_name,
                canonical_name: descriptor.canonical_name,
                value: default_option(descriptor, ordinal, auto_cpu, path_flavor)?,
            });
        }
        Ok(finish_configuration(kind, Arc::from(options), None))
    }

    pub fn default_target(host: &HostConversionInputs) -> Result<Self, SlugConfigurationError> {
        Self::new_default(SlugConfigurationKind::Target, host)
    }

    pub fn default_exec(host: &HostConversionInputs) -> Result<Self, SlugConfigurationError> {
        Self::new_default(SlugConfigurationKind::Exec, host)
    }

    pub fn default_host_like(host: &HostConversionInputs) -> Result<Self, SlugConfigurationError> {
        Self::new_default(SlugConfigurationKind::HostLike, host)
    }

    pub fn kind(&self) -> SlugConfigurationKind {
        self.0.kind
    }

    pub fn root_string_setting(&self) -> Option<&RootStringSettingValue> {
        self.0.root_string_setting.as_ref()
    }

    pub fn with_root_string_setting(&self, value: RootStringSettingValue) -> Self {
        finish_configuration(self.0.kind, self.0.options.dupe(), Some(value))
    }

    pub fn projection(&self) -> SlugConfigurationProjection {
        self.0.projection
    }

    /// Exact bytes fed to the Slug-native configuration projection.  These are
    /// not Bazel checksum or ActionKey bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0.canonical_bytes
    }

    pub fn option_count(&self) -> usize {
        self.0.options.len()
    }
}

impl Hash for SlugConfiguration {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This is a structural hash, not the projection's digest.  Writing the
        // exact canonical bytes also preserves the RegexFilter semantic Eq.
        state.write(self.canonical_bytes());
    }
}

impl PartialOrd for SlugConfiguration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SlugConfiguration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.canonical_bytes().cmp(other.canonical_bytes())
    }
}

impl StrongHash for SlugConfiguration {
    fn strong_hash<H: Hasher>(&self, state: &mut H) {
        // strong_hash is deliberately just an adapter around the reviewed
        // canonical byte grammar; its derive grammar is not this identity.
        state.write(self.canonical_bytes());
    }
}

fn default_option(
    descriptor: &NativeOptionDescriptor,
    ordinal: u32,
    auto_cpu: AutoCpuToken,
    path_flavor: HostPathFlavor,
) -> Result<OptionValue, SlugConfigurationError> {
    match (descriptor.class_name, descriptor.canonical_name) {
        ("com.google.devtools.build.lib.analysis.PlatformOptions", "platform_mappings") => {
            // The empty PlatformMappingKey has no path separators to normalize,
            // but requiring the flavor preserves the converter's fail-closed
            // Host boundary for future non-empty admitted values.
            let _ = path_flavor;
            Ok(OptionValue::Native(NativeOccurrence::Scalar(
                NativeValue::Text(CompactString::new("")),
            )))
        }
        (
            "com.google.devtools.build.lib.analysis.ShellConfiguration.Options",
            "shell_executable",
        ) => Ok(OptionValue::Native(NativeOccurrence::Absent)),
        ("com.google.devtools.build.lib.analysis.config.CoreOptions", "cpu" | "host_cpu") => {
            Ok(OptionValue::Native(NativeOccurrence::Scalar(
                NativeValue::Text(CompactString::new(auto_cpu.as_str())),
            )))
        }
        (
            "com.google.devtools.build.lib.analysis.test.TestConfiguration.TestOptions",
            "default_test_resources",
        ) => Ok(OptionValue::Native(NativeOccurrence::Absent)),
        (
            _,
            "modify_execution_info"
            | "host_per_file_copt"
            | "per_file_copt"
            | "per_file_ltobackendopt",
        ) if descriptor.raw_default == "\"null\"" && descriptor.allow_multiple => Ok(
            OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([])))),
        ),
        _ if label_convert::classify(descriptor).is_some() => {
            let value = label_convert::materialize_label_default(
                descriptor,
                OptionLabelContext::FirstRoundCanonical,
            )
            .map_err(|error| match error {
                LabelConvertError::Invalid | LabelConvertError::Unsupported => {
                    SlugConfigurationError::InvalidLabelDefault { ordinal }
                }
            })?;
            Ok(OptionValue::Label(value))
        }
        _ if label_convert::classify_mixed(descriptor).is_some() => {
            let value = label_convert::materialize_mixed_default(
                descriptor,
                OptionLabelContext::FirstRoundCanonical,
            )
            .map_err(|_| SlugConfigurationError::InvalidLabelDefault { ordinal })?;
            Ok(OptionValue::Mixed(value))
        }
        _ => materialize_default(descriptor)
            .map_err(|error| match error {
                ConvertError::Unsupported => {
                    SlugConfigurationError::UnsupportedDescriptor { ordinal }
                }
                ConvertError::Invalid => SlugConfigurationError::InvalidDefault { ordinal },
            })
            .map(|value| {
                OptionValue::Native(match value {
                    None => NativeOccurrence::Absent,
                    Some(NativeValue::List(values)) if descriptor.allow_multiple => {
                        NativeOccurrence::List(values)
                    }
                    Some(value) => NativeOccurrence::Scalar(value),
                })
            }),
    }
}

fn finish_configuration(
    kind: SlugConfigurationKind,
    options: Arc<[OptionRecord]>,
    root_string_setting: Option<RootStringSettingValue>,
) -> SlugConfiguration {
    let provisional = SlugConfigurationData {
        kind,
        options: options.dupe(),
        root_string_setting: root_string_setting.clone(),
        canonical_bytes: Arc::from([]),
        projection: SlugConfigurationProjection([0; 32]),
    };
    let canonical_bytes: Arc<[u8]> = Arc::from(canonical_bytes(&provisional));
    let mut hasher = blake3::Hasher::new_derive_key(PROJECTION_CONTEXT);
    hasher.update(&canonical_bytes);
    let projection = SlugConfigurationProjection(*hasher.finalize().as_bytes());
    SlugConfiguration(Arc::new(SlugConfigurationData {
        kind,
        options,
        root_string_setting,
        canonical_bytes,
        projection,
    }))
}

fn canonical_bytes(data: &SlugConfigurationData) -> Vec<u8> {
    let mut encoder = Encoder::default();
    encoder.bytes(PROJECTION_MAGIC);
    encoder.u16(PROJECTION_VERSION);
    encoder.field(0x0001, |root| {
        root.field(0x0010, |kind| kind.field(data.kind.tag(), |_| {}));
        root.field(0x0011, |options| {
            options.u64(u64::try_from(data.options.len()).expect("option count fits u64"));
            for record in data.options.iter() {
                options.field(0x0100, |entry| {
                    entry.field(0x0101, |value| value.u32(record.ordinal));
                    entry.field(0x0102, |value| value.raw_text(record.class_name));
                    entry.field(0x0103, |value| value.raw_text(record.canonical_name));
                    entry.field(0x0104, |value| value.option_value(&record.value));
                });
            }
        });
        root.field(0x0012, |setting| match &data.root_string_setting {
            None => setting.field(0x0600, |_| {}),
            Some(value) => setting.field(0x0601, |some| {
                some.field(0x0610, |label| label.raw_text("@@//:setting"));
                some.field(0x0611, |value_field| value_field.raw_text(value.as_str()));
            }),
        });
    });
    encoder.into_bytes()
}

#[derive(Default)]
struct Encoder(Vec<u8>);

impl Encoder {
    fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_be_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_be_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes(&value.to_be_bytes());
    }

    fn raw_text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn field(&mut self, tag: u16, write: impl FnOnce(&mut Self)) {
        let mut payload = Self::default();
        write(&mut payload);
        self.u16(tag);
        self.u64(u64::try_from(payload.0.len()).expect("payload length fits u64"));
        self.bytes(&payload.0);
    }

    fn option_value(&mut self, value: &OptionValue) {
        match value {
            OptionValue::Native(value) => {
                self.field(0x0200, |field| field.native_occurrence(value))
            }
            OptionValue::Label(value) => {
                self.field(0x0201, |field| field.label_occurrence(value.as_ref()))
            }
            OptionValue::Mixed(value) => {
                self.field(0x0202, |field| field.mixed_occurrence(value.as_ref()))
            }
        }
    }

    fn sequence<T>(&mut self, values: &[T], tag: u16, write: impl Fn(&mut Self, &T)) {
        self.u64(u64::try_from(values.len()).expect("sequence length fits u64"));
        for value in values {
            self.field(tag, |field| write(field, value));
        }
    }

    fn native_occurrence(&mut self, value: &NativeOccurrence) {
        match value {
            NativeOccurrence::Absent => self.field(0x0210, |_| {}),
            NativeOccurrence::Scalar(value) => {
                self.field(0x0211, |field| field.native_value(value))
            }
            NativeOccurrence::List(values) => self.field(0x0212, |field| {
                field.u64(u64::try_from(values.len()).expect("list length fits u64"));
                for value in values.iter() {
                    field.field(0x0370, |item| item.native_value(value));
                }
            }),
        }
    }

    fn native_value(&mut self, value: &NativeValue) {
        match value {
            NativeValue::Bool(value) => {
                self.field(0x0300, |field| field.bytes(&[u8::from(*value)]))
            }
            NativeValue::Int(value) => self.field(0x0301, |field| field.i32(*value)),
            NativeValue::Text(value) => self.field(0x0302, |field| field.raw_text(value)),
            NativeValue::Tri(value) => self.field(0x0303, |field| {
                field.field(
                    match value {
                        TriState::Auto => 0x0310,
                        TriState::Yes => 0x0311,
                        TriState::No => 0x0312,
                    },
                    |_| {},
                )
            }),
            NativeValue::Enum(value) => self.field(0x0304, |field| {
                field.field(enum_family_tag(value.family), |_| {});
                field.field(0x0330, |member| member.raw_text(&value.member));
            }),
            NativeValue::Duration(value) => self.field(0x0305, |field| {
                field.field(0x0380, |seconds| seconds.i64(value.seconds));
                field.field(0x0381, |nanos| nanos.u32(value.nanos));
            }),
            NativeValue::Dotted(value) => self.field(0x0306, |field| field.raw_text(value)),
            NativeValue::Entry(key, value) => self.field(0x0307, |field| {
                field.field(0x0390, |key_field| key_field.raw_text(key));
                field.field(0x0391, |value_field| value_field.raw_text(value));
            }),
            NativeValue::Env(value) => self.field(0x0308, |field| match value {
                EnvValue::Set(key, value) => field.field(0x0340, |set| {
                    set.field(0x0343, |key_field| key_field.raw_text(key));
                    set.field(0x0344, |value_field| value_field.raw_text(value));
                }),
                EnvValue::Inherit(value) => field.field(0x0341, |inherit| {
                    inherit.field(0x0345, |name| name.raw_text(value));
                }),
                EnvValue::Unset(value) => field.field(0x0342, |unset| {
                    unset.field(0x0345, |name| name.raw_text(value));
                }),
            }),
            NativeValue::Shard(value) => self.field(0x0309, |field| match value {
                ShardValue::Explicit => field.field(0x0350, |_| {}),
                ShardValue::Disabled => field.field(0x0351, |_| {}),
                ShardValue::Forced(value) => field.field(0x0352, |forced| {
                    forced.field(0x0353, |number| number.i32(*value));
                }),
            }),
            NativeValue::Runs(value) => self.field(0x030a, |field| {
                field.field(0x03a0, |runs| runs.i32(value.positive_runs().get()));
            }),
            NativeValue::RegexFilterDefault(value) => self.field(0x030b, |field| {
                // Eq/Ord are semantic-only, so source spelling is intentionally omitted.
                field.field(
                    match value.semantic {
                        RegexFilterDefaultSemantic::ExcludeAll => 0x0360,
                        RegexFilterDefaultSemantic::InstrumentationDefault => 0x0361,
                    },
                    |_| {},
                );
            }),
            NativeValue::List(values) => self.field(0x030c, |field| {
                field.sequence(values, 0x0370, |item, value| item.native_value(value));
            }),
            NativeValue::OrderedMap(values) => self.field(0x030d, |field| {
                field.u64(u64::try_from(values.len()).expect("map length fits u64"));
                for (key, value) in values.iter() {
                    field.field(0x0371, |pair| {
                        pair.field(0x0372, |key_field| key_field.native_value(key));
                        pair.field(0x0373, |value_field| value_field.native_value(value));
                    });
                }
            }),
        }
    }

    fn label_occurrence(&mut self, value: Option<&LabelValue>) {
        match value {
            None => self.field(0x0400, |_| {}),
            Some(value) => self.field(0x0401, |field| field.label_value(value)),
        }
    }

    fn label_value(&mut self, value: &LabelValue) {
        match value {
            LabelValue::Label(value) => self.field(0x0410, |field| {
                field.field(0x0420, |label| label.raw_text(&value.unambiguous_form()));
            }),
            LabelValue::Labels(values) => self.field(0x0411, |field| {
                field.u64(u64::try_from(values.0.len()).expect("label count fits u64"));
                for value in values.0.iter() {
                    field.field(0x0421, |item| {
                        item.field(0x0420, |label| label.raw_text(&value.unambiguous_form()));
                    });
                }
            }),
            LabelValue::LabelToStringEntry(value) => self.field(0x0412, |field| {
                field.field(0x0422, |label| {
                    label.raw_text(&value.label.unambiguous_form())
                });
                field.field(0x0423, |text| text.raw_text(&value.value));
            }),
            LabelValue::LabelMap(values) => self.field(0x0413, |field| {
                field.u64(u64::try_from(values.0.len()).expect("map length fits u64"));
                for (key, value) in values.0.iter() {
                    field.field(0x0424, |pair| {
                        pair.field(0x0425, |key_field| key_field.raw_text(key));
                        match value {
                            None => pair.field(0x0426, |_| {}),
                            Some(label) => pair.field(0x0427, |present| {
                                present
                                    .field(0x0420, |item| item.raw_text(&label.unambiguous_form()));
                            }),
                        }
                    });
                }
            }),
            LabelValue::FlagAlias(value) => self.field(0x0414, |field| {
                field.field(0x0428, |entry| {
                    entry.field(0x0429, |alias| alias.raw_text(&value.alias));
                    entry.field(0x042a, |label| {
                        label.raw_text(&value.label.unambiguous_form())
                    });
                });
            }),
        }
    }

    fn mixed_occurrence(&mut self, value: Option<&MixedValue>) {
        match value {
            None => self.field(0x0500, |_| {}),
            Some(value) => self.field(0x0501, |field| field.mixed_value(value)),
        }
    }

    fn mixed_value(&mut self, value: &MixedValue) {
        match value {
            MixedValue::CustomFlag(value) => self.field(0x0511, |field| field.raw_text(value)),
            MixedValue::RunUnder(value) => self.field(0x0510, |field| match value {
                label_convert::RunUnder::Label {
                    original,
                    suffix,
                    label,
                } => field.field(0x0520, |label_variant| {
                    label_variant.field(0x0522, |field| field.raw_text(original));
                    label_variant.field(0x0523, |field| {
                        field.sequence(&suffix.0, 0x0526, |item, value| item.raw_text(value));
                    });
                    label_variant.field(0x0524, |field| field.raw_text(&label.unambiguous_form()));
                }),
                label_convert::RunUnder::Command {
                    original,
                    suffix,
                    command,
                } => field.field(0x0521, |command_variant| {
                    command_variant.field(0x0522, |field| field.raw_text(original));
                    command_variant.field(0x0523, |field| {
                        field.sequence(&suffix.0, 0x0526, |item, value| item.raw_text(value));
                    });
                    command_variant.field(0x0525, |field| field.raw_text(command));
                }),
            }),
        }
    }
}

fn enum_family_tag(value: EnumFamily) -> u16 {
    match value {
        EnumFamily::StrictDeps => 0x0320,
        EnumFamily::Exec => 0x0321,
        EnumFamily::OutputName => 0x0322,
        EnumFamily::OutputPaths => 0x0323,
        EnumFamily::Include => 0x0324,
        EnumFamily::Android => 0x0325,
        EnumFamily::Apk => 0x0326,
        EnumFamily::Merger => 0x0327,
        EnumFamily::MergerOrder => 0x0328,
        EnumFamily::Apple => 0x0329,
        EnumFamily::Dynamic => 0x032a,
        EnumFamily::Classpath => 0x032b,
        EnumFamily::OneVersion => 0x032c,
        EnumFamily::Cancel => 0x032d,
        EnumFamily::Compilation => 0x032e,
        EnumFamily::Strip => 0x032f,
    }
}

#[cfg(test)]
mod tests {
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;

    use super::*;
    use crate::native::host::HostConversionInputs;

    #[derive(Default)]
    struct RecordingHasher(Vec<u8>);

    impl Hasher for RecordingHasher {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
    }

    fn host(auto_cpu: AutoCpuToken, path_flavor: HostPathFlavor) -> HostConversionInputs {
        HostConversionInputs::new(
            Some(auto_cpu),
            Some(path_flavor),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap()
    }

    #[test]
    fn default_configuration_materializes_all_pinned_descriptors() {
        let configuration =
            SlugConfiguration::default_target(&host(AutoCpuToken::K8, HostPathFlavor::Unix))
                .unwrap();
        assert_eq!(configuration.option_count(), 341);
        assert_eq!(configuration.kind(), SlugConfigurationKind::Target);
    }

    #[test]
    fn projection_is_full_namespaced_and_stable_for_equal_structure() {
        let first =
            SlugConfiguration::default_target(&host(AutoCpuToken::K8, HostPathFlavor::Unix))
                .unwrap();
        let second =
            SlugConfiguration::default_target(&host(AutoCpuToken::K8, HostPathFlavor::Unix))
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.projection(), second.projection());
        assert_eq!(
            hex::encode(first.projection().as_bytes()),
            "abc6de66486cc9eff604c3e0795796631112a6d92cf3336370de8e8f6acf953a"
        );
        assert_eq!(&first.canonical_bytes()[..10], b"slugcfg\0\0\x01");
        assert_eq!(&first.canonical_bytes()[10..12], &0x0001u16.to_be_bytes());
        assert_eq!(
            first.projection().display_token().len(),
            "slugcfg-v1:".len() + 64
        );
        assert_eq!(
            first.projection().path_component().len(),
            "slugcfg-v1-".len() + 64
        );
        assert!(
            first
                .projection()
                .display_token()
                .starts_with("slugcfg-v1:")
        );
    }

    #[test]
    fn cpu_kind_and_root_setting_are_structural_and_domain_separated() {
        let base = SlugConfiguration::default_target(&host(AutoCpuToken::K8, HostPathFlavor::Unix))
            .unwrap();
        let cpu_changed =
            SlugConfiguration::default_target(&host(AutoCpuToken::Aarch64, HostPathFlavor::Unix))
                .unwrap();
        let setting = base.with_root_string_setting(RootStringSettingValue::new("command"));
        assert_ne!(base.projection(), cpu_changed.projection());
        assert_ne!(base.projection(), setting.projection());
        assert_ne!(
            base.projection(),
            SlugConfiguration::default_exec(&host(AutoCpuToken::K8, HostPathFlavor::Unix))
                .unwrap()
                .projection()
        );
    }

    #[test]
    fn semantic_regex_seed_does_not_leak_original_spelling_into_identity() {
        let descriptor = &NATIVE_OPTION_DESCRIPTORS[6];
        let base = SlugConfiguration::default_target(&host(AutoCpuToken::K8, HostPathFlavor::Unix))
            .unwrap();
        let mut data = (*base.0).clone();
        let mut options = data.options.as_ref().to_vec();
        let OptionValue::Native(NativeOccurrence::Scalar(NativeValue::RegexFilterDefault(seed))) =
            &options[6].value
        else {
            panic!("expected regex seed");
        };
        assert_eq!(descriptor.canonical_name, "toolchain_resolution_debug");
        options[6].value =
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::RegexFilterDefault(
                RegexFilterDefaultSeed::new("different spelling", seed.semantic),
            )));
        data.options = Arc::from(options);
        let changed = finish_configuration(data.kind, data.options, data.root_string_setting);
        assert_eq!(base, changed);
        assert_eq!(base.projection(), changed.projection());
        let mut first = RecordingHasher::default();
        let mut second = RecordingHasher::default();
        base.hash(&mut first);
        changed.hash(&mut second);
        assert_eq!(first.0, base.canonical_bytes());
        assert_eq!(first.0, second.0);
    }

    #[test]
    fn absent_required_host_facts_fail_closed() {
        let missing =
            HostConversionInputs::new(None, None, None, Arc::from([]), Arc::from([])).unwrap();
        assert_eq!(
            SlugConfiguration::default_target(&missing),
            Err(SlugConfigurationError::MissingAutoCpu)
        );
    }
}
