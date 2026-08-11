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
use slug_analysis_v2::ConfiguredActionExecGroup;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputKind;
use slug_identity_v2::CanonicalLabel;

use super::dice::ResolvedFileWriteSemanticView;

const MAGIC: &[u8] = b"slugact\0";
const VERSION: u16 = 1;
const AQUERY_DISPLAY_CONTEXT: &str = "slug.v2.filewrite.aquery-display.v1";

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct FileWriteSemanticIdentity(Arc<[u8]>);

impl FileWriteSemanticIdentity {
    pub fn from_resolved(view: &ResolvedFileWriteSemanticView<'_>) -> Result<Self, &'static str> {
        let mut encoder = Encoder::default();
        encoder.bytes(MAGIC);
        encoder.bytes(&VERSION.to_be_bytes());
        encoder.field(0x0001, |field| configured_key(field, view.action().owner()))?;
        encoder.field(0x0002, |field| {
            if view.action().output().kind() != ActionOutputKind::File {
                return Err("FileWrite identity requires a File output");
            }
            field.field(0x0210, |_| {});
            field.field(0x0201, |path| path.text(view.action().output().path()));
            Ok::<(), &'static str>(())
        })?;
        let ActionKind::Write {
            content,
            is_executable,
        } = view.action().spec().kind()
        else {
            return Err("FileWrite identity requires a Write action");
        };
        write_action(
            &mut encoder,
            view.action().spec().mnemonic(),
            content,
            *is_executable,
        );
        encoder.field(0x0004, |field| match view.action().exec_group() {
            ConfiguredActionExecGroup::Default => field.field(0x0401, |_| {}),
        });
        let selected = view.action().execution_platform();
        if view.platform().configured_target_key() != Some(selected) {
            return Err("FileWrite identity platform key mismatch");
        }
        encoder.field(0x0005, |field| configured_key(field, selected))?;
        if view
            .platform_fact()
            .exec_properties
            .windows(2)
            .any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err("FileWrite identity requires key-ordered exec properties");
        }
        encoder.field(0x0006, |field| {
            field.count(view.platform_fact().exec_properties.len());
            for (key, value) in view.platform_fact().exec_properties.iter() {
                field.field(0x0601, |entry| {
                    entry.field(0x0610, |key_field| key_field.bytes(key.as_bytes()));
                    entry.field(0x0611, |value_field| value_field.bytes(value.as_bytes()));
                });
            }
        });
        encoder.field(0x0007, |field| {
            field.count(view.platform_constraints().len());
            for (index, constraint) in view.platform_constraints().iter().enumerate() {
                field.field(0x0701, |entry| {
                    entry.field(0x0710, |value| value.count(index));
                    entry.field(0x0711, |value| {
                        configured_key(
                            value,
                            constraint
                                .constraint_value()
                                .configured_target_key()
                                .ok_or("constraint value is not configured")?,
                        )
                    })?;
                    entry.field(0x0712, |setting| {
                        configured_key(
                            setting,
                            constraint
                                .constraint_setting()
                                .configured_target_key()
                                .ok_or("constraint setting is not configured")?,
                        )
                    })
                })?;
            }
            Ok::<(), &'static str>(())
        })?;
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
    use slug_analysis_v2::ConfigurationKey;
    use slug_analysis_v2::ConfiguredTargetKey;
    use slug_identity_v2::CanonicalLabel;

    use super::Encoder;
    use super::configured_key;
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
}
