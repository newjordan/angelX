# Local ureq 2.12.1 patch

Copied from the existing local Cargo registry; no download or version upgrade.
The origin hashes are recorded in the R04c vendor-origin.json receipt.
Upstream MIT and Apache licenses and source tests are retained.

Request::with_abort_handle attaches a shared AbortHandle to the request's TCP
connection before TLS negotiation (a fresh connection) or before the request is
written (a pooled one), covering request upload and response headers/body.
AbortHandle::abort shuts down that socket. Cancellation before attachment is
remembered and shuts down the socket when attachment occurs. On Linux such a
request may reuse a pooled connection, and its connection returns to the pool
once its body has been read to the end, like any other request's. Before a
socket goes back to the pool, the handle is detached from it under the
handle's lock, and an aborted socket is closed instead of pooled, so a late
abort cannot close a subsequent request. Before each receive on such a request
the socket asks for TCP_QUICKACK: a reused connection sits in delayed-ACK mode,
and a server that leaves Nagle on would otherwise hold each small SSE write
for our delayed ACK (40 ms on Linux). On other platforms, which lack
TCP_QUICKACK, these requests keep a dedicated connection marked
Connection: close that never touches the pool, as all of them did until
2026-10-14.
A pooled connection the server closed while idle is replaced without an
error. The idle probe treats a probe error (an RST received while idle reads
as ECONNRESET) as closed, the same as EOF. A request whose recycled connection
dies before a single byte of response is sent once more on a fresh
connection. That covers a reset, an abort, a broken pipe, or EOF where the
status line should be, which is an idle close crossing the request on the
wire. The resend carries the same bytes, so it applies only to byte-slice and
empty bodies. An aborted request is never sent again.
The cockpit joins its flag-watching worker on every attempt exit. No socket read
timeout or chunk/compression decoder behavior is changed.

One obsolete pair of parentheses in MiddlewareNext was removed to avoid a
current compiler warning. Cargo's explicit example/integration target declarations
are omitted because this vendored dependency is consumed as a library; all
vendored source tests remain intact.

Limit: DNS and TCP connection establishment retain their configured bounds;
there is no attached TCP socket to shut down until connection establishment ends.

TCP keepalive (src/stream.rs, Cargo.toml, src/lib.rs): every connection from
connect_host enables SO_KEEPALIVE with a 30 s idle time, 10 s between probes and
6 probes (TCP_KEEPALIVE on macOS), so a read from a host that has vanished fails
about 90 s after its last byte instead of blocking forever. A live peer's kernel
answers the probes however long the server takes to reply. setsockopt needs the
unix libc dependency and one `unsafe` call, so the crate's `forbid(unsafe_code)`
is now `deny(unsafe_code)` with a single local allow. Other platforms are unchanged.
