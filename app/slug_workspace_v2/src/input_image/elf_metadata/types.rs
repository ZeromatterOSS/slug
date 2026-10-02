//! Finite wire facts. Syntax admission and physical allocation credit are separate.

use allocative::Allocative;
use dupe::Dupe;

use super::super::BudgetedImageBlock;
use super::super::ImageDescriptorBudget;
use super::super::ImageDescriptorBudgetError;
use super::super::ImageDescriptorReservation;
use super::ElfMetadataError;
use crate::FileContentDigest;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub struct ElfSourceDigestBinding {
    sha256: [u8; 32],
    size: u64,
}
impl ElfSourceDigestBinding {
    pub fn from_digest(digest: FileContentDigest) -> Self {
        Self {
            sha256: *digest.sha256(),
            size: digest.size_bytes(),
        }
    }
    pub fn digest(&self) -> FileContentDigest {
        FileContentDigest::new(self.sha256, self.size)
            .expect("binding comes from a validated digest")
    }
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub fn size_bytes(&self) -> u64 {
        self.size
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub enum ElfMetadataProfile {
    Elf64LittleLinuxX86_64V1,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub struct ElfMetadataLimits {
    pub max_program_headers: usize,
    pub max_dynamic_entries: usize,
    /// Literal bytes plus their terminating NUL, including an empty literal.
    pub max_string_bytes: usize,
    /// Logical parser-read occurrences, including repeated overlapping reads.
    pub max_proof_bytes: usize,
    pub max_proof_spans: usize,
    pub max_fact_bytes: usize,
}
impl Default for ElfMetadataLimits {
    fn default() -> Self {
        Self {
            max_program_headers: 1024,
            max_dynamic_entries: 8192,
            max_string_bytes: 4096,
            max_proof_bytes: 1 << 20,
            max_proof_spans: 8192,
            max_fact_bytes: 1 << 20,
        }
    }
}
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub struct ElfMetadataPolicy {
    profile: ElfMetadataProfile,
    limits: ElfMetadataLimits,
}
impl ElfMetadataPolicy {
    pub fn new(
        profile: ElfMetadataProfile,
        limits: ElfMetadataLimits,
    ) -> Result<Self, ElfMetadataError> {
        let cap = ElfMetadataLimits::default();
        if limits.max_program_headers == 0
            || limits.max_program_headers > cap.max_program_headers
            || limits.max_dynamic_entries == 0
            || limits.max_dynamic_entries > cap.max_dynamic_entries
            || limits.max_string_bytes == 0
            || limits.max_string_bytes > cap.max_string_bytes
            || limits.max_proof_bytes == 0
            || limits.max_proof_bytes > cap.max_proof_bytes
            || limits.max_proof_spans == 0
            || limits.max_proof_spans > cap.max_proof_spans
            || limits.max_fact_bytes == 0
            || limits.max_fact_bytes > cap.max_fact_bytes
        {
            return Err(ElfMetadataError::InvalidPolicy);
        }
        Ok(Self { profile, limits })
    }
    pub fn profile(&self) -> ElfMetadataProfile {
        self.profile
    }
    pub fn limits(&self) -> &ElfMetadataLimits {
        &self.limits
    }
}
impl Default for ElfMetadataPolicy {
    fn default() -> Self {
        Self {
            profile: ElfMetadataProfile::Elf64LittleLinuxX86_64V1,
            limits: ElfMetadataLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ElfMalformedReason {
    Header,
    ProgramTable,
    Segment,
    DynamicTable,
    StringTable,
    String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ElfUnsupportedReason {
    Class,
    Endian,
    Machine,
    ObjectType,
    Version,
    Abi,
    ExtendedProgramCount,
    MultipleInterpreter,
    MultipleDynamic,
    AmbiguousLoadMapping,
    Layout,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ElfMetadataLimit {
    ProgramHeaders,
    DynamicEntries,
    StringBytes,
    ProofBytes,
    ProofSpans,
    FactBytes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum ElfMetadataRejection {
    Malformed(ElfMalformedReason),
    Unsupported(ElfUnsupportedReason),
    PolicyLimit(ElfMetadataLimit),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub struct ElfHeaderFacts {
    pub object_type: u16,
    pub machine: u16,
    pub os_abi: u8,
    pub abi_version: u8,
    pub flags: u32,
    pub entry: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub struct ElfProgramHeader {
    pub segment_type: u32,
    pub flags: u32,
    pub offset: u64,
    pub virtual_address: u64,
    pub physical_address: u64,
    pub file_size: u64,
    pub memory_size: u64,
    pub alignment: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub struct ElfDynamicEntry {
    pub tag: i64,
    pub value: u64,
}

// No allocating Clone or owned data extraction from a published shared block.
#[derive(Debug, PartialEq, Eq, Allocative)]
pub struct ElfStringReference {
    pub dynamic_index: usize,
    pub tag: i64,
    pub bytes: Vec<u8>,
}
impl ElfStringReference {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
pub struct ElfMetadataFacts {
    pub header: ElfHeaderFacts,
    pub program_headers: Vec<ElfProgramHeader>,
    pub interpreter: Option<Vec<u8>>,
    pub dynamic_entries: Vec<ElfDynamicEntry>,
    pub string_refs: Vec<ElfStringReference>,
}
impl ElfMetadataFacts {
    pub fn header(&self) -> &ElfHeaderFacts {
        &self.header
    }
    pub fn program_headers(&self) -> &[ElfProgramHeader] {
        &self.program_headers
    }
    pub fn interpreter(&self) -> Option<&[u8]> {
        self.interpreter.as_deref()
    }
    pub fn dynamic_entries(&self) -> &[ElfDynamicEntry] {
        &self.dynamic_entries
    }
    pub fn string_refs(&self) -> &[ElfStringReference] {
        &self.string_refs
    }

    /// Versioned logical accounting, independent of Vec capacity/allocator.
    pub fn logical_bytes(&self) -> Option<usize> {
        let mut bytes = 128usize.checked_add(64)?;
        bytes = bytes.checked_add(self.program_headers.len().checked_mul(56)?)?;
        bytes = bytes.checked_add(self.dynamic_entries.len().checked_mul(16)?)?;
        bytes = bytes.checked_add(self.string_refs.len().checked_mul(32)?)?;
        for literal in self
            .interpreter
            .iter()
            .chain(self.string_refs.iter().map(|s| &s.bytes))
        {
            bytes = bytes.checked_add(literal.len().checked_add(1)?)?;
        }
        Some(bytes)
    }
    pub fn within_policy(&self, policy: ElfMetadataPolicy) -> bool {
        let limits = policy.limits();
        let valid_header = matches!(self.header.object_type, 2 | 3)
            && self.header.machine == 62
            && matches!(self.header.os_abi, 0 | 3)
            && self.header.abi_version == 0;
        valid_header
            && self.program_headers.len() <= limits.max_program_headers
            && self.dynamic_entries.len() <= limits.max_dynamic_entries
            && self.string_refs.len() <= self.dynamic_entries.len()
            && self
                .logical_bytes()
                .is_some_and(|bytes| bytes <= limits.max_fact_bytes)
            && self
                .interpreter
                .iter()
                .chain(self.string_refs.iter().map(|s| &s.bytes))
                .all(|literal| {
                    !literal.contains(&0)
                        && literal
                            .len()
                            .checked_add(1)
                            .is_some_and(|n| n <= limits.max_string_bytes)
                })
            && self.string_refs.iter().enumerate().all(|(i, string)| {
                matches!(string.tag, 1 | 14 | 15 | 29)
                    && self
                        .dynamic_entries
                        .get(string.dynamic_index)
                        .is_some_and(|entry| entry.tag == string.tag)
                    && (i == 0 || self.string_refs[i - 1].dynamic_index < string.dynamic_index)
            })
    }
    fn retained_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .program_headers
            .capacity()
            .checked_mul(std::mem::size_of::<ElfProgramHeader>())?
            .checked_add(
                self.dynamic_entries
                    .capacity()
                    .checked_mul(std::mem::size_of::<ElfDynamicEntry>())?,
            )?
            .checked_add(
                self.string_refs
                    .capacity()
                    .checked_mul(std::mem::size_of::<ElfStringReference>())?,
            )?;
        for literal in self
            .interpreter
            .iter()
            .chain(self.string_refs.iter().map(|s| &s.bytes))
        {
            bytes = bytes.checked_add(literal.capacity())?;
        }
        Some(bytes)
    }
}
#[derive(Debug, PartialEq, Eq, Allocative)]
pub enum ElfMetadataOutcome {
    Parsed(ElfMetadataFacts),
    Rejected(ElfMetadataRejection),
}
#[derive(Debug, PartialEq, Eq, Allocative)]
struct VerifiedInner {
    digest: ElfSourceDigestBinding,
    policy: ElfMetadataPolicy,
    outcome: ElfMetadataOutcome,
}

/// An immutable credited raw observation, not filesystem or publication authority.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct VerifiedElfMetadata(BudgetedImageBlock<VerifiedInner>);
impl VerifiedElfMetadata {
    pub fn new(
        digest: ElfSourceDigestBinding,
        policy: ElfMetadataPolicy,
        outcome: ElfMetadataOutcome,
        mut reservation: ImageDescriptorReservation,
    ) -> Result<Self, ImageDescriptorBudgetError> {
        if let ElfMetadataOutcome::Parsed(facts) = &outcome {
            if !facts.within_policy(policy) {
                return Err(ImageDescriptorBudgetError::ChargeOverflow);
            }
            let retained = facts
                .retained_bytes()
                .ok_or(ImageDescriptorBudgetError::ChargeOverflow)?;
            // Native reserves before allocation. Independently pin all supplied
            // capacities before this builder can publish them into DICE.
            if retained > reservation.charged_bytes() {
                reservation.try_grow(retained - reservation.charged_bytes())?;
            }
        }
        Ok(Self(reservation.freeze(VerifiedInner {
            digest,
            policy,
            outcome,
        })?))
    }
    pub fn digest(&self) -> ElfSourceDigestBinding {
        self.0.get().digest
    }
    pub fn policy(&self) -> ElfMetadataPolicy {
        self.0.get().policy
    }
    pub fn outcome(&self) -> &ElfMetadataOutcome {
        &self.0.get().outcome
    }
    pub fn same_budget(&self, budget: &ImageDescriptorBudget) -> bool {
        self.0.same_budget(budget)
    }
}
