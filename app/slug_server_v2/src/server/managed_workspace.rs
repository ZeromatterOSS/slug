//! Workspace admission for the private managed listener. The admitted binding
//! must stay stable throughout one command. Ordinary edits within it are allowed.
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;

use super::DaemonRequest;
use super::DaemonResponse;
use super::decode_response;
use super::frame;
use super::read_line;

const MAGIC: [u8; 4] = *b"SWK\x01";
const MAX_PATH_BYTES: usize = 4096;
const FACT_BYTES: usize = 5 * 8;
const TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN: &[u8] = b"shutdown\n";

/// The opener must return a close-on-exec directory descriptor and refuse
/// non-directories, including if the pathname changes during open.
type OpenDirectory = fn(&Path) -> io::Result<File>;

#[doc(hidden)]
pub struct ManagedWorkspace {
    path: PathBuf,
    facts: [u64; 5],
    _directory: File,
    _namespace: File,
    open_directory: OpenDirectory,
}

impl ManagedWorkspace {
    pub fn pin(
        workspace: &Path,
        directory: File,
        open_directory: OpenDirectory,
    ) -> anyhow::Result<Self> {
        let path = workspace.canonicalize()?;
        anyhow::ensure!(
            path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES,
            "managed daemon workspace path exceeds its byte limit"
        );
        let namespace = File::open("/proc/self/ns/mnt")?;
        let facts = binding_facts(&directory, &namespace)?;
        let owner = Self {
            path,
            facts,
            _directory: directory,
            _namespace: namespace,
            open_directory,
        };
        owner.verify_binding()?;
        Ok(owner)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn require_runtime_workspace(&self, workspace: &Path) -> anyhow::Result<()> {
        if workspace != self.path() {
            return Err(AdmissionError::Stale.into());
        }
        self.verify_binding().map_err(Into::into)
    }

    fn verify_binding(&self) -> Result<(), AdmissionError> {
        let result = (|| -> anyhow::Result<bool> {
            if self.path.canonicalize()? != self.path {
                return Ok(false);
            }
            let resolved = (self.open_directory)(&self.path)?;
            let namespace = File::open("/proc/self/ns/mnt")?;
            Ok(binding_facts(&resolved, &namespace)? == self.facts)
        })();
        match result {
            Ok(true) => Ok(()),
            _ => Err(AdmissionError::Stale),
        }
    }

    /// Write this pinned workspace identity using the managed prelude encoding.
    pub fn write_binding(&self, output: &mut impl Write) -> anyhow::Result<()> {
        self.verify_binding()?;
        let path = self.path.as_os_str().as_bytes();
        let mut header = [0; MAGIC.len() + 2 + FACT_BYTES];
        header[..4].copy_from_slice(&MAGIC);
        header[4..6].copy_from_slice(&(path.len() as u16).to_be_bytes());
        for (slot, value) in header[6..].chunks_exact_mut(8).zip(self.facts) {
            slot.copy_from_slice(&value.to_be_bytes());
        }
        output.write_all(&header)?;
        output.write_all(path)?;
        Ok(())
    }

    /// Verify the peer's identity against this pin before admitting its use.
    pub fn require_peer_binding(&self, input: &mut impl Read) -> anyhow::Result<()> {
        let mut first = [0];
        input
            .read_exact(&mut first)
            .map_err(|_| AdmissionError::Protocol)?;
        self.read_expectation(input, first[0]).map_err(Into::into)
    }

    fn send_expectation(&self, stream: &mut UnixStream) -> anyhow::Result<()> {
        // The managed server may still be completing another command. Preserve
        // ordinary queueing on the client while bounding each accepted prelude.
        self.write_binding(stream)?;
        let mut response = [0; MAGIC.len() + 1];
        stream.read_exact(&mut response)?;
        if response[..4] != MAGIC {
            return Err(AdmissionError::Protocol.into());
        }
        match response[4] {
            0 => Ok(()),
            1 => Err(AdmissionError::Protocol.into()),
            2 => Err(AdmissionError::Mismatch.into()),
            3 => Err(AdmissionError::Stale.into()),
            _ => Err(AdmissionError::Protocol.into()),
        }
    }

    pub(super) fn admit(&self, stream: &mut UnixStream) -> io::Result<Ingress> {
        let deadline = Instant::now() + TIMEOUT;
        let mut input = DeadlineReader { stream, deadline };
        let mut first = [0];
        if let Err(error) = input.read_exact(&mut first) {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                return Ok(Ingress::Closed);
            }
            return Err(error);
        }
        if first[0] == SHUTDOWN[0] {
            let mut tail = [0; SHUTDOWN.len() - 1];
            input.read_exact(&mut tail)?;
            return Ok(if tail == SHUTDOWN[1..] {
                Ingress::Shutdown
            } else {
                Ingress::Closed
            });
        }
        let result = self.read_expectation(&mut input, first[0]);
        let code = result.err().map_or(0, AdmissionError::code);
        let mut response = [0; MAGIC.len() + 1];
        response[..4].copy_from_slice(&MAGIC);
        response[4] = code;
        stream.set_write_timeout(Some(TIMEOUT))?;
        stream.write_all(&response)?;
        stream.set_write_timeout(None)?;
        Ok(if code == 0 {
            Ingress::Request(deadline)
        } else {
            Ingress::Closed
        })
    }

    fn read_expectation(&self, input: &mut impl Read, first: u8) -> Result<(), AdmissionError> {
        if first != MAGIC[0] {
            return Err(AdmissionError::Protocol);
        }
        let mut header = [0; MAGIC.len() + 2 + FACT_BYTES];
        header[0] = first;
        input
            .read_exact(&mut header[1..])
            .map_err(|_| AdmissionError::Protocol)?;
        let len = usize::from(u16::from_be_bytes([header[4], header[5]]));
        if header[..4] != MAGIC || len == 0 || len > MAX_PATH_BYTES {
            return Err(AdmissionError::Protocol);
        }
        let mut path = [0; MAX_PATH_BYTES];
        input
            .read_exact(&mut path[..len])
            .map_err(|_| AdmissionError::Protocol)?;
        if &path[..len] != self.path.as_os_str().as_bytes()
            || !header[6..]
                .chunks_exact(8)
                .zip(self.facts)
                .all(|(bytes, value)| bytes == value.to_be_bytes())
        {
            return Err(AdmissionError::Mismatch);
        }
        self.verify_binding()
    }

    pub(super) fn stale_response(&self) -> Option<DaemonResponse> {
        self.verify_binding().err().map(|_| DaemonResponse {
            exit_code: 2,
            stdout: String::new(),
            stderr: b"{\"error\":\"daemon_workspace_binding_changed\"}\n".to_vec(),
            invalidated_files: 0,
            run_launch_plan: None,
        })
    }
}

#[doc(hidden)]
pub struct ManagedDaemonConnection {
    stream: UnixStream,
    _workspace: ManagedWorkspace,
}

impl ManagedDaemonConnection {
    pub fn connect(socket: &Path, workspace: ManagedWorkspace) -> anyhow::Result<Self> {
        let mut stream = UnixStream::connect(socket)
            .with_context(|| format!("connecting to managed daemon socket {}", socket.display()))?;
        workspace.send_expectation(&mut stream)?;
        Ok(Self {
            stream,
            _workspace: workspace,
        })
    }

    pub fn send(mut self, request: DaemonRequest) -> anyhow::Result<DaemonResponse> {
        let command = match &request {
            DaemonRequest::Build(_) => "build",
            DaemonRequest::Run(_) => "run",
            DaemonRequest::Query(_) => "query",
            DaemonRequest::Aquery(_) => "aquery",
            DaemonRequest::Cquery(_) => "cquery",
        };
        let wire = frame::encode(&request, frame::MAX_FRAME_BYTES)
            .with_context(|| format!("serializing {command} request for daemon"))?;
        self.stream.write_all(&wire)?;
        let line = read_line(&mut self.stream)?;
        decode_response(&line, command, command == "run")
    }
}

pub(super) enum Ingress {
    Closed,
    Shutdown,
    Request(Instant),
}

/// Stable prelude refusal kinds. The wire carries a code and never an input
/// pathname, formatter body or underlying filesystem error.
#[derive(Debug)]
enum AdmissionError {
    Protocol,
    Mismatch,
    Stale,
}

impl AdmissionError {
    fn code(self) -> u8 {
        match self {
            Self::Protocol => 1,
            Self::Mismatch => 2,
            Self::Stale => 3,
        }
    }
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Protocol => "invalid managed daemon workspace prelude",
            Self::Mismatch => "managed daemon workspace mismatch",
            Self::Stale => "managed daemon workspace binding changed",
        })
    }
}

impl std::error::Error for AdmissionError {}

pub(super) fn read_request(stream: &mut UnixStream, deadline: Instant) -> anyhow::Result<String> {
    frame::read_line(
        &mut DeadlineReader { stream, deadline },
        frame::MAX_FRAME_BYTES,
    )
}

struct DeadlineReader<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "managed daemon request timed out",
            ));
        }
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(bytes)
    }
}

fn binding_facts(directory: &File, namespace: &File) -> anyhow::Result<[u64; 5]> {
    let root = directory.metadata()?;
    anyhow::ensure!(
        root.is_dir(),
        "managed daemon workspace must be a directory"
    );
    let ns = namespace.metadata()?;
    // Linux procfs reports the mount ID for this exact open directory,
    // including an O_PATH descriptor, without reopening its pathname.
    let info = File::open(format!("/proc/self/fdinfo/{}", directory.as_raw_fd()))?;
    let mut text = String::new();
    info.take(1025).read_to_string(&mut text)?;
    anyhow::ensure!(
        text.len() <= 1024,
        "managed daemon workspace FD metadata exceeds its byte limit"
    );
    let mount = text
        .lines()
        .find_map(|line| line.strip_prefix("mnt_id:"))
        .and_then(|id| id.trim().parse::<u64>().ok())
        .filter(|id| *id != 0)
        .context("missing managed daemon workspace mount identity")?;
    Ok([root.dev(), root.ino(), mount, ns.dev(), ns.ino()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_body_cannot_read_after_its_prelude_deadline() {
        let (mut receiver, mut sender) = UnixStream::pair().unwrap();
        sender.write_all(b"{}\n").unwrap();
        let error = read_request(&mut receiver, Instant::now()).unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().unwrap().kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(read_line(&mut receiver).unwrap(), "{}");
    }
}
