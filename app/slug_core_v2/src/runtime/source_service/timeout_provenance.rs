//! Finite wait-boundary tags; stored evidence exists only in the Linux test service.
#![cfg_attr(not(all(test, target_os = "linux")), allow(dead_code))]

use std::time::Duration;
use std::time::Instant;

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
code_enum!(SourceTimeoutSite {
    GenericScope = 1, GenericWait = 2, Dns = 3, Connect = 4,
    TlsHandshake = 5, HttpHandshake = 6, Headers = 7, Body = 8,
});
code_enum!(SourceTimeoutDomain { Uncomputed = 0, Common = 1, Step = 2, Tie = 3 });
code_enum!(SourceTimeoutPhase {
    UnattributedPoison = 0, ScopeCheck = 1, DuringEntryCheck = 2,
    Timer = 3, Postcheck = 4, DuringFinalCheck = 5,
});
code_enum!(SourceTimeoutReturn { Unknown = 0, Deadline = 1, CompletedPrimary = 2 });

#[cfg(all(test, target_os = "linux"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct SourceTimeoutCaller {
    pub(in crate::runtime) owner: u8,
    pub(in crate::runtime) line: u32,
}
#[cfg(all(test, target_os = "linux"))]
impl SourceTimeoutCaller {
    pub(super) fn project(file: &str, line: u32) -> Self {
        let owner = match file.strip_prefix("app/slug_core_v2/").unwrap_or(file) {
            "src/runtime/source_service/scope.rs" => 1,
            "src/runtime/source_service/regular_file.rs" => 2,
            "src/runtime/path_observation.rs" => 3,
            "src/runtime/path_observation/scoped_directory.rs" => 4,
            "src/runtime/dice.rs" => 5,
            "src/runtime/input_image_bridge.rs" => 6,
            "src/runtime/source_service/diagnostic.rs" => 7,
            "src/runtime/source_service/timeout_provenance/tests.rs" => 8,
            _ => 0,
        };
        Self {
            owner,
            line: if owner == 0 { 0 } else { line },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct SourceTimeoutOrigin {
    pub(in crate::runtime) site: SourceTimeoutSite,
    pub(in crate::runtime) domain: SourceTimeoutDomain,
    pub(in crate::runtime) phase: SourceTimeoutPhase,
    pub(in crate::runtime) return_disposition: SourceTimeoutReturn,
    pub(in crate::runtime) step_add_overflow: bool,
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) caller: Option<SourceTimeoutCaller>,
}
impl SourceTimeoutOrigin {
    pub(super) fn common(site: SourceTimeoutSite, phase: SourceTimeoutPhase) -> Self {
        Self {
            site,
            domain: SourceTimeoutDomain::Common,
            phase,
            return_disposition: SourceTimeoutReturn::Deadline,
            step_add_overflow: false,
            #[cfg(all(test, target_os = "linux"))]
            caller: None,
        }
    }
    pub(super) fn unattributed() -> Self {
        Self {
            site: SourceTimeoutSite::GenericScope,
            domain: SourceTimeoutDomain::Uncomputed,
            phase: SourceTimeoutPhase::UnattributedPoison,
            return_disposition: SourceTimeoutReturn::Unknown,
            step_add_overflow: false,
            #[cfg(all(test, target_os = "linux"))]
            caller: None,
        }
    }
}

pub(super) struct SourceTimeoutDeadline {
    pub(super) deadline: Instant,
    domain: SourceTimeoutDomain,
    overflow: bool,
}
impl SourceTimeoutDeadline {
    pub(super) fn select(common: Instant, start: Instant, step: Duration) -> Self {
        let end = start.checked_add(step);
        let (deadline, domain) = match end {
            Some(end) if end < common => (end, SourceTimeoutDomain::Step),
            Some(end) if end == common => (common, SourceTimeoutDomain::Tie),
            _ => (common, SourceTimeoutDomain::Common),
        };
        Self {
            deadline,
            domain,
            overflow: end.is_none(),
        }
    }
    pub(super) fn origin(
        &self,
        site: SourceTimeoutSite,
        phase: SourceTimeoutPhase,
        return_disposition: SourceTimeoutReturn,
    ) -> SourceTimeoutOrigin {
        SourceTimeoutOrigin {
            site,
            domain: self.domain,
            phase,
            return_disposition,
            step_add_overflow: self.overflow,
            #[cfg(all(test, target_os = "linux"))]
            caller: None,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::runtime) struct SourceTimeoutSnapshot {
    pub(in crate::runtime) armed: bool,
    pub(in crate::runtime) first_error_is_work_deadline: bool,
    pub(in crate::runtime) origin: Option<SourceTimeoutOrigin>,
    pub(in crate::runtime) later_sticky_work_deadline_observed: bool,
}

#[cfg(all(test, target_os = "linux"))]
#[derive(Default)]
pub(super) struct SourceTimeoutState {
    armed: bool,
    origin: Option<SourceTimeoutOrigin>,
    sticky: bool,
}
#[cfg(all(test, target_os = "linux"))]
impl SourceTimeoutState {
    pub(super) fn is_armed(&self) -> bool {
        self.armed
    }
    pub(super) fn arm(&mut self) {
        self.armed = true;
    }
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(super) fn created(&mut self, origin: SourceTimeoutOrigin) {
        if self.armed {
            self.origin = Some(origin);
        }
    }
    pub(super) fn sticky(&mut self) {
        if self.armed {
            self.sticky = true;
        }
    }
    pub(super) fn snapshot(&self, first_is_work: bool) -> SourceTimeoutSnapshot {
        SourceTimeoutSnapshot {
            armed: self.armed,
            first_error_is_work_deadline: first_is_work,
            origin: self.origin,
            later_sticky_work_deadline_observed: self.sticky,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
thread_local! {
    // One-shot, scalar test boundary. No callback runs under a service lock.
    static FINAL_CHECK_DELAY: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}
#[cfg(all(test, target_os = "linux"))]
pub(super) fn before_final_check_for_test() {
    if let Some(bound) = FINAL_CHECK_DELAY.with(|slot| slot.take()) {
        while Instant::now() < bound {
            std::hint::spin_loop();
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "timeout_provenance/tests.rs"]
mod tests;
