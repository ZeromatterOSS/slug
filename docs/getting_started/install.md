---
id: install
title: Installing Slug
---

## Installing Slug

Slug is currently build-from-source only. Prebuilt release artifacts may appear
later under the
[`latest` release page](https://github.com/ZeromatterOSS/slug/releases/tag/latest).

To get started, first install [rustup](https://rustup.rs/), then compile the
`slug` executable:

```bash
rustup install nightly-2025-08-01
cargo +nightly-2025-08-01 install --git https://github.com/ZeromatterOSS/slug.git slug
```

The above commands install `slug` into a suitable directory, such as
`$HOME/.cargo/bin`, which you should then add to your `$PATH`:

Linux / macOS

```sh
export PATH=$HOME/.cargo/bin:$PATH
```

Windows Powershell

```powershell
$Env:PATH += ";$HOME\.cargo\bin"
```

With Slug installed, you can build projects with `slug`!

You can verify that it's working by running `slug --help`.
