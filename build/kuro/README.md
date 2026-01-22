# Easy kuro builds for Facebook projects

This directory contains kuro targets designed to simplify kuro builds of
Meta open source projects.

The most notable target is `//build/kuro/install_deps`, which will attempt to
discover and install necessary third party packages from apt / dnf / etc.
See the "repos" directory for the currently supported platforms.

## Deployment

This directory is copied literally into a number of different Facebook open
source repositories.  Any change made to code in this directory will be
automatically be replicated by our open source tooling into all GitHub hosted
repositories that use `kuro`.  Typically this directory is copied
into the open source repositories as `build/kuro/`.
