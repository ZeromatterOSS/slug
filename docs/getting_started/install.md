---
id: install
title: Installing Kuro
---

import { FbInternalOnly } from 'docusaurus-plugin-internaldocs-fb/internal';

<FbInternalOnly>

## Internal Meta User

For Internal Meta Users, Kuro is already configured and available for you.
Simply cloning the
[`fbsource`](https://www.internalfb.com/wiki/Repositories/fbsource/#cloning)
repository is all that's required to get started; no separate installation steps
for Kuro are necessary.

If you have any issues, please check [here](../../users/faq/meta_installation).

</FbInternalOnly>

## Installing Kuro

The latest set of `kuro` executables can be found under the
[`latest` release page](https://github.com/facebook/kuro/releases/tag/latest).

Additionally, for each bi-monthly release there is a
[dotslash](https://dotslash-cli.com) file that is appropriate for committing to
a repository. This will automatically fetch the correct version and architecture
for each user, and ensures a consistent build environment for each commit in the
repo.

To get started, first install [rustup](https://rustup.rs/), then compile the
`kuro` executable:

```bash
rustup install nightly-2025-08-01
cargo +nightly-2025-08-01 install --git https://github.com/facebook/kuro.git kuro
```

The above commands install `kuro` into a suitable directory, such as
`$HOME/.cargo/bin`, which you should then add to your `$PATH`:

Linux / macOS

```sh
export PATH=$HOME/.cargo/bin:$PATH
```

Windows Powershell

```powershell
$Env:PATH += ";$HOME\.cargo\bin"
```

With Kuro installed, you can build projects with `kuro`!

You can verify that it's working by running `kuro --help`.
