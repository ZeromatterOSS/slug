//! Finite wire facts, without loader search or executable admission.
use slug_workspace_v2::ElfDynamicEntry;
use slug_workspace_v2::ElfHeaderFacts;
use slug_workspace_v2::ElfMalformedReason;
use slug_workspace_v2::ElfMetadataFacts;
use slug_workspace_v2::ElfMetadataLimit;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ElfMetadataRejection;
use slug_workspace_v2::ElfProgramHeader;
use slug_workspace_v2::ElfStringReference;
use slug_workspace_v2::ElfUnsupportedReason;
use slug_workspace_v2::ImageDescriptorReservation;

use super::super::InputImageCaptureError;
use super::proof::ProofReader;
use super::proof::ReadError;
use super::proof::reserve_rows;

enum ParseError {
    Rejected(ElfMetadataRejection),
    Operational(InputImageCaptureError),
}
impl From<InputImageCaptureError> for ParseError {
    fn from(error: InputImageCaptureError) -> Self {
        Self::Operational(error)
    }
}
fn malformed(reason: ElfMalformedReason) -> ParseError {
    ParseError::Rejected(ElfMetadataRejection::Malformed(reason))
}
fn unsupported(reason: ElfUnsupportedReason) -> ParseError {
    ParseError::Rejected(ElfMetadataRejection::Unsupported(reason))
}
fn limit(reason: ElfMetadataLimit) -> ParseError {
    ParseError::Rejected(ElfMetadataRejection::PolicyLimit(reason))
}

fn read<'a>(
    reader: &'a mut ProofReader<'_>,
    offset: u64,
    length: usize,
    reason: ElfMalformedReason,
) -> Result<&'a [u8], ParseError> {
    reader.read(offset, length).map_err(|error| match error {
        ReadError::OutsideFile => malformed(reason),
        ReadError::Limit(which) => limit(which),
        ReadError::Operational(error) => ParseError::Operational(error),
    })
}
fn u16_at(bytes: &[u8], start: usize) -> u16 {
    u16::from_le_bytes(
        bytes[start..start + 2]
            .try_into()
            .expect("checked wire width"),
    )
}
fn u32_at(bytes: &[u8], start: usize) -> u32 {
    u32::from_le_bytes(
        bytes[start..start + 4]
            .try_into()
            .expect("checked wire width"),
    )
}
fn u64_at(bytes: &[u8], start: usize) -> u64 {
    u64::from_le_bytes(
        bytes[start..start + 8]
            .try_into()
            .expect("checked wire width"),
    )
}

struct FactsBudget {
    bytes: usize,
    maximum: usize,
}
impl FactsBudget {
    fn add(&mut self, bytes: usize) -> Result<(), ParseError> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.maximum)
            .ok_or_else(|| limit(ElfMetadataLimit::FactBytes))?;
        Ok(())
    }
}

fn literal(
    reader: &mut ProofReader<'_>,
    offset: u64,
    available: u64,
    policy: ElfMetadataPolicy,
    credit: &mut ImageDescriptorReservation,
    facts: &mut FactsBudget,
) -> Result<Vec<u8>, ParseError> {
    let maximum = policy.limits().max_string_bytes;
    let mut bytes = Vec::new();
    let mut consumed = 0usize;
    loop {
        if consumed as u64 >= available {
            return Err(malformed(ElfMalformedReason::String));
        }
        if consumed >= maximum {
            return Err(limit(ElfMetadataLimit::StringBytes));
        }
        let length = 64usize
            .min(maximum - consumed)
            .min((available - consumed as u64).min(usize::MAX as u64) as usize);
        let position = offset
            .checked_add(consumed as u64)
            .ok_or_else(|| malformed(ElfMalformedReason::String))?;
        let chunk = read(reader, position, length, ElfMalformedReason::String)?;
        let terminator = chunk.iter().position(|byte| *byte == 0);
        let saved = terminator.unwrap_or(chunk.len());
        facts.add(saved + usize::from(terminator.is_some()))?;
        reserve_rows(&mut bytes, credit, saved)?;
        bytes.extend_from_slice(&chunk[..saved]);
        if terminator.is_some() {
            return Ok(bytes);
        }
        consumed += length;
    }
}

fn check_range(
    offset: u64,
    size: u64,
    file_size: u64,
    reason: ElfMalformedReason,
) -> Result<(), ParseError> {
    if offset.checked_add(size).is_none_or(|end| end > file_size) {
        return Err(malformed(reason));
    }
    Ok(())
}

fn parse_inner(
    reader: &mut ProofReader<'_>,
    policy: ElfMetadataPolicy,
    credit: &mut ImageDescriptorReservation,
) -> Result<ElfMetadataFacts, ParseError> {
    let mut logical = FactsBudget {
        bytes: 0,
        maximum: policy.limits().max_fact_bytes,
    };
    let file_size = reader.size();
    logical.add(128 + 64)?;
    let bytes = read(reader, 0, 64, ElfMalformedReason::Header)?;
    if &bytes[..4] != b"\x7fELF" {
        return Err(malformed(ElfMalformedReason::Header));
    }
    if bytes[4] != 2 {
        return Err(unsupported(ElfUnsupportedReason::Class));
    }
    if bytes[5] != 1 {
        return Err(unsupported(ElfUnsupportedReason::Endian));
    }
    if bytes[6] != 1 || u32_at(bytes, 20) != 1 {
        return Err(unsupported(ElfUnsupportedReason::Version));
    }
    if !matches!(bytes[7], 0 | 3) || bytes[8] != 0 {
        return Err(unsupported(ElfUnsupportedReason::Abi));
    }
    let object_type = u16_at(bytes, 16);
    if !matches!(object_type, 2 | 3) {
        return Err(unsupported(ElfUnsupportedReason::ObjectType));
    }
    if u16_at(bytes, 18) != 62 {
        return Err(unsupported(ElfUnsupportedReason::Machine));
    }
    if u16_at(bytes, 52) != 64 || u16_at(bytes, 54) != 56 {
        return Err(malformed(ElfMalformedReason::Header));
    }
    let count = u16_at(bytes, 56) as usize;
    if count == 0xffff {
        return Err(unsupported(ElfUnsupportedReason::ExtendedProgramCount));
    }
    if count > policy.limits().max_program_headers {
        return Err(limit(ElfMetadataLimit::ProgramHeaders));
    }
    let phoff = u64_at(bytes, 32);
    check_range(
        phoff,
        (count as u64)
            .checked_mul(56)
            .ok_or_else(|| malformed(ElfMalformedReason::ProgramTable))?,
        file_size,
        ElfMalformedReason::ProgramTable,
    )?;
    let header = ElfHeaderFacts {
        object_type,
        machine: 62,
        os_abi: bytes[7],
        abi_version: bytes[8],
        flags: u32_at(bytes, 48),
        entry: u64_at(bytes, 24),
    };
    let mut result = ElfMetadataFacts {
        header,
        program_headers: Vec::new(),
        interpreter: None,
        dynamic_entries: Vec::new(),
        string_refs: Vec::new(),
    };
    let mut interpreter = None;
    let mut dynamic = None;
    let mut last_load = None;
    for index in 0..count {
        logical.add(56)?;
        let bytes = read(
            reader,
            phoff + index as u64 * 56,
            56,
            ElfMalformedReason::ProgramTable,
        )?;
        let row = ElfProgramHeader {
            segment_type: u32_at(bytes, 0),
            flags: u32_at(bytes, 4),
            offset: u64_at(bytes, 8),
            virtual_address: u64_at(bytes, 16),
            physical_address: u64_at(bytes, 24),
            file_size: u64_at(bytes, 32),
            memory_size: u64_at(bytes, 40),
            alignment: u64_at(bytes, 48),
        };
        match row.segment_type {
            1 => {
                check_range(
                    row.offset,
                    row.file_size,
                    file_size,
                    ElfMalformedReason::Segment,
                )?;
                if row.file_size > row.memory_size
                    || row.virtual_address.checked_add(row.memory_size).is_none()
                    || (row.alignment > 1
                        && (!row.alignment.is_power_of_two()
                            || row.virtual_address % row.alignment != row.offset % row.alignment))
                    || last_load.is_some_and(|old| old > row.virtual_address)
                {
                    return Err(malformed(ElfMalformedReason::Segment));
                }
                last_load = Some(row.virtual_address);
            }
            2 => {
                if dynamic.is_some() {
                    return Err(unsupported(ElfUnsupportedReason::MultipleDynamic));
                }
                check_range(
                    row.offset,
                    row.file_size,
                    file_size,
                    ElfMalformedReason::DynamicTable,
                )?;
                if row.file_size % 16 != 0 {
                    return Err(malformed(ElfMalformedReason::DynamicTable));
                }
                dynamic = Some((row.offset, row.file_size));
            }
            3 => {
                if interpreter.is_some() {
                    return Err(unsupported(ElfUnsupportedReason::MultipleInterpreter));
                }
                check_range(
                    row.offset,
                    row.file_size,
                    file_size,
                    ElfMalformedReason::String,
                )?;
                interpreter = Some((row.offset, row.file_size));
            }
            _ => {} // Raw unknown segment controls remain in the result.
        }
        reserve_rows(&mut result.program_headers, credit, 1)?;
        result.program_headers.push(row);
    }
    if let Some((offset, length)) = interpreter {
        result.interpreter = Some(literal(
            reader,
            offset,
            length,
            policy,
            credit,
            &mut logical,
        )?);
    }
    if let Some((offset, length)) = dynamic {
        let mut index = 0u64;
        loop {
            if index >= length / 16 {
                return Err(malformed(ElfMalformedReason::DynamicTable));
            }
            if index as usize >= policy.limits().max_dynamic_entries {
                return Err(limit(ElfMetadataLimit::DynamicEntries));
            }
            logical.add(16)?;
            let bytes = read(
                reader,
                offset + index * 16,
                16,
                ElfMalformedReason::DynamicTable,
            )?;
            let row = ElfDynamicEntry {
                tag: u64_at(bytes, 0) as i64,
                value: u64_at(bytes, 8),
            };
            let finished = row.tag == 0;
            reserve_rows(&mut result.dynamic_entries, credit, 1)?;
            result.dynamic_entries.push(row);
            if finished {
                break;
            }
            index += 1;
        }
    }
    if result
        .dynamic_entries
        .iter()
        .any(|row| matches!(row.tag, 1 | 14 | 15 | 29))
    {
        let mut tables = result.dynamic_entries.iter().filter(|row| row.tag == 5);
        let table = tables
            .next()
            .ok_or_else(|| malformed(ElfMalformedReason::StringTable))?
            .value;
        if tables.next().is_some() {
            return Err(malformed(ElfMalformedReason::StringTable));
        }
        let mut sizes = result.dynamic_entries.iter().filter(|row| row.tag == 10);
        let size = sizes
            .next()
            .ok_or_else(|| malformed(ElfMalformedReason::StringTable))?
            .value;
        if sizes.next().is_some() {
            return Err(malformed(ElfMalformedReason::StringTable));
        }
        let end = table
            .checked_add(size)
            .ok_or_else(|| malformed(ElfMalformedReason::StringTable))?;
        let mut translated = None;
        for load in result
            .program_headers
            .iter()
            .filter(|row| row.segment_type == 1)
        {
            let load_end = load
                .virtual_address
                .checked_add(load.file_size)
                .ok_or_else(|| malformed(ElfMalformedReason::Segment))?;
            if table < load_end && load.virtual_address < end {
                if translated.is_some() {
                    return Err(unsupported(ElfUnsupportedReason::AmbiguousLoadMapping));
                }
                if table < load.virtual_address || end > load_end {
                    return Err(unsupported(ElfUnsupportedReason::Layout));
                }
                translated = Some(
                    load.offset
                        .checked_add(table - load.virtual_address)
                        .ok_or_else(|| malformed(ElfMalformedReason::StringTable))?,
                );
            }
        }
        let offset = translated.ok_or_else(|| unsupported(ElfUnsupportedReason::Layout))?;
        check_range(offset, size, file_size, ElfMalformedReason::StringTable)?;
        for index in 0..result.dynamic_entries.len() {
            let row = &result.dynamic_entries[index];
            if !matches!(row.tag, 1 | 14 | 15 | 29) {
                continue;
            }
            let (tag, position) = (row.tag, row.value);
            if position >= size {
                return Err(malformed(ElfMalformedReason::StringTable));
            }
            logical.add(32)?;
            let bytes = literal(
                reader,
                offset + position,
                size - position,
                policy,
                credit,
                &mut logical,
            )?;
            reserve_rows(&mut result.string_refs, credit, 1)?;
            result.string_refs.push(ElfStringReference {
                dynamic_index: index,
                tag,
                bytes,
            });
        }
    }
    debug_assert_eq!(result.logical_bytes(), Some(logical.bytes));
    Ok(result)
}

pub(super) fn parse(
    reader: &mut ProofReader<'_>,
    policy: ElfMetadataPolicy,
    credit: &mut ImageDescriptorReservation,
) -> Result<ElfMetadataOutcome, InputImageCaptureError> {
    match parse_inner(reader, policy, credit) {
        Ok(facts) => Ok(ElfMetadataOutcome::Parsed(facts)),
        Err(ParseError::Rejected(reason)) => Ok(ElfMetadataOutcome::Rejected(reason)),
        Err(ParseError::Operational(error)) => Err(error),
    }
}
