//! ASCII scanner equivalent to the pinned Java whole-component patterns.
use super::Component;
use super::DottedVersionParseError;
use super::DottedVersionPrepareError;
use super::Span;

#[derive(Debug)]
pub(super) struct Scan {
    pub original: usize,
    pub normalized: usize,
    pub examined: usize,
}

pub(super) fn scan<'a, E>(
    input: &'a str,
    admit: &mut impl FnMut() -> Result<(), E>,
    mut emit: impl FnMut(Component),
) -> Result<Scan, DottedVersionPrepareError<'a, E>> {
    let invalid = || DottedVersionPrepareError::Invalid(DottedVersionParseError { input });
    let mut result = Scan {
        original: 0,
        normalized: 0,
        examined: 0,
    };
    let mut start = 0;
    loop {
        admit().map_err(DottedVersionPrepareError::Admission)?;
        result.examined = result
            .examined
            .checked_add(1)
            .ok_or(DottedVersionPrepareError::StorageOverflow)?;
        let tail = &input[start..];
        let end = tail.find('.').map_or(input.len(), |n| start + n);
        let bytes = input[start..end].as_bytes();
        // A descriptive component terminates parsing BEFORE the ignored tail.
        if bytes.first().is_some_and(u8::is_ascii_alphabetic)
            && bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            break;
        }
        let component = component(bytes, start, end).ok_or_else(invalid)?;
        result.original = result
            .original
            .checked_add(1)
            .ok_or(DottedVersionPrepareError::StorageOverflow)?;
        if result.original == 1 || !component.is_zero() {
            result.normalized = result.original;
        }
        emit(component);
        if end == input.len() {
            break;
        }
        start = end + 1;
    }
    if result.original == 0 {
        return Err(invalid());
    }
    Ok(result)
}

fn component(bytes: &[u8], start: usize, end: usize) -> Option<Component> {
    let first_end = bytes
        .iter()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(bytes.len());
    if first_end == 0 {
        return None;
    }
    let first = number(&bytes[..first_end])?;
    let suffix = &bytes[first_end..];
    if !suffix.iter().all(u8::is_ascii_alphanumeric) {
        return None;
    }
    let alpha_end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_digit())
        .map_or(first_end, |n| n + 1);
    let alpha = (alpha_end > first_end).then_some(Span {
        start: start + first_end,
        end: start + alpha_end,
    });
    let second = if alpha_end < bytes.len() {
        number(&bytes[alpha_end..])?
    } else {
        0
    };
    Some(Component {
        first,
        alpha,
        second,
        spelling: Span { start, end },
    })
}

fn number(bytes: &[u8]) -> Option<i32> {
    bytes.iter().try_fold(0i32, |n, b| {
        n.checked_mul(10)?.checked_add(i32::from(b - b'0'))
    })
}
