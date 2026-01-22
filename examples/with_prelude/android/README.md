## A simple Hello World project using the kuro-prelude

This example demonstrates how a simple C++ project might be built with Kuro
using the prelude.

In the `toolchains` cell, we define two toolchains needed:
`system_cxx_toolchain` and `system_python_bootstrap_toolchain`, both pulled in
from the prelude. The `BUCK` file at the project root contain a `cxx_binary`
target and its `cxx_library` dependency. `.buckconfig` contains the
configuration to set the target platform for the project:

```
[parser]
target_platform_detector_spec = target:root//...->prelude//platforms:default \
  target:prelude//...->prelude//platforms:default \
  target:toolchains//...->prelude//platforms:default
```

## Setup

Run `kuro init --git`.

## Sample commands

To view all targets in the project,

```bash
kuro targets //...
```

To build the main C++ binary,

```bash
kuro build //:main
```

To run the main C++ binary,

```bash
# Should print "Hello from C++!"
kuro run //:main
```
