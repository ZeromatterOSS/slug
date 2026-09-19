//! Internal artifact aliases. Neither transports nor remote results can author links.

use nix::fcntl::readlinkat;
use nix::unistd::symlinkat;

use super::*;

impl Staging {
    pub(in crate::runtime::action_output_staging) fn new_alias(
        workspace: &Path,
        configuration: &SlugConfiguration,
        output: &ActionOutput,
        target: &str,
        reserved: &starlark_map::small_set::SmallSet<&str>,
    ) -> io::Result<Self> {
        if output.kind() != ActionOutputKind::File
            || !Path::new(target).is_absolute()
            || target.contains('\0')
        {
            return Err(io::Error::other(
                "artifact alias requires File output and absolute target",
            ));
        }
        let mut staging = Self::new(
            workspace,
            configuration,
            std::slice::from_ref(output),
            reserved,
        )?;
        let destination = &mut staging.destinations[0];
        // Replace only our owned hidden regular sibling, never the destination.
        if entry(&destination.parent, &destination.name)? != destination.cleanup {
            return Err(io::Error::other("alias staging sibling changed"));
        }
        unlinkat(
            &destination.parent,
            destination.name.as_str(),
            UnlinkatFlags::NoRemoveDir,
        )?;
        destination.cleanup = None;
        symlinkat(target, &destination.parent, destination.name.as_str())?;
        capture_link(destination, target)?;
        destination.created.store(true, Ordering::Relaxed);
        staging.alias = Some(target.to_owned());
        Ok(staging)
    }

    pub(super) fn check_alias(&self) -> io::Result<()> {
        let Some(target) = &self.alias else {
            return Ok(());
        };
        let destination = &self.destinations[0];
        if entry(&destination.parent, &destination.name)? != destination.cleanup
            || readlinkat(&destination.parent, destination.name.as_str())?
                .as_os_str()
                .as_bytes()
                != target.as_bytes()
        {
            return Err(io::Error::other("artifact alias changed during staging"));
        }
        Ok(())
    }
}

// Creation itself returns no descriptor. Claim cleanup only after opening a
// symlink without following it and checking both identity and exact text. A
// foreign directory substituted in this window must never become cleanup-owned.
fn capture_link(destination: &mut Destination, target: &str) -> io::Result<()> {
    let staged = File::from(openat(
        &destination.parent,
        destination.name.as_str(),
        OFlag::O_PATH | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
        Mode::empty(),
    )?);
    if !staged.metadata()?.file_type().is_symlink() {
        return Err(io::Error::other("alias staging sibling is not a symlink"));
    }
    let expected = Entry {
        identity: identity(&staged)?,
        kind: SFlag::S_IFLNK,
    };
    if entry(&destination.parent, &destination.name)? != Some(expected)
        || readlinkat(&destination.parent, destination.name.as_str())?
            .as_os_str()
            .as_bytes()
            != target.as_bytes()
    {
        return Err(io::Error::other(
            "alias staging sibling changed during creation",
        ));
    }
    destination.staged = staged;
    destination.cleanup = Some(expected);
    Ok(())
}

#[cfg(test)]
mod tests;
