# Multi-Package C++ Example

This example demonstrates a multi-package C++ project built with Kuro,
showing common Bazel patterns.

## Features Demonstrated

- **Multi-package layout**: Library code in `lib/`, application in `app/`, tests in `test/`
- **cc_library**: Compile a shared C++ library with headers
- **cc_binary**: Link an executable against the library
- **cc_test**: Unit tests linked against the library
- **test_suite**: Group tests for easy execution
- **select()**: Platform-specific source selection (`@platforms//os:windows`, etc.)
- **genrule**: Generate files with shell commands
- **exports_files**: Share files across packages with visibility
- **Cross-package deps**: `//lib:math` referenced from `//app` and `//test`
- **.bazelrc**: Build configuration file

## Building

```bash
# Build everything
kuro build //...

# Build specific target
kuro build //app:calculator

# Build with optimization
kuro build //app:calculator --compilation_mode=opt
```

## Running

```bash
kuro run //app:calculator
```

## Testing

```bash
# Run specific test
kuro test //test:math_test

# Run test suite
kuro test //test:all_tests
```

## Querying

```bash
# List dependencies
kuro query "deps(//app:calculator)"

# Configured query (with platform resolution)
kuro cquery "deps(//app:calculator)" --output=json

# Graphviz dependency graph
kuro query "deps(//app:calculator)" --output=graph
```
