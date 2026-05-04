## Remote execution integration with EngFlow

This project provides a small example of what a project that utilizes
[EngFlow](https://www.engflow.com/)'s RE offering might look like.

In this document, we will go over the key configs used in this setup.

### Relevant configs in .bazelrc

```
build --digest_function=SHA256
build --remote_executor=$ENGFLOW_ENDPOINT
build --tls_client_certificate=$ENGFLOW_CERTIFICATE
```

`$ENGFLOW_ENDPOINT` and `$ENGFLOW_CERTIFICATE` are substituted by the
RE client at connection time, so the values flow through from the
shell environment.

### Relevant configs in `ExecutionPlatformInfo`

EngFlow takes in a Docker image as its execution platform. The execution
platform used in this project `root//platforms:platforms` uses the
`container-image` key to set this up.
