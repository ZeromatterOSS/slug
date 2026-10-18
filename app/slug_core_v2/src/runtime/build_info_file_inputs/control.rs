//! Fixed native attempt control only; no runtime bytes or transferable credit.
use std::alloc::Layout;
use std::mem::size_of;

use super::Mutex;
use super::NativeBuildInfoFileInputsTerminal;
use super::ReadPermit;

const MAX_CONTROL_BYTES: usize = 4096;
const MAX_CONTROL_FACTS: usize = 512;

fn cost() -> Result<(usize, usize), &'static str> {
    // Actual Arc control counters and aligned complete Mutex/Option/enum body.
    // Inline terminal storage includes its tags and shared handles. The Vec's
    // body belongs to the runtime ledger, never to this fixed admission.
    let (grant, _) = Layout::new::<[usize; 2]>()
        .extend(Layout::new::<Mutex<Option<ReadPermit>>>())
        .map_err(|_| "native BuildInfo control layout overflow")?;
    let bytes = grant
        .pad_to_align()
        .size()
        .checked_add(
            Layout::new::<NativeBuildInfoFileInputsTerminal>()
                .pad_to_align()
                .size(),
        )
        .ok_or("native BuildInfo control bytes overflow")?;
    let facts = bytes
        .div_ceil(size_of::<usize>())
        .checked_add(4)
        .ok_or("native BuildInfo control facts overflow")?;
    Ok((facts, bytes))
}
fn admit_with(facts_limit: usize, bytes_limit: usize) -> Result<(), &'static str> {
    let (facts, bytes) = cost()?;
    if facts > facts_limit || bytes > bytes_limit {
        Err("native BuildInfo fixed attempt control budget exceeded")
    } else {
        Ok(())
    }
}
pub(super) fn admit() -> Result<(), &'static str> {
    admit_with(MAX_CONTROL_FACTS, MAX_CONTROL_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_grant_admits_exact_typed_control_and_refuses_each_insufficient_dimension() {
        let (facts, bytes) = cost().unwrap();
        assert!(facts > 4 && bytes > 0);
        assert_eq!(admit_with(facts, bytes), Ok(()));
        assert!(admit_with(facts - 1, bytes).is_err());
        assert!(admit_with(facts, bytes - 1).is_err());
        assert_eq!(admit_with(facts + 1, bytes + 1), Ok(()));
        assert_eq!(admit(), Ok(()));
        // Independently include the complete enum, Option, Mutex and terminal;
        // a discriminant/phase addition cannot silently evade this envelope.
        assert!(
            bytes
                >= size_of::<Mutex<Option<ReadPermit>>>()
                    + 2 * size_of::<usize>()
                    + size_of::<NativeBuildInfoFileInputsTerminal>()
        );
        assert!(bytes <= MAX_CONTROL_BYTES && facts <= MAX_CONTROL_FACTS);
    }
}
