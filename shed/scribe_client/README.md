# Slug Scribe Client

This folder houses Slug's Scribe client, which Slug uses to send information
that powers all of our internal tooling around Slug. Despite this client
serving the needs of Slug, there is no Slug-specific logic contained within
this library.

See
[this post](https://fb.workplace.com/groups/slugprototyping/posts/2829650903999058)
for justification of why this library exists and why it is here. This library is
intended to be an implementation detail of Slug; please do not depend directly
on this library without speaking to us first.

Slug writes to Scribe by interfacing directly with the Thrift service running
on port 1456 on all Meta-owned machines. In prod, the service listening on port
`1456` is
[`scribed`](https://www.internalfb.com/intern/wiki/Documentation/Scribe/), our
production Scribe daemon. In corp, or in non-Linux prod, the service listening
on on this port is
[`scribbled`](https://www.internalfb.com/intern/wiki/Scribe/users/Knowledge_Base/Interacting_with_Scribe_categories/Write_from_Alternative_Environments/Scribble/).
Both services are expected to behave the same, as far as this client is
cooncerned, so this client concerns itself with using the
[ProducerService Thrift API](https://www.internalfb.com/intern/wiki/Scribe/users/Knowledge_Base/Interacting_with_Scribe_categories/producer/producer-service-thrift-api/)
to send messages to Scribe.

Why don't we use the already-existing
[Rust wrapper around the ProducerService Thrift API](https://www.internalfb.com/intern/wiki/Scribe/users/Knowledge_Base/Interacting_with_Scribe_categories/producer/producer-service-thrift-api/#producerservice-thrift-c)?
Unfortunately, this library does not provide a few key features that we need in
Slug:

1. On Linux, this library
   [defaults to using ServiceRouter to construct a client](https://fburl.com/code/15fy5dyk),
   which is not acceptable for Slug (which often runs in environments where
   ServiceRouter cannot function).
2. `ScribeProducer` presents an asynchronous API for pushing messages, which is
   not acceptable for Slug.
3. Slug needs functionality that exists in the C++ Scribe client -
   specifically, intelligent retries and message buffering. The Rust
   ProducerService client does not provide any of these things, and we would
   need to implement them on top of the library anyway.

While this library cannot build in OSS, the code is still available for people
to inspect.
