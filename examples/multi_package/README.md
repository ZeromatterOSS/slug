# Multi-Package C++ Example

This example demonstrates a multi-package C++ project built with Slug,
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
slug build //...

# Build specific target
slug build //app:calculator

# Build with optimization
slug build //app:calculator --compilation_mode=opt
```

## Running

```bash
slug run //app:calculator
```

## Testing

```bash
# Run specific test
slug test //test:math_test

# Run test suite
slug test //test:all_tests
```

## Querying

```bash
# List dependencies
slug query "deps(//app:calculator)"

# Configured query (with platform resolution)
slug cquery "deps(//app:calculator)" --output=json

# Graphviz dependency graph
slug query "deps(//app:calculator)" --output=graph
```
