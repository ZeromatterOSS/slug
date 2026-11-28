//! Length-framed SHA-256 for diagnostic event origins only.

use std::marker::PhantomData;
use std::path::Path;

use sha2::Digest;
use sha2::Sha256;
use starlark_map::small_map::SmallMap;

use super::BzlModuleIdentity;
use super::fingerprint_complete_identity;

pub(crate) struct EventOriginWriter(Sha256);

impl EventOriginWriter {
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut writer = Self(Sha256::new());
        writer.bytes(domain);
        writer
    }

    pub(crate) fn tag(&mut self, value: u8) {
        self.0.update([value]);
    }

    pub(crate) fn count(&mut self, value: usize) {
        self.0.update((value as u64).to_be_bytes());
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) {
        self.count(value.len());
        self.0.update(value);
    }

    pub(crate) fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    pub(crate) fn path(&mut self, value: &Path) {
        self.bytes(value.as_os_str().as_encoded_bytes());
    }

    pub(crate) fn optional<T>(&mut self, value: Option<T>, write: impl FnOnce(&mut Self, T)) {
        match value {
            Some(value) => {
                self.tag(1);
                write(self, value);
            }
            None => self.tag(0),
        }
    }

    pub(crate) fn identity(&mut self, identity: &BzlModuleIdentity) {
        fingerprint_complete_identity(&mut self.0, identity);
    }

    pub(crate) fn append_stamp(&mut self, value: &[u8; 32]) {
        self.0.update(value);
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(crate) enum EventOperand {
    BzlInputs,
    BuiltinCatalog,
    BuiltinSource,
}

impl EventOperand {
    fn domain(self) -> &'static [u8] {
        match self {
            Self::BzlInputs => b"slug-diagnostic-bzl-inputs-v1",
            Self::BuiltinCatalog => b"slug-diagnostic-builtin-catalog-v1",
            Self::BuiltinSource => b"slug-diagnostic-builtin-source-v1",
        }
    }
}

/// Only lives while one printing BUILD hashes its current borrowed operands.
/// Addresses select reusable results and never enter the diagnostic stamp.
#[derive(Default)]
pub(crate) struct EventOriginMemo<'a> {
    stamps: SmallMap<(EventOperand, *const ()), [u8; 32]>,
    owners: PhantomData<&'a ()>,
}

impl<'a> EventOriginMemo<'a> {
    pub(crate) fn stamp<T>(
        &mut self,
        operand: EventOperand,
        owner: &'a T,
        write: impl FnOnce(&mut Self, &mut EventOriginWriter),
    ) -> [u8; 32] {
        let key = (operand, std::ptr::from_ref(owner).cast::<()>());
        if let Some(stamp) = self.stamps.get(&key) {
            return *stamp;
        }
        let mut writer = EventOriginWriter::new(operand.domain());
        write(self, &mut writer);
        let stamp = writer.finish();
        self.stamps.insert(key, stamp);
        stamp
    }
}
