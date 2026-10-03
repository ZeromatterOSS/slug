//! One optional first-poison POD; no payload, caller identity or side trace.
#![cfg_attr(not(all(test, target_os = "linux")), allow(dead_code))]

macro_rules! code_enum {
    ($name:ident { $($variant:ident = $code:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(in crate::runtime) enum $name { $($variant),+ }
        impl $name {
            pub(in crate::runtime) fn code(self) -> u64 {
                match self { $(Self::$variant => $code),+ }
            }
        }
    };
}
code_enum!(SourceBufferSite {
    Unattributed = 0, RegistryCapture = 1, RegistryArc = 2,
    RegistryAddress = 3, RegistryFileRead = 4, HttpParser = 5,
    HttpDuplexParser = 6, ArchiveAddress = 7, SelectedPayload = 8,
    SelectedScratch = 9, PatchCapture = 10, PatchTarget = 11,
    SharedIoBuffer = 12, AssociationRecorder = 13,
    RegistryUrlLength = 20, HttpUrlLength = 21, ArchiveUrlLength = 22,
    RedirectLength = 23, DnsCardinality = 24, SelectedPayloadGrowth = 25,
});
code_enum!(SourceBufferUnit { Unknown = 0, Bytes = 1, Entries = 2 });

#[cfg(all(test, target_os = "linux"))]
code_enum!(SourceBufferKind {
    Unknown = 0, ReservationCeiling = 1, ReservationOverflow = 2, ExplicitBound = 3,
});

#[cfg(all(test, target_os = "linux"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct SourceBufferOrigin {
    pub(in crate::runtime) site: SourceBufferSite,
    pub(in crate::runtime) kind: SourceBufferKind,
    pub(in crate::runtime) unit: SourceBufferUnit,
    pub(in crate::runtime) requested: Option<usize>,
    pub(in crate::runtime) bound: Option<usize>,
    pub(in crate::runtime) live_bytes: usize,
    pub(in crate::runtime) bank_ceiling_bytes: usize,
}
#[cfg(all(test, target_os = "linux"))]
impl SourceBufferOrigin {
    pub(super) fn unknown(live_bytes: usize, bank_ceiling_bytes: usize) -> Self {
        Self {
            site: SourceBufferSite::Unattributed,
            kind: SourceBufferKind::Unknown,
            unit: SourceBufferUnit::Unknown,
            requested: None,
            bound: None,
            live_bytes,
            bank_ceiling_bytes,
        }
    }
    pub(in crate::runtime) fn complete(self) -> bool {
        use SourceBufferKind as K;
        use SourceBufferSite as S;
        use SourceBufferUnit as U;
        if self.bank_ceiling_bytes == 0 || self.live_bytes > self.bank_ceiling_bytes {
            return false;
        }
        let (Some(requested), Some(bound)) = (self.requested, self.bound) else {
            return false;
        };
        let reservation_site = matches!(
            self.site,
            S::RegistryCapture
                | S::RegistryArc
                | S::RegistryAddress
                | S::RegistryFileRead
                | S::HttpParser
                | S::HttpDuplexParser
                | S::ArchiveAddress
                | S::SelectedPayload
                | S::SelectedScratch
                | S::PatchCapture
                | S::PatchTarget
                | S::SharedIoBuffer
                | S::AssociationRecorder
        );
        match self.kind {
            K::ReservationCeiling | K::ReservationOverflow => {
                if !reservation_site || self.unit != U::Bytes || bound != self.bank_ceiling_bytes {
                    return false;
                }
                match (self.kind, self.live_bytes.checked_add(requested)) {
                    (K::ReservationCeiling, Some(next)) => next > bound,
                    (K::ReservationOverflow, None) => true,
                    _ => false,
                }
            }
            K::ExplicitBound => {
                requested > bound
                    && match self.site {
                        S::RegistryUrlLength
                        | S::HttpUrlLength
                        | S::ArchiveUrlLength
                        | S::RedirectLength => self.unit == U::Bytes && bound == 8192,
                        S::DnsCardinality => self.unit == U::Entries && bound == 64,
                        S::SelectedPayloadGrowth => {
                            self.unit == U::Bytes
                                && requested <= 32 * 1024 * 1024
                                && bound <= 32 * 1024 * 1024
                        }
                        _ => false,
                    }
            }
            K::Unknown => false,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::runtime) struct SourceBufferSnapshot {
    pub(in crate::runtime) armed: bool,
    pub(in crate::runtime) first_error_is_buffer_limit: bool,
    pub(in crate::runtime) origin: Option<SourceBufferOrigin>,
}
#[cfg(all(test, target_os = "linux"))]
#[derive(Default)]
pub(super) struct SourceBufferState {
    armed: bool,
    origin: Option<SourceBufferOrigin>,
}
#[cfg(all(test, target_os = "linux"))]
impl SourceBufferState {
    pub(super) fn arm(&mut self) {
        self.armed = true;
    }
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    // Scope's original first-poison transaction is the only creation authority.
    pub(super) fn created(&mut self, origin: SourceBufferOrigin) {
        if self.armed {
            self.origin = Some(origin);
        }
    }
    pub(super) fn snapshot(&self, first_is_buffer: bool) -> SourceBufferSnapshot {
        SourceBufferSnapshot {
            armed: self.armed,
            first_error_is_buffer_limit: first_is_buffer,
            origin: self.origin,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "buffer_provenance/tests.rs"]
mod tests;
