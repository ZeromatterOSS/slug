//! Finite analysis-only resource callback identity; execution remains deferred.
use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative, Dupe)]
enum ResourceSetKind {
    RulesCcLink,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative, Dupe)]
pub struct RetainedSpawnResourceSet(ResourceSetKind);

impl RetainedSpawnResourceSet {
    pub const SOURCE_LABEL: &'static str = "@@rules_cc+//cc/private/link:finalize_link_action.bzl";
    pub const SOURCE_DIGEST: [u8; 32] = [
        0xad, 0xc6, 0xea, 0x3b, 0x35, 0x5d, 0x0c, 0x5e, 0x5f, 0xbf, 0x1b, 0x1e, 0x9e, 0xaa, 0x7d,
        0x7d, 0xd7, 0xc0, 0xc0, 0x95, 0x23, 0x4a, 0x0c, 0xff, 0x7f, 0xdb, 0x4f, 0xc7, 0x2e, 0xb1,
        0x67, 0xc9,
    ];

    pub fn new(source: CanonicalLabel, digest: [u8; 32]) -> Result<Self, String> {
        if source != CanonicalLabel::parse(Self::SOURCE_LABEL).expect("pinned canonical source")
            || digest != Self::SOURCE_DIGEST
        {
            return Err("resource_set requires the pinned rules_cc link source".to_owned());
        }
        Ok(Self(ResourceSetKind::RulesCcLink))
    }

    pub fn source(&self) -> &'static str {
        Self::SOURCE_LABEL
    }

    pub fn digest(&self) -> &'static [u8; 32] {
        &Self::SOURCE_DIGEST
    }
}
