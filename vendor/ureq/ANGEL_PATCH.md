# Local ureq 2.12.1 patch

Copied from the existing local Cargo registry; no download or version upgrade.
The origin hashes are recorded in the R04c vendor-origin.json receipt.
Upstream MIT and Apache licenses and source tests are retained.

Request::with_abort_handle attaches a shared AbortHandle to a dedicated TCP
connection before TLS negotiation, request upload, and response headers/body.
AbortHandle::abort shuts down that socket. Cancellation before attachment is
remembered and shuts down the socket when attachment occurs. These requests
neither borrow from nor return to the connection pool, so a late abort cannot
close a subsequent request. Other requests retain upstream pooling behavior.
The cockpit joins its flag-watching worker on every attempt exit. No socket read
timeout or chunk/compression decoder behavior is changed.

One obsolete pair of parentheses in MiddlewareNext was removed to avoid a
current compiler warning. Cargo's explicit example/integration target declarations
are omitted because this vendored dependency is consumed as a library; all
vendored source tests remain intact.

Limit: DNS and TCP connection establishment retain their configured bounds;
there is no attached TCP socket to shut down until connection establishment ends.
