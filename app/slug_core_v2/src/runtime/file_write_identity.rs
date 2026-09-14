/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use slug_analysis_v2::ConfiguredExecGroup;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputKind;
use slug_identity_v2::CanonicalLabel;

use super::dice::ResolvedFileWriteSemanticView;

const MAGIC: &[u8] = b"slugact\0";
const VERSION: u16 = 1;
const AQUERY_DISPLAY_CONTEXT: &str = "slug.v2.filewrite.aquery-display.v1";
const NATIVE_DEFAULT_MISSING_TOOLCHAIN_ERROR: &str = "For more information on platforms or toolchains see https://bazel.build/concepts/platforms-intro.";

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct FileWriteSemanticIdentity(Arc<[u8]>);

impl FileWriteSemanticIdentity {
    pub fn from_resolved(view: &ResolvedFileWriteSemanticView<'_>) -> Result<Self, &'static str> {
        Self::from_configured(view.action())
    }

    fn from_configured(
        action: &slug_analysis_v2::ConfiguredActionView<'_>,
    ) -> Result<Self, &'static str> {
        let mut encoder = Encoder::default();
        encoder.bytes(MAGIC);
        encoder.bytes(&VERSION.to_be_bytes());
        encoder.field(0x0001, |field| configured_key(field, action.owner()))?;
        encoder.field(0x0002, |field| {
            if action.output().kind() != ActionOutputKind::File {
                return Err("FileWrite identity requires a File output");
            }
            field.field(0x0210, |_| {});
            field.field(0x0201, |path| path.text(action.output().path()));
            Ok::<(), &'static str>(())
        })?;
        let ActionKind::Write {
            content,
            is_executable,
        } = action.spec().kind()
        else {
            return Err("FileWrite identity requires a Write action");
        };
        write_action(
            &mut encoder,
            action.spec().mnemonic(),
            content,
            *is_executable,
        );
        encoder.field(0x0004, |field| match action.exec_group() {
            ConfiguredExecGroup::Default => field.field(0x0401, |_| {}),
            ConfiguredExecGroup::Named(name) => field.field(0x0402, |value| value.text(name)),
        });
        let selected = action.execution_platform();
        encoder.field(0x0005, |field| configured_key(field, selected))?;
        if action
            .platform_fact()
            .exec_properties
            .windows(2)
            .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err("FileWrite identity requires key-ordered exec properties");
        }
        encoder.field(0x0006, |field| {
            field.count(action.platform_fact().exec_properties.len());
            for (key, value) in action.platform_fact().exec_properties.iter() {
                field.field(0x0601, |entry| {
                    entry.field(0x0610, |key_field| key_field.bytes(key.as_bytes()));
                    entry.field(0x0611, |value_field| value_field.bytes(value.as_bytes()));
                });
            }
        });
        encoder.field(0x0007, |field| {
            field.count(action.platform_constraints().len());
            for (index, constraint) in action.platform_constraints().iter().enumerate() {
                field.field(0x0701, |entry| {
                    entry.field(0x0710, |value| value.count(index));
                    entry.field(0x0711, |value| {
                        configured_key(value, constraint.constraint_value())
                    })?;
                    entry.field(0x0712, |setting| {
                        configured_key(setting, constraint.constraint_setting())
                    })
                })?;
            }
            Ok::<(), &'static str>(())
        })?;
        let raw = action.raw_platform_fact();
        if raw
            .exec_properties
            .windows(2)
            .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err("FileWrite identity requires key-ordered raw exec properties");
        }
        encode_raw_platform_fact(&mut encoder, raw, action.platform_fact());
        Ok(Self(encoder.0.into()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Presentation-only projection; never semantic, DICE, cache, or REAPI identity.
    pub(crate) fn aquery_display_token(&self) -> String {
        format!(
            "slugact-display-v1:{}",
            hex::encode(blake3::derive_key(AQUERY_DISPLAY_CONTEXT, self.as_bytes(),))
        )
    }
}

fn encode_properties<'a>(
    encoder: &mut Encoder,
    properties: impl ExactSizeIterator<Item = (&'a str, &'a str)>,
) {
    encoder.count(properties.len());
    for (key, value) in properties {
        encoder.field(0x0601, |entry| {
            entry.field(0x0610, |key_field| key_field.bytes(key.as_bytes()));
            entry.field(0x0611, |value_field| value_field.bytes(value.as_bytes()));
        });
    }
}

fn encode_raw_platform_fact(
    encoder: &mut Encoder,
    raw: &slug_analysis_v2::PlatformSemanticFact,
    merged: &slug_analysis_v2::PlatformSemanticFact,
) {
    if raw.exec_properties != merged.exec_properties {
        encoder.field(0x0008, |field| {
            encode_properties(
                field,
                raw.exec_properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            )
        });
    }
    if raw.missing_toolchain_error.as_deref() != Some(NATIVE_DEFAULT_MISSING_TOOLCHAIN_ERROR) {
        encoder.field(0x0009, |field| match &raw.missing_toolchain_error {
            Some(message) => field.field(0x0901, |value| value.text(message)),
            None => field.field(0x0900, |_| {}),
        });
    }
}

fn write_action(encoder: &mut Encoder, mnemonic: &str, content: &str, is_executable: bool) {
    encoder.field(0x0003, |field| {
        field.field(0x0301, |value| value.text(mnemonic));
        field.field(0x0302, |value| value.text(content));
        field.field(0x0303, |value| value.bytes(&[u8::from(is_executable)]));
    });
}

fn configured_key(encoder: &mut Encoder, key: &ConfiguredTargetKey) -> Result<(), &'static str> {
    encoder.field(0x1001, |field| canonical_label(field, key.label()));
    let configuration = key
        .configuration()
        .slug_configuration()
        .ok_or("FileWrite identity rejects legacy configuration")?;
    encoder.field(0x1002, |field| field.bytes(configuration.canonical_bytes()));
    if let Some(platform) = key.toolchain_execution_platform() {
        encoder.field(0x1003, |field| canonical_label(field, platform));
    }
    Ok(())
}

fn canonical_label(encoder: &mut Encoder, label: &CanonicalLabel) {
    encoder.field(0x1101, |field| field.text(label.package().repo().as_str()));
    encoder.field(0x1102, |field| {
        field.text(label.package().package().as_str())
    });
    encoder.field(0x1103, |field| field.text(label.target().as_str()));
    encoder.field(0x1104, |field| match label.mapping_id() {
        None => field.field(0x1110, |_| {}),
        Some(mapping) => field.field(0x1111, |value| value.text(mapping.as_str())),
    });
}

#[derive(Default)]
struct Encoder(Vec<u8>);

impl Encoder {
    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    fn count(&mut self, value: usize) {
        self.bytes(&u64::try_from(value).expect("length fits u64").to_be_bytes());
    }
    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    fn field<T>(&mut self, tag: u16, write: impl FnOnce(&mut Self) -> T) -> T {
        let mut payload = Self::default();
        let result = write(&mut payload);
        self.bytes(&tag.to_be_bytes());
        self.bytes(
            &u64::try_from(payload.0.len())
                .expect("field payload length fits u64")
                .to_be_bytes(),
        );
        self.bytes(&payload.0);
        result
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slug_analysis_v2::ConfigurationKey;
    use slug_analysis_v2::ConfiguredTargetKey;
    use slug_analysis_v2::PlatformSemanticFact;
    use slug_configuration_v2::SlugConfiguration;
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    use slug_identity_v2::CanonicalLabel;

    use super::Encoder;
    use super::NATIVE_DEFAULT_MISSING_TOOLCHAIN_ERROR;
    use super::canonical_label;
    use super::configured_key;
    use super::encode_raw_platform_fact;
    use super::write_action;

    fn pair(left: &str, right: &str) -> Vec<u8> {
        let mut encoder = Encoder::default();
        encoder.field(1, |field| field.bytes(left.as_bytes()));
        encoder.field(2, |field| field.bytes(right.as_bytes()));
        encoder.0
    }

    #[test]
    fn framing_separates_prefixes_and_embedded_nuls() {
        assert_ne!(pair("ab", "c"), pair("a", "bc"));
        assert_ne!(pair("a\0", "b"), pair("a", "\0b"));
    }

    #[test]
    fn configured_key_rejects_legacy_configuration() {
        let key = ConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:legacy").unwrap(),
            ConfigurationKey::exec("legacy").unwrap(),
        );
        assert_eq!(
            configured_key(&mut Encoder::default(), &key),
            Err("FileWrite identity rejects legacy configuration")
        );
    }

    #[test]
    fn write_action_discriminates_executable_bit() {
        let encode = |is_executable| {
            let mut encoder = Encoder::default();
            write_action(&mut encoder, "FileWrite", "content\n", is_executable);
            encoder.0
        };
        assert_ne!(encode(false), encode(true));
    }

    #[test]
    fn selected_toolchain_request_owner_bytes() {
        let host = HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap();
        let configuration =
            ConfigurationKey::from_slug(SlugConfiguration::default_target(&host).unwrap());
        let label = CanonicalLabel::parse("@@//pkg:owner").unwrap();
        let key = |preference| {
            let key = ConfiguredTargetKey::new(label.clone(), configuration.clone());
            match preference {
                Some(preference) => key.with_toolchain_execution_platform(preference),
                None => key,
            }
        };
        let encode = |key: &ConfiguredTargetKey| {
            let mut encoder = Encoder::default();
            configured_key(&mut encoder, key).unwrap();
            encoder.0
        };
        let none = encode(&key(None));
        let mut expected = Encoder::default();
        expected.field(0x1001, |field| canonical_label(field, &label));
        expected.field(0x1002, |field| {
            field.bytes(
                configuration
                    .slug_configuration()
                    .unwrap()
                    .canonical_bytes(),
            )
        });
        assert_eq!(none, expected.0);

        let a_one = Arc::new(CanonicalLabel::parse("@@//:platform_a").unwrap());
        let a_two = Arc::new(CanonicalLabel::parse("@@//:platform_a").unwrap());
        let b = Arc::new(CanonicalLabel::parse("@@//:platform_b").unwrap());
        assert!(!Arc::ptr_eq(&a_one, &a_two));
        let a = encode(&key(Some(a_one)));
        assert_eq!(a, encode(&key(Some(a_two))));
        let b = encode(&key(Some(b)));
        assert_ne!(none, a);
        assert_ne!(none, b);
        assert_ne!(a, b);
    }

    #[test]
    fn raw_platform_facts_distinguish_masked_properties_and_messages() {
        let fact = |properties: &[(&str, &str)], message: Option<&str>| PlatformSemanticFact {
            exec_properties: properties
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into()))
                .collect::<Vec<_>>()
                .into(),
            missing_toolchain_error: message.map(Arc::from),
        };
        let encode = |raw: PlatformSemanticFact, merged: PlatformSemanticFact| {
            let mut encoder = Encoder::default();
            encode_raw_platform_fact(&mut encoder, &raw, &merged);
            encoder.0
        };
        let default = Some(NATIVE_DEFAULT_MISSING_TOOLCHAIN_ERROR);
        assert_eq!(
            encode(
                fact(&[("raw", "one")], default),
                fact(&[("raw", "one")], default)
            ),
            Vec::<u8>::new(),
        );
        assert_ne!(
            encode(
                fact(&[("raw", "one")], default),
                fact(&[("merged", "one")], default)
            ),
            encode(
                fact(&[("raw", "two")], default),
                fact(&[("merged", "one")], default)
            ),
        );
        assert_ne!(
            encode(fact(&[], None), fact(&[], None)),
            encode(fact(&[], Some("custom")), fact(&[], Some("custom"))),
        );
    }
}
