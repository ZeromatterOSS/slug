use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::ptr::NonNull;
#[cfg(test)]
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicU64;
#[cfg(test)]
use std::sync::atomic::Ordering;

pub(super) const WORDS: usize = 64;
pub(super) const BYTES: usize = WORDS * size_of::<AtomicU64>();
pub(super) const MAGIC: u64 = u64::from_le_bytes(*b"SLGOBS01");
pub(super) const VERSION: u64 = 1;
pub(super) const DISABLED: usize = 3;
pub(super) const OVERFLOW: usize = 4;
pub(super) const DROPPED: usize = 5;
pub(super) const ACTIVITY_CLAIM: usize = 6;
pub(super) const PHASE_SELECTOR: usize = 7;
pub(super) const COUNTERS: usize = 8;
pub(super) const PHASE_FRAMES: usize = 14;
pub(super) const ACTIVITY_SELECTOR: usize = 22;
pub(super) const ACTIVITY_FRAMES: usize = 23;
pub(super) const PHASE_FRAME_WORDS: usize = 4;
pub(super) const ACTIVITY_FRAME_WORDS: usize = 14;

#[cfg(test)]
pub(crate) static CALLBACK_GATE: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
pub(crate) fn callback_test_hook(tag: &str) {
    if tag != "BlockForDisable" {
        return;
    }
    if CALLBACK_GATE
        .compare_exchange(1, 2, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        while CALLBACK_GATE.load(Ordering::Acquire) != 3 {
            std::thread::yield_now();
        }
    }
}

#[derive(Copy, Clone)]
#[repr(u16)]
pub(super) enum PhaseStatus {
    Entry = 1,
    Exit = 2,
    Aborted = 3,
}

pub(super) struct Mapping {
    words: NonNull<[AtomicU64; WORDS]>,
}

// The mapping contains only naturally aligned AtomicU64 cells. Its lifetime is
// owned by Mapping, and every cross-thread access uses those atomics.
unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}

impl Mapping {
    pub(super) fn from_fd(fd: OwnedFd) -> io::Result<Self> {
        let raw = fd.as_raw_fd();
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe { libc::fstat(raw, stat.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_size != BYTES as libc::off_t || stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "observer fd is not an exact 512-byte regular file",
            ));
        }
        let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if flags & libc::O_ACCMODE != libc::O_RDWR {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "observer fd is not read-write",
            ));
        }
        let seals = unsafe { libc::fcntl(raw, libc::F_GET_SEALS) };
        if seals < 0 {
            return Err(io::Error::last_os_error());
        }
        let required = libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        if seals & required != required || seals & libc::F_SEAL_WRITE != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "observer fd has invalid seals",
            ));
        }
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                BYTES,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                raw,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        // mmap is page-aligned, BYTES is exactly the array extent, and the file
        // remains fixed-size for the mapping lifetime because of the seals.
        let words = unsafe { NonNull::new_unchecked(ptr.cast::<[AtomicU64; WORDS]>()) };
        let mapping = Self { words };
        if mapping
            .words()
            .iter()
            .any(|word| word.load(std::sync::atomic::Ordering::Relaxed) != 0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "observer mapping is not zero initialized",
            ));
        }
        Ok(mapping)
    }

    pub(super) fn words(&self) -> &[AtomicU64; WORDS] {
        unsafe { self.words.as_ref() }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        let result = unsafe { libc::munmap(self.words.as_ptr().cast(), BYTES) };
        debug_assert_eq!(result, 0, "validated observer mapping must unmap");
    }
}

pub(super) fn clocks() -> (u64, u64, u64) {
    fn read(clock: libc::clockid_t) -> Option<u64> {
        let mut value = std::mem::MaybeUninit::<libc::timespec>::uninit();
        if unsafe { libc::clock_gettime(clock, value.as_mut_ptr()) } != 0 {
            return None;
        }
        let value = unsafe { value.assume_init() };
        u64::try_from(value.tv_sec)
            .ok()?
            .checked_mul(1_000_000_000)?
            .checked_add(u64::try_from(value.tv_nsec).ok()?)
    }
    let wall = read(libc::CLOCK_MONOTONIC);
    let cpu = read(libc::CLOCK_PROCESS_CPUTIME_ID);
    (
        wall.unwrap_or(0),
        cpu.unwrap_or(0),
        u64::from(wall.is_some()) | (u64::from(cpu.is_some()) << 1),
    )
}
