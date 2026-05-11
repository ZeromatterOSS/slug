---
id: install
title: Installing Kuro
---

## Installing Kuro

Kuro is currently build-from-source only. Prebuilt release artifacts may appear
later under the
[`latest` release page](https://github.com/ZeromatterOSS/kuro/releases/tag/latest).

To get started, first install [rustup](https://rustup.rs/), then compile the
`kuro` executable:

```bash
rustup install nightly-2025-08-01
cargo +nightly-2025-08-01 install --git https://github.com/ZeromatterOSS/kuro.git kuro
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
