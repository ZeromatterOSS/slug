## Remote execution integration with BuildBuddy

This project provides a small example of what a project that utilizies
[BuildBuddy](https://www.buildbuddy.io/)'s RE might look like.

In this document, we will go over the key configs used in this setup.

### Relevant configs in .bazelrc

First, the BuildBuddy endpoint and api key should be configured as the
following:

```
build --remote_executor=$BUILDBUDDY_ENDPOINT
build --remote_header=x-buildbuddy-api-key=$BUILDBUDDY_API_KEY
```

`$BUILDBUDDY_ENDPOINT` and `$BUILDBUDDY_API_KEY` are substituted by the
RE client at connection time, so the values flow through from the
shell environment.

### Relevant configs in `ExecutionPlatformInfo`

BuildBuddy takes in a Docker image and OSFamily in its execution platform's
execution properties(`exec_properties`) to select an executor. The execution
platform used in this project `root//platforms:platforms` uses the
`container-image` key to set this up.
