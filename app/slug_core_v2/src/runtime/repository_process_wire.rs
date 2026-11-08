//! Private bounded transport for the general repository helper. Correlation
//! bytes authenticate one anonymous channel, never semantic root authority.
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

pub(super) const HELPER_ARGUMENT: &str = "--internal-repository-helper-v1";
pub(super) const CONTROL_FD: i32 = 3;
pub(super) const PLAN_FD: i32 = 4;
pub(super) const RESERVED_PREFIX: &str = "/__slug_repositories__";
pub(super) const MAX_PLAN: usize = 2 * 1024 * 1024;
pub(super) const MAX_ITEMS: usize = 8192;
// Includes encoded+decoded overlap, CString/vector headers and cursor scratch.
// Native callers must reserve this allowance before invoking the codec/worker.
pub(super) const PLAN_RETAINED_BYTES: usize = 8 * 1024 * 1024;
const PLAN_MAGIC: &[u8; 8] = b"SLUG425P";
const FRAME_MAGIC: &[u8; 8] = b"SLUG425C";
const FRAME_LEN: usize = 98;
const SEALS: i32 = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid repository helper protocol",
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum RunMode {
    InitializeArchive = 1,
    FirstExecute = 2,
    ContinueExecute = 3,
}
impl TryFrom<u8> for RunMode {
    type Error = io::Error;
    fn try_from(v: u8) -> io::Result<Self> {
        match v {
            1 => Ok(Self::InitializeArchive),
            2 => Ok(Self::FirstExecute),
            3 => Ok(Self::ContinueExecute),
            _ => Err(invalid()),
        }
    }
}
pub(super) const MAX_MIRRORS: usize = 128;
pub(super) const MAX_MIRROR_TEXT: usize = 64 * 1024;
pub(super) struct MirrorEntry {
    pub name: CString,
    pub fd: i32,
    pub device: u64,
    pub inode: u64,
    pub mode: u32,
    pub link: Option<CString>,
}
pub(super) struct RunPlan {
    pub mode: RunMode,
    pub display: CString,
    pub workspace: CString,
    pub cwd: CString,
    pub argv: Vec<CString>,
    pub env: Vec<CString>,
    pub mirrors: Vec<MirrorEntry>,
    pub nonce: [u8; 16],
    pub index: u64,
    pub timeout_millis: u64,
}
fn absolute(value: &[u8]) -> bool {
    value.starts_with(b"/")
        && value.len() <= 4096
        && (value == b"/"
            || value[1..]
                .split(|b| *b == b'/')
                .all(|s| !s.is_empty() && s != b"." && s != b".." && s.len() <= 255))
}
fn beneath(value: &[u8], parent: &[u8]) -> bool {
    value == parent
        || value
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with(b"/"))
}
impl RunPlan {
    pub fn validate(&self) -> io::Result<()> {
        let display = self.display.as_bytes();
        if !absolute(display)
            || display == RESERVED_PREFIX.as_bytes()
            || !beneath(display, RESERVED_PREFIX.as_bytes())
            || display[RESERVED_PREFIX.len() + 1..]
                .split(|b| *b == b'/')
                .count()
                > 16
            || !absolute(self.workspace.as_bytes())
            || !absolute(self.cwd.as_bytes())
            || !beneath(self.cwd.as_bytes(), display)
            || !(1..=27000).contains(&self.timeout_millis)
            || self.argv.len() > MAX_ITEMS
            || self.env.len() > MAX_ITEMS
            || (self.mode == RunMode::InitializeArchive
                && (!self.argv.is_empty() || !self.env.is_empty() || self.index != 0))
            || (self.mode != RunMode::InitializeArchive && self.argv.is_empty())
        {
            return Err(invalid());
        }
        let mut names = Vec::new();
        names
            .try_reserve_exact(self.env.len())
            .map_err(|_| invalid())?;
        for item in &self.env {
            let value = item.as_bytes();
            let split = value.iter().position(|b| *b == b'=').ok_or_else(invalid)?;
            // Empty names are accepted by the immutable API and passed literally.
            names.push(&value[..split]);
        }
        names.sort_unstable();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid());
        }
        if self.mirrors.len() > MAX_MIRRORS {
            return Err(invalid());
        }
        let mut text = 0usize;
        for (i, row) in self.mirrors.iter().enumerate() {
            let name = row.name.as_bytes();
            if name.is_empty()
                || name.len() > 255
                || name.contains(&b'/')
                || name == b"."
                || name == b".."
                || [
                    b"proc".as_slice(),
                    b"dev",
                    b"tmp",
                    &RESERVED_PREFIX.as_bytes()[1..],
                ]
                .contains(&name)
                || self.mirrors[..i].iter().any(|prior| prior.name == row.name)
                || row.inode == 0
                || row.fd != 6 + i as i32
            {
                return Err(invalid());
            }
            let kind = row.mode & libc::S_IFMT;
            if !matches!(kind, libc::S_IFDIR | libc::S_IFREG | libc::S_IFLNK)
                || (kind == libc::S_IFLNK) != row.link.is_some()
            {
                return Err(invalid());
            }
            text = text
                .checked_add(name.len())
                .and_then(|n| n.checked_add(row.link.as_ref().map_or(0, |v| v.as_bytes().len())))
                .ok_or_else(invalid)?;
            if text > MAX_MIRROR_TEXT {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(MAX_PLAN).map_err(|_| invalid())?;
        bytes.extend_from_slice(PLAN_MAGIC);
        bytes.push(1);
        bytes.push(self.mode as u8);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes.extend_from_slice(&self.timeout_millis.to_le_bytes());
        fn item(out: &mut Vec<u8>, value: &[u8]) -> io::Result<()> {
            let end = out
                .len()
                .checked_add(4)
                .and_then(|n| n.checked_add(value.len()))
                .filter(|n| *n <= MAX_PLAN)
                .ok_or_else(invalid)?;
            out.extend_from_slice(
                &u32::try_from(value.len())
                    .map_err(|_| invalid())?
                    .to_le_bytes(),
            );
            out.extend_from_slice(value);
            debug_assert_eq!(out.len(), end);
            Ok(())
        }
        for value in [&self.display, &self.workspace, &self.cwd] {
            item(&mut bytes, value.as_bytes())?;
        }
        for values in [&self.argv, &self.env] {
            if bytes.len() > MAX_PLAN - 4 {
                return Err(invalid());
            }
            bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for value in values {
                item(&mut bytes, value.as_bytes())?;
            }
        }
        if bytes.len() > MAX_PLAN - 4 {
            return Err(invalid());
        }
        bytes.extend_from_slice(&(self.mirrors.len() as u32).to_le_bytes());
        for row in &self.mirrors {
            item(&mut bytes, row.name.as_bytes())?;
            if bytes.len() > MAX_PLAN - 24 {
                return Err(invalid());
            }
            bytes.extend_from_slice(&row.device.to_le_bytes());
            bytes.extend_from_slice(&row.inode.to_le_bytes());
            bytes.extend_from_slice(&row.mode.to_le_bytes());
            bytes.extend_from_slice(&row.fd.to_le_bytes());
            item(&mut bytes, row.link.as_ref().map_or(&[], |v| v.as_bytes()))?;
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() > MAX_PLAN {
            return Err(invalid());
        }
        struct Cursor<'a>(&'a [u8]);
        impl<'a> Cursor<'a> {
            fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
                if self.0.len() < n {
                    return Err(invalid());
                }
                let (a, b) = self.0.split_at(n);
                self.0 = b;
                Ok(a)
            }
            fn number(&mut self) -> io::Result<usize> {
                Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize)
            }
            fn string(&mut self) -> io::Result<CString> {
                let n = self.number()?;
                CString::new(self.take(n)?).map_err(|_| invalid())
            }
            fn array(&mut self) -> io::Result<Vec<CString>> {
                let n = self.number()?;
                if n > MAX_ITEMS || n > self.0.len() / 4 {
                    return Err(invalid());
                }
                let mut items = Vec::new();
                items.try_reserve_exact(n).map_err(|_| invalid())?;
                for _ in 0..n {
                    items.push(self.string()?);
                }
                Ok(items)
            }
        }
        let mut c = Cursor(bytes);
        if c.take(8)? != PLAN_MAGIC || c.take(1)? != [1] {
            return Err(invalid());
        }
        let mode = RunMode::try_from(c.take(1)?[0])?;
        let nonce = c.take(16)?.try_into().unwrap();
        let index = u64::from_le_bytes(c.take(8)?.try_into().unwrap());
        let timeout_millis = u64::from_le_bytes(c.take(8)?.try_into().unwrap());
        let display = c.string()?;
        let workspace = c.string()?;
        let cwd = c.string()?;
        let argv = c.array()?;
        let env = c.array()?;
        let count = c.number()?;
        if count > MAX_MIRRORS || count > c.0.len() / 32 {
            return Err(invalid());
        }
        let mut mirrors = Vec::new();
        mirrors.try_reserve_exact(count).map_err(|_| invalid())?;
        for _ in 0..count {
            let name = c.string()?;
            let device = u64::from_le_bytes(c.take(8)?.try_into().unwrap());
            let inode = u64::from_le_bytes(c.take(8)?.try_into().unwrap());
            let mode = u32::from_le_bytes(c.take(4)?.try_into().unwrap());
            let fd = i32::from_le_bytes(c.take(4)?.try_into().unwrap());
            let link = c.string()?;
            let link = if mode & libc::S_IFMT == libc::S_IFLNK {
                Some(link)
            } else {
                if !link.as_bytes().is_empty() {
                    return Err(invalid());
                }
                None
            };
            mirrors.push(MirrorEntry {
                name,
                device,
                inode,
                mode,
                link,
                fd,
            });
        }
        let plan = Self {
            mode,
            nonce,
            index,
            timeout_millis,
            display,
            workspace,
            cwd,
            argv,
            env,
            mirrors,
        };
        if !c.0.is_empty() {
            return Err(invalid());
        }
        plan.validate()?;
        Ok(plan)
    }
    pub fn sealed_file(&self) -> io::Result<File> {
        let bytes = self.encode()?;
        let fd = unsafe {
            libc::memfd_create(
                c"slug-repository-plan".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(&bytes)?;
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, SEALS) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(file)
    }
    pub fn read_sealed(file: &File) -> io::Result<Self> {
        use std::os::unix::fs::FileExt;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.len() > MAX_PLAN as u64
            || unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) } != SEALS
        {
            return Err(invalid());
        }
        let size = metadata.len() as usize;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|_| invalid())?;
        bytes.resize(size, 0);
        file.read_exact_at(&mut bytes, 0)?;
        let mut extra = [0];
        if file.read_at(&mut extra, size as u64)? != 0 {
            return Err(invalid());
        }
        Self::decode(&bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Kind {
    Hello = 1,
    Ready = 2,
    Incoming = 3,
    Quota = 4,
    Successor = 5,
    Ack = 6,
    ExecError = 7,
    Done = 8,
    SetupError = 9,
    HelloAck = 10,
}
impl TryFrom<u8> for Kind {
    type Error = io::Error;
    fn try_from(v: u8) -> io::Result<Self> {
        match v {
            1 => Ok(Self::Hello),
            2 => Ok(Self::Ready),
            3 => Ok(Self::Incoming),
            4 => Ok(Self::Quota),
            5 => Ok(Self::Successor),
            6 => Ok(Self::Ack),
            7 => Ok(Self::ExecError),
            8 => Ok(Self::Done),
            9 => Ok(Self::SetupError),
            10 => Ok(Self::HelloAck),
            _ => Err(invalid()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Frame {
    pub kind: Kind,
    pub nonce: [u8; 16],
    pub index: u64,
    pub words: [u64; 8],
}
impl Frame {
    pub fn new(kind: Kind, nonce: [u8; 16], index: u64) -> Self {
        Self {
            kind,
            nonce,
            index,
            words: [0; 8],
        }
    }
    pub fn matches(&self, plan: &RunPlan) -> bool {
        self.nonce == plan.nonce && self.index == plan.index
    }
    fn bytes(&self) -> [u8; FRAME_LEN] {
        let mut b = [0; FRAME_LEN];
        b[..8].copy_from_slice(FRAME_MAGIC);
        b[8] = 1;
        b[9] = self.kind as u8;
        b[10..26].copy_from_slice(&self.nonce);
        b[26..34].copy_from_slice(&self.index.to_le_bytes());
        for (i, v) in self.words.iter().enumerate() {
            b[34 + 8 * i..42 + 8 * i].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    pub fn decode(b: &[u8]) -> io::Result<Self> {
        if b.len() != FRAME_LEN || &b[..8] != FRAME_MAGIC || b[8] != 1 {
            return Err(invalid());
        }
        let mut f = Self::new(
            Kind::try_from(b[9])?,
            b[10..26].try_into().unwrap(),
            u64::from_le_bytes(b[26..34].try_into().unwrap()),
        );
        for (i, v) in f.words.iter_mut().enumerate() {
            *v = u64::from_le_bytes(b[34 + 8 * i..42 + 8 * i].try_into().unwrap());
        }
        Ok(f)
    }
}
#[repr(C, align(16))]
struct Control([u8; 128]);
pub(super) fn check_socket(fd: BorrowedFd<'_>) -> io::Result<()> {
    let mut domain = 0i32;
    let mut domain_len = std::mem::size_of_val(&domain) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_DOMAIN,
            (&mut domain as *mut i32).cast(),
            &mut domain_len,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if domain != libc::AF_UNIX || domain_len as usize != std::mem::size_of_val(&domain) {
        return Err(invalid());
    }
    for peer in [false, true] {
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of_val(&address) as libc::socklen_t;
        let result = unsafe {
            if peer {
                libc::getpeername(
                    fd.as_raw_fd(),
                    (&mut address as *mut libc::sockaddr_un).cast(),
                    &mut length,
                )
            } else {
                libc::getsockname(
                    fd.as_raw_fd(),
                    (&mut address as *mut libc::sockaddr_un).cast(),
                    &mut length,
                )
            }
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if address.sun_family as i32 != libc::AF_UNIX
            || length as usize != std::mem::size_of::<libc::sa_family_t>()
        {
            return Err(invalid());
        }
    }
    let mut kind = 0i32;
    let mut len = std::mem::size_of_val(&kind) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut i32).cast(),
            &mut len,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if kind != libc::SOCK_SEQPACKET || len as usize != std::mem::size_of_val(&kind) {
        return Err(invalid());
    }
    Ok(())
}
pub(super) fn send(fd: BorrowedFd<'_>, frame: &Frame, rights: &[BorrowedFd<'_>]) -> io::Result<()> {
    if rights.len() > 3 {
        return Err(invalid());
    }
    let bytes = frame.bytes();
    let mut io = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    let mut control = Control([0; 128]);
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut io;
    msg.msg_iovlen = 1;
    if !rights.is_empty() {
        msg.msg_control = control.0.as_mut_ptr().cast();
        msg.msg_controllen =
            unsafe { libc::CMSG_SPACE((rights.len() * std::mem::size_of::<i32>()) as u32) }
                as usize;
        let c = unsafe { libc::CMSG_FIRSTHDR(&msg) };
        unsafe {
            (*c).cmsg_level = libc::SOL_SOCKET;
            (*c).cmsg_type = libc::SCM_RIGHTS;
            (*c).cmsg_len =
                libc::CMSG_LEN((rights.len() * std::mem::size_of::<i32>()) as u32) as usize;
            for (i, right) in rights.iter().enumerate() {
                libc::CMSG_DATA(c)
                    .cast::<i32>()
                    .add(i)
                    .write_unaligned(right.as_raw_fd());
            }
        }
    }
    loop {
        let n = unsafe {
            libc::sendmsg(
                fd.as_raw_fd(),
                &msg,
                libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
            )
        };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        return if n as usize == bytes.len() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
}
/// Nonblocking receive. Every delivered SCM_RIGHTS descriptor is owned before
/// validation, including an oversized/truncated/malformed/unexpected packet.
pub(super) fn receive(fd: BorrowedFd<'_>) -> io::Result<(Frame, Vec<File>)> {
    let mut bytes = [0; FRAME_LEN + 1];
    let mut io = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut control = Control([0; 128]);
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut io;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = unsafe { libc::CMSG_LEN(3 * std::mem::size_of::<i32>() as u32) } as usize;
    let n = loop {
        let n = unsafe {
            libc::recvmsg(
                fd.as_raw_fd(),
                &mut msg,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if n < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        break n as usize;
    };
    let mut descriptors: [Option<OwnedFd>; 3] = std::array::from_fn(|_| None);
    let mut count = 0;
    let mut bad = false;
    let mut c = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    while !c.is_null() {
        unsafe {
            let header = &*c;
            let base = libc::CMSG_LEN(0) as usize;
            if header.cmsg_level == libc::SOL_SOCKET
                && header.cmsg_type == libc::SCM_RIGHTS
                && header.cmsg_len >= base
            {
                let len = header.cmsg_len - base;
                if len % std::mem::size_of::<i32>() != 0 {
                    bad = true;
                }
                for i in 0..len / std::mem::size_of::<i32>() {
                    let raw = libc::CMSG_DATA(c).cast::<i32>().add(i).read_unaligned();
                    let owner = OwnedFd::from_raw_fd(raw);
                    if count < descriptors.len() {
                        descriptors[count] = Some(owner);
                        count += 1;
                    } else {
                        bad = true;
                        drop(owner);
                    }
                }
            } else {
                bad = true;
            }
            c = libc::CMSG_NXTHDR(&msg, c);
        }
    }
    if bad || count > 3 || msg.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0 {
        return Err(invalid());
    }
    let frame = Frame::decode(&bytes[..n])?;
    let mut files = Vec::with_capacity(count);
    for owner in descriptors.iter_mut().take(count) {
        files.push(File::from(owner.take().unwrap()));
    }
    Ok((frame, files))
}

#[cfg(test)]
#[path = "repository_process_wire/tests.rs"]
mod tests;
