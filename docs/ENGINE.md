# The engine: how angelX games are played together

This is the foundation every angelX game is played on: the Delve, and the
games that come after it. It replaces the Delve's picture stream and the
patches layered on top of it. The decisions are fixed here so the pieces can
be built in parallel against one contract. Anything here that turns out to be
wrong gets changed here first, then in code.

## What it must do

These come from the operator (2026-10-13). None of them is optional.

1. **Every desktop now, phones next.** Students in browsers on Mac and
   Windows (Chrome, Edge, Firefox, Safari) play together and with angelX
   terminals. The browser is the main client, not a fallback. Phones come
   later: nothing built now may stand in their way (input goes through
   `Hands`, the game's touch layout is already in the ABI, WebGL1 and
   canvas fallbacks stay), but touch controls and phone tuning wait.
2. **One link.** The host shares one link, or its QR code. It works on
   campus wifi, on cellular, and across a network change mid-game.
3. **Up to 200 players** in one session, for a game built for crowds (not the
   Delve, which seats a party).
4. **It feels local.** Each client draws at its display's own rate, and your
   own actions answer on the next frame, never after a round trip.

## What is wrong today, and why patches cannot fix it

- **Browser friends are watching video.** The host paints every browser
  seat's picture in software and PNG-encodes it (`together_guest.rs`
  `paint_frames` and `socket_wire`). Every key waits a round trip before a
  picture can show it. Since 07e065ff the page has faked its own knight with
  slides, eases and uncover strips, and the host has led that knight by the
  round trip. Each of these patched what the one before it broke.
- **Input runs on the wall clock.** Held keys live on leases (250 and
  500 ms) and are renewed every 80 ms. A heading change is back-dated inside
  the tick (`sub_tick`). Taps are latched with how long they were down. So a
  tick is not a pure function of state and inputs, and no client can
  re-simulate it.
- **Copies scale with seats.** One tick makes 1 to 11 whole `Run` clones on
  the sim thread. The painters make 10 to 18 more, plus up to 8 full
  serialize and deserialize round trips. Live JSON is encoded every tick, and
  a checkpoint `fsync`s on the sim thread every 3 s.
- **The server caps out at four.** It is a thread per request, with at most
  4 connections in flight, 4 painter threads and `MAX_PLAYERS` = 4.
- **A native friend cannot cross Funnel.** `/net` is plain HTTP on raw TCP,
  so a joined angelX behind an https link retries forever.
- **The rules live in the renderer's tree.** The kingdom's geography that the
  rules walk on is built from renderer modules (`map.rs` re-exports
  `overworld::kingdom`). `Run` also carries presentation (shake, sparks,
  banners, the kingdom camera), and some rules read it back.

## The shape

One authoritative server steps the game at a fixed tick. **Every player is a
client that draws its own screen** from replicated state, and that includes
the host's own terminal. No picture ever crosses the network.

```
   desktop browser                 angelX terminal (friend)       host terminal
   wasm client                       native client                 in-process client
   ├ input → commands               ├ same                        ├ reads snapshots directly
   ├ snapshots → interp + predict   ├ same                        └ renders locally
   └ renders locally (WebGL)        └ renders locally
            │  wss (one protocol for both)   │
            └──────────────┬─────────────────┘
                     angel-serve (host)
          sessions · links · per-client replication · HTTP assets
                           │
                     game server step (fixed tick, pure)
```

### Crates (`crates/`, a workspace; the cockpit depends on them by path)

| crate | target | what it is |
|---|---|---|
| `angel-net` | native + wasm | protocol messages, byte codec, input commands, replication (document and entity), clock and interpolation, prediction helper. No IO, no threads. |
| `angel-serve` | native | the server: WebSocket and HTTP on blocking sockets, in-process connections for the host, sessions, links, seats, per-client writers, asset serving, telemetry. And the native client's side: `WsClient` (`wss://` behind the `tls` feature) and `NativeLink`. |
| `angel-web` | wasm + JS | the browser runtime: the JS shell (socket, keyboard and gamepad input, touch later, frame loop, WebGL present, audio, reconnect) and the wasm ABI every game exports. |
| `angel-bots` | native | load generator: N headless clients that join and play, with measurements. |
| `angel-delve` | native + wasm | the Delve: rules, renderer, its replication schema and prediction. Moved out of the cockpit. |
| `angel-muster` | native + wasm | the crowd test game: 200 knights on one field. It proves the engine at scale before any real 200-player game exists. |

The cockpit keeps the angelX parts: the realm save, settling into the realm,
the terminal UI, and hosting a session through `angel-serve`.

## Ticks and input commands

The wall-clock hacks are replaced by one standard model.

- **Fixed tick.** Each game has a fixed tick rate (the Delve: 30 Hz). The
  step is a pure function: `step(state, commands_for_this_tick) -> events`.
  No `Instant`, no file, no env, no lease inside it.
- **Commands.** A client samples its input at the same fixed rate on its own
  clock and sends one command per client tick: `Cmd { seq: u32, held, presses }`.
  - `held` is the buttons and axes held at the sample.
  - `presses` are the edges seen since the last sample, so a tap shorter than
    a tick is never lost.
  - The keyboard's stick (from the direction buttons) counts every direction
    held at any moment since the last sample. A tap shorter than a tick
    walks that tick, and a key let go mid-tick walks the tick out. So what a
    client drew a part-tick ahead (see "Own knight on the next frame") is
    exactly what the server steps, and nothing drawn is taken back. An
    analog stick gives where it leans at the sample.
  - The payload is game-defined bytes: the Delve sends its own `Input`, and
    the muster sends 4 bytes.
- **The server's per-seat queue** (`angel_net::cmd::SeatInputs`). Each tick
  the server takes one command per seat, in seq order, and plays each seq on
  one tick only: a key held for ten client ticks walks ten server ticks, and
  the client's own replay of its commands lands where the server's does.
  The link is never even, so the queue plays commands out a little behind
  their arrival:
  - **The cushion.** A seat's first command is taken once 2 more are queued
    behind it (or once it has waited 2 ticks, if no more come). From then
    on a command up to 2 ticks (66 ms) late is still on time. Others see
    this seat 66 ms later; its own player sees nothing of it, since their
    knight is drawn from their own commands. A seat played in process (the
    host's own, a couch partner's) has no link to cushion against, and no
    cushion: each command is taken on the tick after it was sent.
  - **A guess.** A tick whose seq has not arrived plays the seat's held keys
    again (never its presses) and counts as that seq: the ack moves on. The
    late command is not played again. Its presses go into the next tick,
    and its held keys are what the next guess plays. A guess is wrong only
    when the keys changed on exactly that tick, and then by one tick.
  - **A stall.** After 4 guesses in a row the link has stopped: the seat
    plays its held keys without counting seqs, then fills its cushion again
    from the newest commands (what piled up folds into them, presses kept).
  - **A drain.** A queue that has sat past its cushion for 30 ticks (a
    client clock running fast) drops one still command (nothing held,
    nothing pressed): a knight standing still loses nothing by it.
  - **The cap.** Past 8 queued, the oldest folds into the next, presses
    kept.
  - **No command for 15 ticks:** the seat goes neutral.
  - The server counts what each queue did (`commands taken / guessed /
    wrong / late / waited / drained / folded` in its periodic log).
  - That is the whole input model. No leases, no `sub_tick`, no renewals, no
    pending taps.
- **Acks.** Every snapshot carries, per seat, the `seq` of the last command
  it consumed (taken or guessed). A client predicts its own entity by
  replaying its commands after that `seq` through the game's own movement
  code. The Delve's `Run::predict` steps the very functions the sim steps
  a knight's going with (`Hero::own_keys`, `Hero::own_step`): the roll's
  start, direction, ease and rearm, a shield's or wall's pace, a spin, a
  dagger's blink, the ground's pace. Where something other than the
  knight's keys moves it (a ride, the privy and pit stop, a Bladewind, a
  Ravage's throw, a side-on hall, Turbo's doubled ticks), the client draws
  its own knight from the server's ticks, blended like everyone else. It then walks `partial` of a tick along the hands held
  now (`View::hands`), so a key shows on the next display frame. When the
  server's answer differs, the client corrects smoothly over a fixed
  100 ms.
- **Others are interpolated.** Everyone else is drawn a fixed **100 ms**
  behind the estimated server time, between the two snapshots that bracket
  it. Server time is estimated from the least-delayed snapshot arrival over
  a sliding window.

## Replication

The stream is a WebSocket over TCP: reliable and ordered. So **a client's
baseline is what was last written to it.** There is no ack ring of
baselines. If a client falls behind, its writer skips to the newest tick and
diffs against what it last wrote. A broken connection resumes with a whole
state.

Two modes, chosen by the game:

- **Document mode** (a party of ≤ 8, the Delve). The state is one serde
  struct, turned into a value tree once per tick. Each client gets the tree
  filtered by an interest function (the seat's room), then diffed against
  what it last received. This is today's `together_net` with acks dropped
  from the baseline. Server-private fields (`rng`, `next_id`) are
  never sent.
  - The body (`angel_serve::DocRep`): a whole snapshot is the tree as a
    `doc::wire` value; a patch is a `doc::wire` patch; an empty body (not
    whole) means nothing changed. A whole snapshot starts the client's
    name table over.
- **Entity mode** (crowds, up to 200+). The game lists entities: a `u32` id,
  a kind, a position for interest, and its state encoded in a few bytes.
  - The server stamps an entity's version on the tick its bytes changed.
  - For each client the writer keeps what that client holds (id to
    version). It picks the entities inside the client's view rect plus a
    margin from a uniform grid, and sends:
    - new ones whole;
    - changed ones as the bytes that changed since what that client holds
      (a mask of which, then those bytes): a walking knight changes the low
      bytes of its place, two bytes and not five;
    - unchanged ones not at all;
    - ones that left as a despawn.
  - A thing has two sets of bytes: `spawn` (fixed at its appearance: a
    colour, a name), sent once to each client that sees it, and `state`
    (a place, a pose), at most 24 bytes, sent when it changes.
  - A per-snapshot byte budget (fixed: 1,200 bytes) is filled in priority
    order: nearer first, then longest unsent. What doesn't fit carries to
    the next snapshot.
    - Priority is the ticks a thing has waited times its band's weight: the
      nearest quarter of the view counts 4, then 3, 2, 1. A thing the client
      has never seen counts 8 ticks more than it waited. So near things go
      first and nothing far is starved.
    - The client's own entity is always first: it predicts from it.
    - A snapshot that left out a change to something the client holds says
      so (`DEFERRED`). Otherwise the client takes everything unmentioned as
      unchanged through that tick, so a knight that stood still and then
      walked is drawn standing until it walked.
  - Interest has a margin, then a keep: a thing is sent when it comes into
    view plus the margin, and let go only past a further keep distance, so
    one pacing along the edge is not sent whole again and again.
  - Global state (the clock, scores, a phase) goes as a small document beside
    the entities. It goes first and is never held back, so it must stay small.

Events (sounds, cues, voice lines, notices) ride in the same stream as a list
tagged with their tick. They are never polled from a separate endpoint.

## Transport

- **WebSocket, binary, for every client.** Browsers and the native angelX
  client speak the same thing on `GET /ws`. Over https links it is wss, so
  it crosses Funnel, relays and campus proxies on port 443. Raw TCP framing
  and the separate `/net`, `/play`, `/stream` and `/frames` routes go away.
- **Server threads.** One accept thread. Each connection gets a reader thread
  and a writer thread on blocking sockets, with each socket split by
  `try_clone`. This is portable to Linux, macOS and Windows hosts, simple to
  reason about, and fine to a few hundred connections (fixed cap: 256
  players + 32 spectators).
  - Each writer builds its own client's snapshot from the shared newest tick,
    so encoding spreads across cores by itself.
  - A slow client's writer blocks on its own socket and skips ticks. It never
    slows the sim or anyone else.
  - A writer also holds back while its client is behind on what it was
    sent. The client says which tick it has applied in its commands
    (`applied`). The writer lets at most a window of bytes be unapplied,
    and the window moves like a congestion window, driven by delay.
    - It starts at one full snapshot (1,200 bytes).
    - Each snapshot is timed from being written to being heard applied,
      against the least such delay of the last 5 s.
    - On time (within 250 ms of that least delay), the window grows by what
      the snapshot carried, doubling each round trip. After the client's
      first late snapshot, it grows by 150 bytes per window's worth instead.
    - Late, it halves, but never below one full snapshot.
    - So a fast client, near or far, opens to its round trip's worth and
      is not held again. A link slower than the game's stream holds about
      one snapshot in flight, and is sent fresh state rather than a queue
      of stale state sitting in socket buffers.
    - The window grows on evidence that nothing queued, never on a measured
      rate, so holding back cannot talk a fast client's window down.
- **Limits.** Fixed numbers, all per connection:
  - client message ≤ 1 KB;
  - ≤ 120 messages a second;
  - handshake ≤ 4 KB within 5 s, and the `Hello` within 5 s after it;
  - ping every 1 s, closed after 10 s of silence;
  - at most 512 connections not yet past their `Hello` (page loads, and
    sockets waiting for their hello); more are answered 503 at the door,
    so half-open sockets cannot pile up threads without end. A class of
    200 phones scanning the code at once holds two or three each;
  - a kept-alive HTTP connection waits 2 s for its next request, so idle
    page loads do not hold places a newcomer needs.
- **No Origin check** on `/ws`. The join key (or a seat token) is the
  authority, and the static client may later come from a CDN or a relay on
  another origin.
- A connection whose seat was taken up by another page (a reload, a second
  tab) is closed with code 4000. A shell should not reconnect on it, or two
  tabs would take the seat in turns.
- **HTTP**, from the same listener:
  - `GET /` serves the shell page;
  - `GET /a/<hash>.<ext>` serves the client JS, the game's wasm, and sfx and
    music, named by content hash. Each is sent with
    `Cache-Control: immutable` and precompressed as gzip, since every
    browser takes it.
  - Every response has a strict CSP.

## Sessions and the link

- **One link a session:** `<door>/#<join-key>`.
  - The join key is 128 random bits in base32. It sits in the fragment, so
    it never appears in a request line or a log.
  - Anyone holding it gets a seat until the game's capacity is reached, and
    a spectator place after that. The host can lock the session or make a
    new key.
  - The key is read in any case. The link is upper-cased for its QR code
    (a door with no path is case-blind up to the fragment), so the code
    uses QR's alphanumeric mode: a LAN link fits version 3, 29 modules.
- **Seats resume.** On joining, a client gets a seat token (128 bits). The
  browser keeps it in `localStorage` for this session. A reconnect within
  120 s takes back the same seat: same knight, same inventory. That covers a
  phone moving from wifi to cellular, a locked screen, and a reload.
  - A token works when the session is locked and after a new key: it is
    that player's seat.
  - A kicked player's token is refused from then on. To keep them out,
    the host kicks and makes a new key (they still hold the link).
- **Names.** The page asks for a name once and remembers it. There are no
  accounts.
- **The host shows the link and its QR code** in the terminal, drawn in half
  blocks. Students scan it off the projector.

## The server's game interface (`angel-serve`)

A game implements `GameServer`, and `Server::start(game, ServeConfig)` runs
it.

- **Types.**
  - `Cmd: Command`: one seat's command. `decode_cmd(bytes)` runs on each
    connection's reader thread, so the sim never parses bytes.
  - `Rep: Replicate`: `EntityRep` (entity mode), `DocRep<V>` (document
    mode, where `V: DocView` filters the tree per seat), or the game's own.
- **The sim thread**, each tick, in order:
  - `join`, `leave` and `action` calls, as they arrived;
  - `presence(tick, seats)`: who is here (see "Presence" below);
  - `step(tick, cmds, events)`, where `cmds[seat]` is one command per
    seated seat (taken from that seat's `SeatInputs`);
  - `publish(tick)`, once. The tick is kept behind an `Arc`, and every
    writer reads the same one.
  - Before the first step, the game as it stands is published as tick 1,
    so the first clients see it at once. (Tick 0 is no tick: a client that
    has applied nothing says 0.)
- **Each writer** calls `Replicate::snapshot(tick, seat, viewer, out)`: its
  own client's body, against its own `Viewer`.
  - A new client is sent the tick newest at its welcome, whole, at once,
    rather than nothing until the next tick: a client never samples a
    command with no world to play in.
- **Events** are `Events::all`, `seat(n)` and `near(x, y)`. `near` reaches
  the clients whose interest covers the point. They go out as
  `ToClient::Events` in tick order, before that tick's snapshot.
- **The host's handle**: `links()` (one per door), `link::qr(link)`,
  `key`, `new_key`, `lock`, `kick`, `players`, `notice`, `stop`,
  `connect_local()` (see below), and `totals()`: two readings give the
  `Stats` between them (tick, step and publish times, writer encode time,
  bytes a second per client, joins, rejects, violations).
  `ServeConfig::log_every` prints a line of them.
- **Stepped by hand.** `Server::start_manual(game, config)` runs the same
  server with no sim thread: the caller steps the sim on its own thread
  with `Manual::run_to(now)`, on its own clock. That is for tests that
  step a game exactly (the cockpit's own, as its Delve tests always did).
  - The beat is the sim thread's: it begins at the first call, its first
    tick a step later, and one more than five ticks behind starts over.
  - Before each tick it waits (a second at most) for the readers of the
    in-process connections to act on everything their clients sent. So a
    command sent before `run_to` is in the tick after it, as it is on the
    thread, which sleeps a whole step between ticks.

### Presence: who is here

Before each step the sim tells the game what it knows of every seat's
player (`GameServer::presence`, a `Presence` per seat):

- `seated`: a player holds the seat, connected or held for a resume;
- `connected`: a connection plays it now;
- `playing`: that connection's client shows the game and plays it. It has
  applied the tick it was welcomed at (each `Cmds` says the newest tick its
  client applied), and its newest command is at most `AWAY_AFTER_TICKS`
  (30, a second at 30 Hz) old. The host's own in-process seats are playing
  while connected: they load nothing.
- `applied`: the newest tick its client applied.

A client whose game is off screen sends no commands: the page in a hidden
tab calls `angel_idle` (pings only) instead of drawing, and a friend's
angelX off the game's screen calls `NativeLink::idle`. So a page still
loading, a hidden tab, a link gone quiet, a seat held for a resume and an
empty seat are all not here, within a second.

**The Delve's use of it** (`rules::presence`, `DelveServer`):

- A knight whose player is not here is away: stone, which no foe sees and
  nothing hurts, and which holds up nothing the party does. It is itself
  again the moment its player plays, warded a second. Clients draw "AWAY"
  over it.
- A room the party has just come into holds (`Hold::Arriving`): its foes
  and their shots wait and no knight is hurt, until every player here has
  applied the tick of the arrival, or three seconds (`ARRIVAL_CAP`), so a
  page that never shows the room stalls nobody longer. A knight here may
  walk meanwhile. Clients draw "THE PARTY GATHERS".
- With no living knight here (every player away, or the host's own knight
  stone while friends are gone), the room holds the same way (`Hold::Empty`)
  rather than grinding idle knights down.
- A newcomer's knight is away until its page has the game, so it never
  arrives into a fight it cannot see.
- `/dungeon_host --watch` (or `ANGEL_DELVE_HOST_KNIGHT=0`) hosts without a
  knight of the host's own, for a dedicated machine. Knight 1 stays in the
  run as the host's place (saves and the rules keep a knight 1), but
  absent (`Hero::absent`): stone, counted for nothing, and left out of
  the tree every client reads (`wire::publish`), so no screen draws it.
  Friends keep knights 2 to 4 (the wire's seat `n` is knight `n + 1`
  either way). `bin/delve-local` hosts so.
- The party is the knights whose players are here (`Run::party_len`).
  Monsters' health, what they hit, extra foes, fury and the laurels count
  only those: two people play a party of two whatever statues stand by.
  Who is here can change any tick, so the run rescales at the start of
  each step (`rescale_party`, keeping each foe's share of its hurt), and
  writes `scaled_for` only while someone is away.
- A seat given up for good (a goodbye, a resume window run out, a kick)
  takes its knight out of the party at once (`DelveServer::leave` →
  `Run::leave`), told to everyone. A knight whose seat nobody holds at all
  (a restarted host's restored run, whose players' seats went with the
  old server) leaves after ten seconds (`UNSEATED_LEAVE`).
- Once the run has fallen or is won, any player here can take the party
  back to the kingdom (`rules::again`): Enter on the verdict's board asks
  ("ENTER: BACK TO THE KINGDOM"). The first ask starts a five-second wait
  every board counts down (`AGAIN_WAIT`); the party goes when every player
  here has asked or the wait runs out, so nobody is yanked from the
  verdict the moment it lands and nobody away holds the party there. The
  ask is a flag in the seat's command (`Input::again`); the host's server
  tells the cockpit once (`News::Again`), which does what the host's own R
  does: the run settled into the realm, a new one in the kingdom, the
  seats and their knights kept, the same link.
- After a hit a knight can't be hurt again for `HIT_WARD` ticks (0.8 s),
  blinking meanwhile. The panel's `hit_ward_s` moves it, 0 to 1 s.
- The host's composer pauses only a solo delve. With friends seated it
  plays on and the host's knight is stone, as before; now, with nobody
  here at all, the room holds.

## The host's own seat, and a friend's angelX (`angel-serve`)

### In-process connections

The host's terminal is a client exactly like a remote one. Only the
transport differs: a pipe in place of a socket.

- **`Server::connect_local() -> io::Result<LocalConn>`.** It goes through
  the same hello, admission, seat and token, resume, `SeatInputs`, writer
  (one snapshot a snapshot tick, skipping to the newest when behind),
  replication and events as a socket. Couch players take one each.
- **`LocalConn`** carries exactly the bytes a socket client sends and gets
  in one WebSocket message, with no frames around them:
  - `send(msg)`;
  - `try_recv()` and `recv_timeout(wait)`, each giving
    `Result<Option<Vec<u8>>, Closed>`. `Closed { code }` is the server's
    close code (`REPLACED` when another connection took the seat), given
    after everything sent before it;
  - `beat()`: when the server's writer last said it was alive (once a
    second, as a socket's writer pings);
  - `close()`, or dropping it: a socket closing. The seat is held for a
    resume;
  - `bye()`: a goodbye, then close. The seat is freed;
  - `redial()`: another connection to the same server, to resume on.

  Only `recv_timeout` blocks, so a terminal polls it once a frame.
- **What differs, and why.** Only what a pipe cannot do or need:
  - **The host is let in without the key, and while the session is
    locked.** The server made the pipe itself, so whoever holds it is the
    host. It is still not seated past capacity (it watches), and a kicked
    token stays kicked.
  - **It never counts among the 512 connections not yet in.** There is no
    door to flood.
  - **It has no message-rate limit and no silence close.** It cannot flood
    a network, and it cannot vanish without its pipe saying so.
  - **Its writer is never held back by the unapplied window.** A pipe has
    no link to queue on.
    - The pipe holds at most 64 KB the client has not taken
      (`LOCAL_UNREAD`, the window's ceiling). Only a terminal whose frame
      loop stalled reaches it.
    - Past that, the writer blocks and skips to the newest tick, as on a
      full socket.
    - After 10 s it is cut off, as a socket's write is. The seat is held,
      and the link dials again.
  - **The hello is due within 5 s, as a socket's is.** `NativeLink` sends
    it at once.
- **One code path.** `conn.rs` writes the WebSocket's life once, over a
  transport:
  - an `Inlet` (the reader's: the next message, waiting at most so long;
    after a close, linger for the client's);
  - an `Outlet` (the writer's: put a binary message, a ping, a pong or the
    close);
  - a `Cut` (the connection cut under both halves).

  A socket's inlet parses frames; a pipe's pops whole messages. `LOCAL` on
  the inlet is the only switch, and it switches exactly the list above.

### `wss://` for a friend's angelX

- **Feature `tls`** on angel-serve: `client::WsClient` connects to
  `wss://host[:port]/path`.
  - It uses rustls on ring, TLS 1.3 and 1.2. ring needs only a C compiler
    on Linux, macOS and Windows, and the cockpit carries it already.
  - SNI is sent. The certificate is verified, and its name checked against
    the link's host.
- **Trust: Mozilla's roots, compiled in** (`webpki-roots`), not the OS's
  store.
  - The doors are Funnel (`https://<name>.ts.net`, a Let's Encrypt
    certificate) and a relay on a public domain. Both chain to roots every
    browser trusts.
  - Compiled in, those roots are the same on all three OSes, with nothing
    to configure. The OS's store is not: Windows fills its root store
    lazily (a root arrives when the OS's own verifier first asks for it,
    which rustls never does), and a minimal Linux may have no CA bundle.
  - `Trust::Only(roots)` trusts given roots instead: a test's own
    certificate, or a private relay's. Verification stays on.
- **The two halves of one TLS session.** A link reads on one thread and
  writes on another.
  - A read blocks on the TCP socket with no lock held. It takes the
    session's lock only to decrypt what came.
  - A write takes the lock to encrypt and send.
  - So a blocked read never holds up a command.
- **`client::share_link(link) -> (ws url, join key)`** reads a link as the
  page does:
  - `https://castle.ts.net/#KEY` is `wss://castle.ts.net/ws` and `KEY`;
  - `http://192.168.1.5:8790/#KEY` is `ws://192.168.1.5:8790/ws`;
  - any path or query on the link is the page's, and is left out;
  - upper case (the QR code's) reads the same.

### `NativeLink`: the terminal's driver

`client::NativeLink<G: GameClient>` plays a game from a terminal. Remote
and local take one path.

- **Making one:**
  - `NativeLink::connect(link, name, LinkOptions)` joins from a share
    link;
  - `NativeLink::local(conn, name, LinkOptions)` plays on a `LocalConn`;
  - `LinkOptions` holds `resume` (a kept seat token), `caps` (`NATIVE` is
    always added) and `trust`.
- **Each display frame**, on the terminal's own thread:
  - `link.frame(now_ms) -> &Frame` takes what arrived, dials again if due,
    sends the commands due, and draws;
  - `link.hands()` is the sampler.
- **Or split in two**, for a terminal whose frame costs more than its
  thread should spend (the Delve's: a few milliseconds):
  - `link.tick(now_ms)` takes what arrived, dials again if due and sends
    the commands due, with nothing drawn: on the terminal's own thread,
    every frame, whether the game is on screen or not;
  - `link.draw(now_ms) -> &Frame` takes what arrived and draws, with no
    command sent: on a render worker, while the game is on screen;
  - `link.poll(now_ms)` only takes what arrived (waiting for a welcome
    without sampling a command before it);
  - `Runtime::tick` and `Runtime::draw` are the same halves of
    `Runtime::frame`.
  - Besides: `release_all(now_ms)` (the terminal lost focus), `action`,
    `take_lines`, `status()` (`Connecting`, `Playing`, `Reconnecting`,
    `Rejected`, `Replaced`, `Closed`), `seat_token()`, `reconnects()`,
    `retry_at_ms()`, `bytes()`, and `on_message` (a probe's tap: every
    message and when it arrived).
  - `bye()` frees the seat; `close()`, or dropping the link, holds it.
- **Nothing blocks the thread that draws.**
  - A socket is dialed (name, TCP, TLS, upgrade; 10 s at most), then read,
    on a thread of its own, and written on another.
  - A `LocalConn` needs no threads: sending to it and taking from it never
    block.
- **The page's rules:**
  - a lost link is dialed again after 0.5, 1, 2 and 4 s, then every 5 s,
    resuming the seat with its welcome's token;
  - 3 s with nothing heard (a message, or a ping) is a dead link;
  - a `Reject` stops it;
  - a stale token turned away with `Reject::Key` is dropped, and the link
    joins fresh at once;
  - close code 4000 stops it.
- **Arrival times.** Each message reaches the runtime with the time it
  arrived, on the terminal's clock, not the time the frame looked. So the
  server clock and the round trip are not off by up to a frame.
- **The bots keep their own loop.** They time each snapshot on the thread
  that reads it, and the slow ones read 256 bytes every 250 ms to be a slow
  link. A link reads as fast as bytes come, and would add two threads to
  each bot. `NativeLink`'s own tests run on the muster
  (`crates/bots/tests/link.rs` and `tls.rs`).

## The Delve on the engine

How the Delve moves onto this, once it is a crate (`angel-delve`). The
same rules, the same pictures; a different way of getting them to people.

- **The command.** `DelveCmd { held: Input, pressed: Input }`, in
  angel-delve, with a compact byte encoding of its own (not serde trees).
  - `held` is the keys down at the sample, and `pressed` the edges since
    the last one.
  - The step sees `held` with `pressed` laid over it, so a tap shorter than
    a tick fires that tick.
  - `held_only` drops `pressed` and the one-shot fields: `vote`,
    `leave_activity`, `activity_menu`, `skip_plate`.
  - `absorb` keeps an older command's presses.
  - The rules' press latches (`ult_key`, `take_key`, ...) stay as they are.
  - This retires `Stamped`, the leases, `Held::fresh`, pending taps,
    `sub_tick` and the 80 ms renewals.
- **The server** (`DelveServer: GameServer`, in the cockpit, because it
  settles into the realm). Document mode at 30 Hz.
  - `step` runs `Run::step`, then empties the rules' outboxes:
    - sounds become events; the cues go to the host's chorus, which sends
      what it says as events;
    - `bank`, `orders`, `triumph`, `feats`, `marks` and the rest stay in the
      run for the cockpit, which settles them into the realm as it always
      has (`settle_realm`, `settle_home`), told at once by a flag.
  - The run is behind one lock, which `step` and `publish` take once a
    tick. The cockpit's edits (gear, the realm's wishes and home, the
    intro, a host's vote) take it between ticks (`Sim::as_mut`), as they
    did on the old sim thread: dozens of call sites read back what they did
    (a wish's name, a reforge's error), and a closure applied a tick later
    could not answer them. Nothing does IO holding the lock: the spell
    cards are read off it and applied under it, and the checkpoint and the
    feel ledger are written from a published snapshot.
  - Joins, leaves and seats' actions (a wish, a reforged card, pieces of a
    large one) go to the cockpit as news, in order.
  - `publish` copies the run once, for the cockpit's reads (the snapshot it
    latches each frame), and builds the value tree from that copy, outside
    the lock.
  - The per-seat filter (`Tree::for_seat`: another knight's heat, shots in
    rooms a seat is not in) is the `DocView`.
  - The private fields (`rng`, `next_id`) are never sent. The run's `seed`
    is: the renderer places the merchant's camp and each room's pots by it.
  - Fields the renderer reads but saves leave out (`marks`, `pot_shards`)
    ride in the tree beside the run's own fields, so the save format does
    not change.
  - The pieces are pure functions in `angel_delve::wire`:
    - **Seats.** Engine seat `n` plays knight `n + 1` (`knight_of`,
      `seat_of`): the host's own seat 0 is knight 1, the one `Run::new`
      starts with. `SEATS` is 4.
    - **The tree.** `Publisher::publish(run)` builds it once a tick
      (private fields out, `marks` and `pot_shards` in); `read(tree)`
      gives the client its `Run`. A client checks `drawable(run)`, not
      `Run::valid_snapshot`, which needs the server's dice and ids.
      - What moves by a rule the client knows is sent as where it started
        (`reckon`): a shot flying straight as its place at the start of a
        256-tick span (to a 256th of a unit) and its life as the tick it
        ends; a monster's age as the tick it was born. `read` brings them
        to the tick.
      - Each shot carries an `id` that stays with it from tick to tick
        (the shots have none of their own: a shot is known again by where
        it would be, within 0.75 units), so the shots go as a keyed list
        and one landing does not move every shot after it.
      - Together these took a busy fight from 1,100 bytes a tick to about
        320 (the smoke's busy fight, below).
    - **Interest.** `seat_tree(tree, seat)` is the `DocView`: another
      knight's heat left out; a seat outside the party's room gets no
      shots and none of the games under way there; a watcher sees the
      party's room. It borrows the tree when there is nothing to hide.
    - **Events.** `Event` (`Sound`, `Say`, `Notice`), each with who hears
      it (`Hear::Party`: the party's room and the watchers; `Hear::All`;
      `Hear::Knight(k)`), in a few bytes of its own. `take_sounds(run)`
      empties the run's sounds as party events; the cues stay with the
      host's chorus, which sends what it says as `Say`. The client turns
      each into a shell line, or none when it is not its seat's to hear
      (a seat outside the party's room hears none of its sounds). Music is
      not an event: each client plays `Run::music` of its own view, so a
      reload or a resumed seat hears the right track at once.
    - **Params.** `Params { art_v2 }`, the welcome's params: a version,
      then flags.
  - The checkpoint is written from a published tick on a thread of its
    own (the cockpit's `delve-service`), never on the sim, every 3 s.
  - A solo delve's server listens on loopback with a key nobody is told;
    `/dungeon host` moves the same run onto a server on the host's door
    (the tailnet, then the LAN), and `/dungeon kick` back.
- **Every seat's own screen is made on its own client.** `view_for_seat`
  (the chariot's racer swap, the seat's own pastime heat, a seat outside
  the party's room) runs where the picture is drawn. The server makes no
  per-seat copies of the run at all.
- **The client** (`DelveClient: GameClient`, in angel-delve, built for wasm
  and native; the web crate is `crates/web/delve`).
  - The welcome's params are `wire::Params`: a version, then flags (bit
    0: the second art kit).
  - It holds the value tree, applies each patch, and reads the run back
    with its own `Deserialize`, keeping the last eight ticks. Poses two
    ticks apart still slide, so a host may send 15 snapshots a second.
  - Each display frame:
    - everyone else is drawn at `render_tick`, between the two runs around
      it (`Run::between` on their poses);
    - the own knight is the newest run plus the pending commands replayed
      through `Run::predict`, then walked `partial` of a tick along
      `View::hands`;
    - then `view_for_seat`, then `arena::frame_for_live` into the frame:
      a torch step's lit scenery (every sixth tick) is drawn beside the
      painter while the step before shows, natively on the baker thread,
      in a browser a stage a frame (`DungeonBake`).
  - Events become `sfx`, `say` and `notice` lines; music is the drawn
    view's own `Run::music`, said when it changes (`GameClient::lines`).
  - Its keys are the old browser seat's, in its controls JSON: WASD walk,
    the arrows aim, F fires, E meets the world and takes, Space (or Shift
    with a walk) dashes, Q swings, X bombs, Z drinks, 1–4 cast, G vigil,
    R the ultimate, M , . ; ' the skills, V the ballot (arrows, 1–5, 0, N,
    Esc in it), N held outside it the whole floor's map (the seat's own
    screen only: `DelveClient::map_open`, `arena::MapView`; the run and the
    wire never hear of it), Esc pauses a game in a room and Enter or
    Backspace leaves it; a press during a floor plate skips it. The pad's left trigger is
    a held layer (face buttons the skills, the d-pad the spells); the
    right stick aims and fires.
  - The generic float motion (`motion::Motion`) is not used: it costs a
    whole serialize and deserialize a frame. Knights, monsters, shots and
    cameras slide; what else a game moves (riders, chariots, crabs,
    pigeons, critters) steps at the tick rate. Bringing it back cheaply:
    each activity names its moving floats beside its fields
    (`activity::Activity`), and the client blends those typed fields
    between the two runs, as the pose does.
  - The kingdom's atlas is built once, on first entry.
- **The host's own screen is a client too.** It takes an in-process
  connection (`Server::connect_local`), through the same admission, seats
  and writers as a socket. The host terminal runs a
  `NativeLink<DelveClient>` on it and draws its frame as it draws pictures
  today (kitty PNG, or half blocks). Couch players are one in-process
  connection each. Host and friends take one path, so what the host sees
  is what a friend sees.
  - The terminal's thread samples each seat's commands every frame
    (`NativeLink::tick`); the frames are drawn on a render worker
    (`NativeLink::draw`): the viewer's for Kitty, the seat's own for half
    blocks. A frame costs the terminal's thread nothing.
  - The terminal's keys reach the seat's sampler as the held set of each
    seat (`set_held`), at once when a key comes, and each frame: where the
    terminal sends key releases they are exact; where it does not, its own
    hold (a lease on the key's repeats) lets go, and only there.
  - The menu, the ballot and a plate's skip are taps of the client's own
    keys, one a command, in the order they came.
- **A friend's angelX** joins over the same `/ws`, `wss` included, with
  `NativeLink::connect` and the same `DelveClient`.
  It works through Funnel and any relay.
- **The page** is the angel-web shell with the Delve's wasm. It is built by
  the cockpit's `build.rs` (a nested `cargo build --profile web` of the
  Delve's web crate) and embedded in the binary, so a page and its host
  always match.
- **What went:**
  - `together_guest.rs` (the picture stream, its painters, tile patches,
    the stamp, `FrameWindow`, `Adapt`, the leads, and every HTTP route of
    its own: the gear a friend sends is an action now, `together_gear`);
  - `guest.html`;
  - `together_net` (the trees per seat are `wire::seat_tree`, its transport
    angel-serve's);
  - `together_join`'s polling and legacy paths: a friend's angelX is a
    `NativeLink<DelveClient>`;
  - `mirror::Live` and `apply_live`;
  - `together_sim`'s own clock, leases, pending taps and `sub_tick`, the
    snapshots' per-seat views and blends, and the resident painters;
  - `/play`, `/net`, `/stream`, `/frames`, `/state` and `/shooter/input`;
  - `angel_delve::motion` (its whole-run round trip per frame: the client
    slides what the pose slides, and the rest steps at the tick rate);
  - the stamp's helpers (`arena::focus_stamp_box`, `focus_stamp_walk`) and
    the friend's copy of the host's phrasebook (`phrasebook::take`,
    `version`, `text`);
  - the main loop's shared-pane pacing for browser friends.

## Doors (how the link is reachable)

The server listens on the LAN. Above that, a session has one public door:

1. **LAN**: `http://<lan-ip>:<port>`. It works when everyone is on one
   network that allows peer traffic. Campus wifi often isolates clients, so
   this is not the default for classes.
2. **An https tunnel** in front of the host's port, named by
   `ANGEL_DUNGEON_PUBLIC_URL`. This works today.
3. **Relay** (to decide; see below). The host dials out to a relay over wss,
   and clients connect to the relay. One host connection carries every
   client. It works behind any NAT and needs nothing installed on the host.
   It is the robust answer for classes and for 200 players. The relay can
   also serve the static client from a CDN, so 200 phones downloading the
   game don't share a laptop's uplink.

**Open decision for the operator:** run a relay (a small VPS or a Cloudflare
Worker), and on which domain. The engine is built so a relay is a fourth
door with no change to clients.

## The browser runtime

`crates/web` (`angel-web`): the wasm ABI every game exports, and one shell
page that drives any game through it.

**Desktop first (operator, 2026-10-13).** The targets now are desktop
browsers on Mac and Windows: Chrome, Edge, Firefox and Safari. Phones come
later and are not built for yet: no touch layout, portrait layout,
safe-area work or phone probe profiles. What keeps them a later addition
with no ABI change:

- the game's controls JSON stays in the ABI (`angel_controls`);
- every input source goes through the sampler (`angel_set_held`,
  `angel_stick`), and touch will be one more polled source;
- the WebGL1 and canvas 2D fallbacks stay;
- nothing desktop-only is on the critical path. The wake lock and the
  full-screen button are optional extras.

### The wasm ABI

A game's client crate is a `cdylib` that calls
`angel_web::export_client!(MyGameClient)`. That wraps one
`angel_net::client::Runtime` and exports raw `extern "C"` functions:

- no bindgen and no generated JS;
- memory is shared: the shell writes bytes into buffers it got from
  `angel_alloc` and reads bytes, text and pixels at the pointers returned.

| export | what |
|---|---|
| `angel_alloc(len) -> ptr`, `angel_free(ptr, len)` | buffers the shell writes into |
| `angel_connected(join, join_len, resume, resume_len, name, name_len, caps)` | a socket opened: queue the hello |
| `angel_recv(ptr, len, now_ms) -> ok` | one server message (1 read, 0 malformed) |
| `angel_down(bits)`, `angel_up(bits)` | keys, as `angel_net::input::button` bits |
| `angel_set_held(bits)` | the polled sources (gamepads now, touch later), all at once |
| `angel_stick(on, x, y)` | an analog stick, -127..=127; `on` = 0 takes the stick from the direction bits |
| `angel_aim(on, x, y)` | an analog aim stick (a pad's right stick), -127..=127; `on` = 0 takes the aim from the `AIM_*` bits |
| `angel_release_all(now_ms)` | the tab was hidden or the window lost focus: let go of everything and send a command saying so at once (`Runtime::command_now`) |
| `angel_idle(now_ms)` | the page is hidden: pings only, no commands (the server counts the seat away) |
| `angel_frame(now_ms) -> ptr` | send the commands due, then draw; the frame is `w * h` words of `0xAARRGGBB` |
| `angel_frame_rgba(now_ms) -> ptr` | the same frame as RGBA bytes, for a canvas with no WebGL |
| `angel_frame_w()`, `angel_frame_h()` | the frame's size (0 before the welcome) |
| `angel_out() -> len`, `angel_out_ptr()` | messages to send, packed `[u32 len LE][bytes]` |
| `angel_lines() -> len` | the runtime's lines (`sfx <name>`, `music <name>`, `say <text>`, `notice <text>`), `\n`-separated |
| `angel_status()` | 0 joining, 1 playing, 2 rejected |
| `angel_reject()`, `angel_reject_text() -> len` | the `Reject` code, and the server's words |
| `angel_seat() -> i32` | the seat, or -1 while joining or watching |
| `angel_seat_token() -> len` | the token that takes the seat back |
| `angel_controls() -> len` | the game's controls JSON |
| `angel_rtt() -> f64` | the smoothed round trip in ms, or -1 |
| `angel_text_ptr()` | where every call above that returns a text length wrote |
| `angel_bye()`, `angel_action(kind, ptr, len)` | leaving on purpose; a one-off choice |
| `angel_knobs() -> len` | the knobs as JSON: `{"server": <the server's table or null>, "client": [<this page's>]}` |
| `angel_knobs_said() -> u32` | how many tables the server has said (the panel redraws when it moves) |
| `angel_set_knob(ptr, len, value) -> f64` | set this page's own knob by name: the value it took, or NaN |
| `angel_tune(ptr, len, value)` | ask the server to set its knob |
| `angel_watch_stats(on)`, `angel_stats() -> len` | the server's numbers for this seat once a second, and the game's counts, as JSON |

- **One import.** The module imports `angel.log(level, ptr, len)`. With
  `panic = "abort"`, a panic's message goes there before the trap, so the
  page can say what broke.
- **Held input.** Keys (`down`/`up`) and the polled bits (`set_held`) are
  held side by side, so a pad poll never lets go of a key. Every change
  goes in when it happens, not once a frame: the sampler keeps a tap that
  is shorter than a tick.
- **Own knight on the next frame.** `View` carries `hands`: the hands now,
  not yet sampled into a command. A game draws its own knight with:
  - the server's position;
  - the pending commands replayed on top;
  - `partial` of one tick walked along `hands`.

  So a key shows on the next display frame, not at the next command tick.
  At the muster's 20 Hz that is otherwise up to three frames later: the
  muster drew its knight between its last two predicted ticks and answered
  a key in 2 to 4 frames until it took this up (`MusterClient::own`). A
  press acts once, when its command is sampled, so the part-tick ahead
  walks the held hands only.
- **Twin-stick input.** `Hands` carries two sticks: the move stick
  (`x`, `y`) and the aim stick (`aim_x`, `aim_y`). Each is an analog
  stick where it leans, else made from its four buttons held at any moment
  since the last sample (the directions for the move stick, `AIM_UP`,
  `AIM_DOWN`, `AIM_LEFT` and `AIM_RIGHT` for the aim), so a tap shorter
  than a tick aims that tick as it walks it.
- **Buttons.** 32 bits in `angel_net::input::button`:
  - 0–15: the directions, `FIRE`, `USE`, `SLOT1`–`SLOT4`, `JUMP`, `MENU`,
    `ALT`, `SHIFT`, `MAP`, `OK`;
  - 16–19: the aim stick's directions;
  - 20–31: `X1`–`X12`, which a game names for itself (the Delve: bomb,
    drink, vigil, ultimate, its five skills, the ballot, rate 5, clear).
- **Controls JSON.** The format is
  `{"stick":"left","buttons":[{"id":"use","label":"E","bit":32}, ...]}`.
  `bit` is the button's value in `angel_net::input::button`: `USE` is
  `1 << 5`, so it is 32. The touch layout (later) is built from this
  list. A game with more keys than the default maps below gives its own,
  by button name (`button::NAMES`). Each one given replaces that default
  whole:
  - `"keys"`: `event.code` to a button,
    `{"KeyW": "UP", "ArrowUp": "AIM_UP", "KeyX": "X1", ...}`;
  - `"pad"`: a standard-mapping button index to a button. `"6+0"` is
    button 0 while button 6 is held: a held layer. A button that leads a
    chord does nothing alone, and a chorded button sends the chord's bit
    instead of its own (the Delve: the left trigger turns the face buttons
    into skills and the d-pad into spells);
  - `"sticks"`: `{"move": "left", "aim": "right", "aim_holds": "FIRE"}`:
    which pad stick moves, which aims, and a button held while the aim
    stick leans past half-tilt. The aim stick also sets the `AIM_*` bits
    past half-tilt, as the move stick sets the directions.

### The page (`crates/web/shell/`)

Plain JS in a handful of files, with no framework and no build step:

- `boot.js`: the name prompt, the module, the link, the frame loop, life and
  stats, and which way the frame reaches the screen;
- `present.js` and `stage.js`: putting the frame on screen with WebGL2 (a
  finished frame, or a stage frame drawn from its parts), WebGL1 or a 2D
  canvas;
- `gpu.js`: the same job through the renderer module (`crates/web/render`),
  when the bundle carries one the browser can run (below);
- `input.js`: keyboard and gamepad;
- `sound.js`: Web Audio;
- `loopback.js`: test only, for `?offline`.

The renderer module is `angel-gpu`, the core the native window draws with,
behind the canvas, in a wasm of its own beside the game's (the game's wasm and
its build are as they were). `crates/web/build.sh` makes it in two builds,
`render-webgpu.js` + `render-webgpu_bg.wasm` (WebGPU only) and
`render-webgl.js` + `render-webgl_bg.wasm` (adds wgpu's WebGL2 backend), when
the wasm-bindgen CLI at the version the crate pins is at hand (`$WASM_BINDGEN`,
or `wasm-bindgen` on PATH); without it the bundle has none and the page draws
as before, so no build needs the CLI. The cockpit's build (`cockpit/build.rs`)
makes the same two builds into the full game's page when the CLI is at hand, and
warns and ships `{}` when it is not. `boot.js` loads the WebGPU build where `navigator.gpu` is there and the
WebGL2 one where only WebGL2 is, and keeps `present.js` where neither is in the
bundle or either fails to start. A lost GPU device sends it back to `present.js`
for the rest of the session, said once in the console. `?present=` forces a way:
`gpu`, `webgl`, `legacy`, or (as before) `webgl1` and `2d`.

- **Boot.**
  - The page asks for a name, remembered in `localStorage`, while the wasm
    downloads and compiles (`WebAssembly.instantiateStreaming`, preloaded
    from the page head).
  - A page reloaded on a link it already holds a seat on skips the prompt
    and takes the seat back.
- **The link.**
  - The page connects to `wss://<host>/ws` (`ws://` on an http page), same
    origin.
  - The join key is the URL fragment.
  - The seat token from the welcome is kept in `localStorage` under the
    join key and sent as `resume`. If the server says a stale token is not
    this session's (`Reject::Key`), the page drops it and joins fresh.
- **Reconnect.**
  - The page retries a closed socket after 0.5, 1, 2 and 4 s, then every
    5 s, resuming the seat.
  - **3 s with nothing heard is a dead link.** Snapshots come many times a
    second and pongs every second, so silence means the link is gone even
    if the socket has not noticed (a network change, a laptop that slept).
  - The browser's `online` event retries at once.
  - A `Reject` stops retrying and shows why, in plain words.
- **The frame loop.** Each `requestAnimationFrame` runs, in order:
  1. poll the gamepads;
  2. `angel_frame(t)`;
  3. present;
  4. send what the module queued;
  5. act on its lines;
  6. watch the status.

  Typed-array views over wasm memory are made once and remade only when the
  memory grows. The loop allocates nothing per frame.
- **Present.**
  - WebGL2, else WebGL1, else a 2D canvas.
  - The frame is uploaded with `texSubImage2D` into one reused texture.
    WebGL2 uploads straight from an offset in wasm memory. WebGL1 keeps a
    view that starts at the frame.
  - It is drawn as one quad, scaled by the largest whole number that fits
    the canvas's device pixels, nearest-sampled and letterboxed in black.
    If that whole number would fill less than 80% of what fitting the
    frame would (the Delve's second art, 768 wide, is only 1× in a 1280
    window), or not even 1× fits, it is fitted, still nearest.
  - The bytes are B G R A, so the shader swizzles (`.bgra`). The 2D canvas
    has no shader and draws `angel_frame_rgba` instead, so no pixel is
    touched in JS.
  - The canvas's backing store follows its device-pixel size
    (`ResizeObserver`, `device-pixel-content-box` where the browser has
    it).
  - A lost WebGL context is restored.
  - `?present=webgl1` or `?present=2d` forces a fallback, for testing.
- **Keyboard**, by `event.code`, so WASD sits under the same fingers on any
  layout. The default map, for a game whose controls JSON gives no
  `"keys"`:

  | keys | bit |
  |---|---|
  | WASD and the arrows | the four directions |
  | E | USE |
  | F | FIRE |
  | 1–4 | SLOT1–4 |
  | Space | JUMP |
  | Esc | MENU |
  | Q | ALT |
  | Shift | SHIFT |
  | Tab | MAP |
  | Enter | OK |

  - Ctrl and Cmd combinations are left to the browser. Shift is a key
    like any other (`SHIFT` by default), so a game reads Shift+move itself.
  - Only mapped keys are taken from the browser (`preventDefault`).
  - Losing focus lets go of everything. So does letting go of Cmd, because
    macOS sends no keyup for keys released while Cmd is down.
  - A game's own map arrives with its controls JSON, after the welcome;
    whatever was held under the default is let go then.
- **Gamepad**, the standard mapping, polled only once a pad has connected.
  The default map, for a game whose controls JSON gives no `"pad"`:

  | pad | bit |
  |---|---|
  | A | USE |
  | B | JUMP |
  | X | SLOT1 |
  | Y | SLOT2 |
  | LB | ALT |
  | RB | SHIFT |
  | LT | SLOT3 |
  | RT | FIRE |
  | back | MAP |
  | start | MENU |
  | stick clicks | OK, SLOT4 |
  | d-pad | the directions |

  - The left stick is analog, with a 0.2 dead zone, and also sets the
    direction bits past half-tilt. With no `"sticks"`, it moves and no
    stick aims.
- **Life.**
  - **Sound.** Web Audio resumes on the first key or click. `sfx <name>`
    plays a sound from the bundle's manifest, fetched and decoded the first
    time it is asked for, with at most 6 voices. A sound more than 250 ms
    late is skipped. `music <name>` loops one track.
  - **A hidden tab** lets go of every key and sends that at once.
    Otherwise the server would walk on with the old keys for up to 15
    ticks. The tab then stops drawing and stops the commands: a 1 s timer
    keeps only the pings going (`angel_idle`), so the link and the seat
    hold while the server counts the seat away (a second without a
    command; see "Presence"). Coming back draws from the newest snapshot
    and sends commands afresh.
  - **The window losing focus** lets go of every key, also at once.
  - **The screen wake lock** is held while playing, where the browser has
    it.
  - **A full-screen button** shows where the Fullscreen API exists.
  - **The status line** shows connecting, reconnecting, or turned away and
    why.
  - **A panic in the game** stops the page and shows its message.
- **Stats.**
  - `?stats` shows fps, the module's frame ms (p50, p95), present ms, ping
    and KB/s.
  - `window.__angel` holds the same numbers for probes: status, seat,
    presenter, frames, dropped frames, fps, rtt, bytes in and out,
    reconnects, sounds, and the frame of the last key or button press.
  - `__angel.onframe` is a hook for probes, called after each frame.
- **Touch** comes with phones, later: a floating stick on the left half and
  the game's buttons on the right, laid out from its control map. The
  control map is in the ABI already, and touch is one more polled source
  (`angel_set_held`, `angel_stick`), so it needs no ABI change.
- **Fit for slow machines** (and phones later):
  - Each game renders at its own native size (the Delve 384×224, art v2
    768×448) and the GPU scales it.
  - Static layers (a room's ground) are drawn once and cached, not every
    frame.
  - The budget is in the numbers below, measured under a 4× CPU throttle
    as a stand-in for a weak laptop.

### The bundle

`crates/web/build.sh <game-crate-dir> [game]` builds the game's cdylib with
`--profile web --target wasm32-unknown-unknown` and writes
`crates/web/dist/<game>/`:

- `index.html`;
- the shell's `.js` and `.css` files;
- `<game>.wasm`;
- the game's sound files, from `<game-crate-dir>/sounds/` and from the
  folders `<game-crate-dir>/sounds.list` names (a name prefix, `-` for
  none, then a folder; the Delve's sfx, its knights' voices as
  `knights-…` and its music as `music-…`). The shell fetches each the
  first time it is asked for, so none is part of the first load.

It prints each file's size raw and gzipped.

- **Placeholders.** `index.html` names every file as `{{asset:NAME}}`. The
  server replaces each with `/a/<hash>.<ext>` and serves the files gzipped,
  `immutable`, and the wasm as `application/wasm`.
- **The sounds manifest** is inline in the page
  (`<script type="application/json" id="sounds">`), so the server's asset
  names reach it.
- **What the page needs from the server's CSP:**
  - `script-src 'self' 'wasm-unsafe-eval'`;
  - `style-src 'self'` (no inline styles anywhere);
  - `connect-src 'self'` (the socket, the wasm and the sounds are fetched).

  angel-serve's CSP gives all of this. It also names `ws:` and `wss:` in
  `connect-src`, since Safari before 15.4 does not count a socket to the
  same host as `'self'`.
- **A game's wasm crate** is one line,
  `angel_web::export_client!(TheClient)`, in a `cdylib`. The muster's is
  `crates/web/muster`, kept apart so the muster crate stays its own:

  ```
  crates/web/build.sh web/muster muster
  muster-host --bundle crates/web/dist/muster
  ```
- **Testing without a host.** `crates/web/probe/serve.mjs` serves a bundle
  the same way. `crates/web/testgame` is a test-only game with a loopback
  server in its wasm, played at `?offline` (`?offline=80` adds 80 ms each
  way).
- **A crowd to draw among.** `angel-bots --stand --delay 6` joins 200
  bots after the browser, standing where they spawned (drop `--stand` to
  have them wander).

### What was kept out for each browser

Only Chromium can be driven here, so these are avoided by design rather
than measured:

- **Safari.**
  - It has no `device-pixel-content-box`, so the backing store is the CSS
    size times `devicePixelRatio`, rounded. `image-rendering: pixelated`
    keeps any leftover resample square.
  - Its full screen is `webkit`-prefixed before 16.4, and both names are
    tried. There is no element full screen on iPhone, so the button hides.
  - Its audio needs a gesture; a keydown counts.
  - It needs Safari 15 or later for WebGL2 and for the wasm features rustc
    emits by default (bulk memory, reference types, sign-ext). Safari 15
    is the floor.
- **Firefox.**
  - It rounds `performance.now()` to 1 ms without cross-origin isolation,
    so its `?stats` timings are coarse. The game is unaffected.
- **Chrome, Edge and Firefox.**
  - They expose gamepads only to secure contexts. On a plain `http://` LAN
    link, pads do nothing; the https doors (Funnel, the relay) have them.
  - The wake lock is likewise https-only.

## The dev panel: knobs while it runs

A number in the rules is moved while the game runs, felt at once, and
then written back into the code as the new fixed number. That is the
whole workflow: tune live, then bake it in. (`angel_net::knob`;
`angel_serve::tune`; `crates/web/shell/panel.js`.)

- **A knob** is a name, a group, a label, its bounds and step, and the
  code's constant it stands for. Its default is that constant exactly, so
  nothing changes until a slider moves. At its defaults the Delve plays
  tick for tick as it did before knobs (`together_tune__tests`, against a
  fingerprint of the old code's runs).
- **Server knobs** change the game for everyone.
  - The game's (`GameServer::knobs`, `set_knob`) and the engine's own
    (`snapshot_every`, the ticks between snapshots).
  - A page asks with `ToServer::Action` of kind `knob::action::TUNE`
    (engine kinds start at `0xFFFF_0000`, and the game never sees them).
    The host's handle asks with `Server::tune(name, value)`.
  - The server takes it only from the host (an in-process connection, or
    its handle), unless the session runs in dev mode
    (`ServeConfig::dev`). Anyone else's ask is refused with a notice and
    the table sent again, so the slider goes back.
  - The sim fits the value to the knob's bounds and step and sets it
    between ticks. It then tells every client the new table
    (`ToClient::Knobs`, which also follows every welcome) and writes the
    knobs moved from their defaults to the tune file
    (`ServeConfig::tune_file`, `{"name": value}`) on a thread of its own.
  - At the start, the tune file's numbers are set before the first tick.
    A knob the game already moved keeps its number (a host moving its
    run onto a new door: the file may not have caught up).
- **A number prediction uses** reaches clients with the game's state, not
  only through the table. The Delve keeps its knobs in `Run::tune`, which
  is replicated with the rest of the run, so a client's `predict` walks at
  the server's pace. The run writes it only once a knob has moved, so
  saves and the wire carry nothing more until then.
- **Client knobs** change only this page.
  - The engine's: `interp_ms` (how far behind everyone else is drawn) and
    `correct_ms` (how long a correction of your own character eases; it
    reaches the game through `View::correct_ms`).
  - A game's (`GameClient::knobs`, `set_knob`): the Delve's art kit.
  - The page's own: `scale` (0 auto, 1 whole numbers, 2 fit) and
    `fps_cap`.
  - They are kept per game in `localStorage` and set again at the next
    seat.
- **The panel.**
  - It is offered only on a dev link (`…/#<key>&dev`) or in a dev session,
    and F2 opens and shuts it. A student's page never shows it.
  - Open, it takes a column at the right, and the game is fitted beside it
    rather than covered.
  - Each knob is a slider with its value. A knob moved from its default is
    marked, with a reset beside it. Server knobs a page may not set are
    dimmed, with a note saying how to allow it.
  - **Reset all**, and **copy as constants**: every knob as
    `CONSTANT = value`, to paste into the code (to the clipboard on a
    secure page, and into a box below either way).
  - A slider being dragged asks the server at most every 80 ms, and once
    more when let go. Letting go hands the keyboard back to the game. While
    the panel has focus the game hears no keys (and lets go of any held);
    a click on the game gives them back.
  - **The numbers strip**, once a second:
    - fps and dropped frames;
    - the module's frame time (p50 / p95) and present time;
    - round trip and bytes in and out;
    - the server's tick time (mean / most) and snapshot spacing;
    - this seat's command queue, ticks that repeated its last command,
      and commands skipped or folded (counted on the sim from the acks);
    - the page's own-knight corrections a second.
- **The terminal.** `/dungeon tune` lists the Delve's server knobs and
  where each stands; `/dungeon tune <name> <value>` sets one for everyone.
  `ANGEL_DELVE_DEV=1` puts the host's sessions in dev mode;
  `delve-dev-host --dev [--tune FILE]` and `muster-host --dev [--tune FILE]`
  do the same.
- **The Delve's server knobs** (`angel_delve::rules::tune`):

  | knob | default | what |
  |---|---|---|
  | `hero_speed` | 13 | a knight's walk, units a second (`HERO_SPEED`) |
  | `road_pace` | 2 | the kingdom's roads multiply it (`ROAD_PACE`) |
  | `roll_speed` | 27 | a roll out of the tuck (`ROLL_SPEED`) |
  | `fire_rate` | 100% | every weapon's cooldown, scaled where a knight sets it |
  | `shot_speed` | 1 | the knights' shots, scaled as they are fired |
  | `foe_pace` | 1 | each monster's step, scaled (a leap or a blink stays as it was) |
  | `foe_hit` | 100% | what monsters deal (`DEPTH_HIT`, `PARTY_HIT`) |
  | `foe_health` | 100% | monsters' health at their spawn; never below 100 |
  | `foe_count` | 100% | a room's roster, more of its own kinds; never fewer |
  | `camera_follow` | 0.22 | the kingdom camera's easing a tick, for the server's camera and a client's alike |

- **The input queue's seam.** The per-seat queue's numbers (`DEPTH`,
  `NEUTRAL_AFTER`, and whatever the jitter-tolerant queue adds) are not
  knobs yet. When they become fields a `SeatInputs` can be given, they
  join `angel_serve::tune::Engine` beside `snapshot_every`, and
  `Stepper::set_engine_knob` hands a new value to every seat's queue, as
  it hands `snapshot_every` to the sim and the writers.

## Numbers (fixed; changed only from measurement)

| what | number |
|---|---|
| Delve tick | 30 Hz |
| muster tick / snapshot | 20 Hz |
| interpolation delay | 100 ms |
| prediction correction | 100 ms |
| per-seat command cushion | 2 ticks |
| guesses before a seat counts as stalled | 4 ticks |
| a queue past its cushion drops a still command after | 30 ticks |
| per-seat command queue cap | 8 |
| neutral after silence | 15 ticks |
| a seat with no command for this long is away | 30 ticks (1 s) |
| a new room holds for the party at most | 90 ticks (3 s) |
| a knight back from away is warded | 30 ticks (1 s) |
| seat resume window | 120 s |
| entity snapshot budget | 1,200 bytes |
| a writer's unapplied window | starts at 1,200 bytes; late = applied 250 ms past the least delay of 5 s |
| connections before their hello | 512 (a kept-alive page idles 2 s) |
| an in-process client's untaken bytes before its writer blocks | 64 KB |
| a native link's dial (name, TCP, TLS, upgrade), at most | 10 s |
| capacity | 256 players, 32 spectators |
| **server**: 200 bots on the muster, the whole tick (step and every writer) | p99 ≤ 10 ms |
| **server**: per-client downstream, muster | ≤ 16 KB/s average |
| **client**: the module's frame on a low-end-laptop proxy (Chromium, 4× CPU throttle), Delve and muster | p95 ≤ 8 ms (60 fps) |
| **client**: first load, compressed | ≤ 3 MB |
| own action shown | next display frame |
| **client**: nothing heard for this long is a dead link (reconnect) | 3 s |

## Proof

Nothing counts as done without its measurement:

- `angel-bots` runs 200 clients against the muster over real sockets. It
  reports tick time p50/p99, per-client bytes per second, snapshot interval
  jitter and round trip.
- A headless Chromium probe (`crates/web/probe/probe.mjs`) runs the real
  page at desktop viewports (1× and 2× screens) and once with a 4× CPU
  throttle as a low-end laptop. It reports:
  - the module's frame ms;
  - dropped frames;
  - the display frames from a key to the own knight moving;
  - reconnect after the network is cut.

  Phone profiles join it when the touch layout does.
- The Delve's own tests stay green, and its look is checked by opening the
  renders, as always.

## Measured

### Server (angel-serve, the muster, angel-bots)

**Setup.** 2026-10-13, release build, on one host: an AMD Ryzen 9 9950X3D
(16 cores, 32 threads) running Linux. `muster-host` and 200 `angel-bots`
clients ran over localhost WebSockets.

- **Server numbers** come from `muster-host --measure`: the window while the
  whole crowd is in, starting once it has held steady for 3 s.
- **Client numbers** come from `angel-bots`: every bot runs the same
  `Runtime` a browser runs, measured from 5 s after the last bot joined.
- **The runs:**
  - "wandering": bots walk about within 600 px of the muster point,
    rest, wave and plant banners.
  - "+ 20 slow": 20 of them read only 256 bytes every 250 ms (1 KB/s).
  - "packed": all 200 walk within 100 px, so every client sees the whole
    crowd.
  - "drawing": every bot renders each 60 fps frame.

```text
cargo build --release -p angel-muster -p angel-bots
muster-host --port 8790 --bind 127.0.0.1 --key BENCHKEY --measure
angel-bots --url ws://127.0.0.1:8790/ws --join BENCHKEY --n 200 --secs 60 [--slow 20 | --stray 100 | --draw]
```

| | wandering | + 20 slow | packed | drawing |
|---|---|---|---|---|
| measured for | 60 s | 60 s | 60 s | 30 s |
| things in a client's view, avg / max | 39 / 94 | 40 / 91 | 256 / 334 | 42 / 95 |
| **tick, whole** (step, publish, waking 200 writers) p50 / p99 / max | 0.44 / **0.62** / 0.92 ms | 0.41 / **0.56** / 0.69 ms | 0.69 / **1.70** / 2.75 ms | 0.43 / **0.62** / 0.82 ms |
| step alone, p99 | 0.07 ms | 0.08 ms | 0.08 ms | 0.07 ms |
| writer encode (one client's snapshot), p50 / p99 | 0.006 / 0.027 ms | 0.005 / 0.019 ms | 0.028 / 0.078 ms | 0.006 / 0.022 ms |
| **downstream per client** (server), avg / p99 | **1.86** / 2.46 KB/s | **1.79** / 2.46 KB/s | **11.8** / 12.7 KB/s | **2.15** / 2.94 KB/s |
| snapshots a second per client | 20.00 | 20.00 (slow ones 12.9) | 20.00 | 20.00 |
| snapshot gap at the client, p99 / max | 50.1 / 50.5 ms | 50.1 / 50.6 ms | 50.6 / 52.3 ms | 50.1 / 50.6 ms |
| jitter, \|gap − 50 ms\| p99 | 0.2 ms | 0.1 ms | 0.7 ms | 0.1 ms |
| round trip (ping, localhost), p50 / p99 | 0.02 / 0.06 ms | 0.03 / 0.06 ms (slow ones 1.2 / 1.7 s) | 0.03 / 0.19 ms | 0.05 / 0.15 ms |
| disconnects / rejects | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |

- **Targets.** The whole tick's p99 is under 1.8 ms in every run, against
  10 ms. Downstream per client is under 2.2 KB/s for a wandering crowd,
  and 11.8 KB/s when all 200 are in every view, against 16 KB/s.
- **A slow client slows nobody.**
  - With 20 clients reading at 1 KB/s, the other 180 still got exactly
    20 snapshots a second (gap p99 50.1 ms), and the tick did not move.
  - The slow ones' writers held back 8,811 snapshot ticks and skipped to
    the newest each time.
  - Their round trip stayed under about 2 s: one snapshot in flight on a
    1 KB/s link, not a growing queue.
- **Client CPU, natively.** The muster's `Runtime::frame` (prediction,
  interpolation and a 320×180 software frame) took 0.028 ms at p50 and
  0.115 ms at p99, on a desktop core. The phone-class number (Chromium,
  4× CPU throttle) is the browser runtime's probe to measure.
- **Localhost round trips** show the server adds no delay of its own (a
  pong is written as soon as the reader hands it over). They say nothing
  about real networks.
- **Changed from measurement.**
  - The first build sent a changed thing's state whole. The packed crowd
    measured 19.3 KB/s a client, so updates now carry only the bytes that
    changed since what the client holds: 11.8 KB/s.
  - The first hold-back allowed a fixed count of unapplied snapshots. A
    1 KB/s client then queued up to 7.9 s of stale state, so it is now the
    delay-driven window described under Transport: under 2.1 s.
- **A shared host shows.** One run of "+ 20 slow" on this build shared the
  machine with another benchmark (four cores busy) and measured a tick p99
  of 1.64 ms, its step slower too. Repeated alone, it is the column above.

### The host's own seat among 200 (in process)

**Setup.** 2026-10-13, release build, the same host. `muster-host --local 1`
seats one in-process player, as the host's terminal: a
`NativeLink<MusterClient>` on a `LocalConn`, drawing every 60 fps frame on
a thread of its own and walking about as the bots do. 200 angel-bots join
over sockets as before.

- The machine was shared with other work (load average 5 to 11 of 32
  threads), so the run without a local player was made just before and
  just after, under the same load.
- "packed + local": the local player walks inside the packed crowd
  (`--stray 100` on both), so its own view holds all of it.

```text
muster-host --port 8811 --bind 127.0.0.1 --key BENCHKEY --measure [--local 1] [--stray 100]
angel-bots --url ws://127.0.0.1:8811/ws --join BENCHKEY --n 200 --secs 60 [--stray 100]
```

| server, 60 s | wandering, before | wandering + local | wandering, after | packed + local |
|---|---|---|---|---|
| players | 200 | 201 | 200 | 201 |
| **tick, whole** p50 / p99 / max | 0.56 / 0.92 / 5.77 ms | 0.52 / **0.79** / 1.11 ms | 0.56 / 1.84 / 7.86 ms | 0.82 / **1.64** / 2.03 ms |
| step alone, p99 | 0.10 ms | 0.09 ms | 0.13 ms | 0.10 ms |
| writer encode p50 / p99 | 0.009 / 0.024 ms | 0.008 / 0.020 ms | 0.009 / 0.024 ms | 0.053 / 0.131 ms |
| **downstream per client** avg / p99 | 1.89 / 2.55 KB/s | **1.82** / 2.49 KB/s | 1.89 / 2.51 KB/s | **11.9** / 12.8 KB/s |
| bots' snapshot gap p99 / max | 50.2 / 55.4 ms | 50.2 / 50.9 ms | 50.4 / 58.2 ms | 50.6 / 52.1 ms |
| disconnects / rejects | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |

| the in-process player | wandering | packed |
|---|---|---|
| snapshots a second | 20.00 | 20.00 |
| downstream | 2.1 KB/s | 11.4 KB/s |
| snapshot gap p99 / max | 50.1 / 50.2 ms | 50.7 / 50.8 ms |
| jitter, \|gap − 50 ms\| p99 | 0.1 ms | 0.7 ms |
| round trip p50 / p99 | 0.034 / 0.068 ms | 0.067 / 0.125 ms |
| `NativeLink::frame` p50 / p99 / max | 0.025 / 0.065 / 0.152 ms | 0.062 / 0.215 / 0.322 ms |
| reconnects | 0 | 0 |

- **Targets.** The whole tick's p99 stays under 1.7 ms against 10 ms, and
  downstream per client under 12 KB/s against 16 KB/s.
- **The host is one more writer.** The tick with it sat inside the spread
  of the runs without it, either side.
- **The host's own stream** is as steady as a socket's, or steadier: 20
  snapshots a second, gap p99 within 0.7 ms of 50.
- **Its round trip** (0.03 to 0.13 ms) is the server's own: the reader
  handing a ping over and the writer writing the pong.
- **Its frame** is everything the terminal's thread does for the game:
  taking what arrived, the commands, prediction, interpolation and the
  320×180 picture. p99 0.22 ms with the whole crowd in view.

### Browser runtime (angel-web)

Measured 2026-10-13 with `crates/web/probe/probe.mjs`, driving headless
Chromium 152 on the development machine (Ryzen, RTX 3090 through
ANGLE/OpenGL) against `muster-host` on localhost. The runs:

- **desktop**: 1280×800 at 1×;
- **retina**: 1440×900 at 2×;
- **laptop4x**: 1280×800 with the CPU throttled 4×, the low-end-laptop
  proxy.

The columns:

- "wasm" is the module's whole `angel_frame`: commands, prediction,
  interpolation and drawing.
- "present" is the JS side of the upload and draw.
- "key to moved" counts display frames from a key down to the own knight
  drawn somewhere else. 1 is the next display frame; 12 trials a run, each
  landing at a different point of the command tick.
- Chrome rounds `performance.now()` to 0.1 ms, so 0.0 means under 0.1.

**The muster among 201 knights.** The browser holds seat 0, in the middle
of the spawn spiral. 200 angel-bots join around it and stand, so about 45
knights and 24 name tags are in its 320×180 view: the densest view the
muster has.

| run | fps | dropped | wasm p50 / p95 ms | present p50 / p95 ms | key to moved |
|---|---|---|---|---|---|
| desktop | 60 | 0 | 0.0 / 0.1 | 0.0 / 0.1 | 1 frame, 12 of 12 |
| retina | 60 | 0 | 0.0 / 0.1 | 0.0 / 0.1 | 1 frame, 12 of 12 |
| laptop4x | 60 | 0 | 0.1 / 0.7 | 0.1 / 0.8 | 1 frame, 12 of 12 |
| laptop4x, the crowd wandering | 60 | 0 | 0.1 / 0.6 | 0.0 / 0.5 | 1 frame, 12 of 12 |
| laptop4x, SwiftShader (no GPU) | 60 | 0 | 0.1 / 0.7 | 0.1 / 0.7 | 1 frame, 12 of 12 |
| laptop4x, canvas 2D | 60 | 0 | 0.1 / 0.7 | 0.0 / 0.5 | 1 frame, 12 of 12 |

- **Target: wasm frame p95 ≤ 8 ms at 4×.** Measured 0.7 ms.
- **Target: own knight on the next display frame.** 12 of 12 in every run.
  The muster as first merged, which drew its knight between its last two
  predicted ticks, took 2 to 4 frames, median 3, and 0 of 12.
- **Down.** 0.9 KB/s with the crowd standing, 6.8 KB/s with it wandering.
  Up is 0.2 KB/s.
- **Target: reconnect resumes the same seat.** Every run cut the network
  (`Network.emulateNetworkConditions` offline) twice:
  - **3 s:** Chromium stalls an open socket rather than closing it, and 3 s
    is not past the page's silence rule. The socket carried on with no
    reconnect, the same seat, and play flowing again within 4 ms of the
    network coming back.
  - **8 s:** the page dropped the socket after 3 s of silence and retried
    on its backoff. It took **the same seat** (0) back, playing 51–55 ms
    after the network came back.
- **Hidden tab** (3 s, with a key held):
  - 0 or 1 frames were drawn while hidden;
  - the seat was kept, with no reconnect;
  - the own knight moved 0–2 px across the hide, because the letting go
    is sent at once.
- **Target: first load ≤ 3 MB compressed.** The muster's bundle is 59 KB
  gzip (168 KB raw); its wasm is 44 KB gzip (130 KB raw). The module was
  ready 420–530 ms after navigation, throttled or not, on localhost.
- **Memory.** The module's memory stays at 1,408 KB.

**Test game** (`crates/web/testgame`, 320×180, 25 dots, loopback 40 ms each
way):

- 60 fps, 0 dropped, wasm p95 ≤ 0.4 ms and 1 frame key to moved, in every
  run and on WebGL2, WebGL1 and canvas 2D;
- 60 s at 4× on SwiftShader: 3,636 frames, 0 dropped, memory flat;
- bundle 41 KB gzip.

**Not measured here:**

- Firefox, Safari and Edge (only Chromium can be driven on this machine);
- a real GPU other than this one;
- a real network (everything was on localhost);
- gamepads (the mapping was checked in node against a fake pad).

### The Delve on the engine (live)

Measured 2026-10-13 on the development machine, with other builds running
beside it. The host was started as a user starts it (`bin/angelX`, release
build, in a 200×60 tmux pane with half blocks), hosting on loopback with
`/dungeon_host --2`. A friend's angelX joined the link in a second pane, and
the probe (`crates/web/probe/probe.mjs`) opened it in headless Chromium.
All three stood in the kingdom and the Delve's gate room.

| seat | fps | frame work | key to own knight moved | down / up |
|---|---|---|---|---|
| browser, desktop 1280×800 | 60, 0 dropped | wasm p50 0.3 / p95 0.4 ms | 1 frame, 11 of 12 | 2.5–2.8 / 0.3 KB/s |
| browser, laptop4x | 59.1, 8 dropped | wasm p50 1.4 / p95 20 ms | 1 frame, 12 of 12 | 2.1 / 0.3 KB/s |
| host's terminal | 60 | game p50 0.69 / p95 1.07 ms, draw p50 0.51 / p95 0.72 ms | | in process |
| friend's angelX | 60 | game p50 0.41 / p95 0.94 ms, draw p50 0.63 / p95 4.6 ms | | 1.5–2 KB/s (the server's per-seat average) |

- **The server.** A step is 0.08–0.14 ms. The publish (the run copied
  for the cockpit, and its tree built) is 1.2 ms with two seats and 2.5 ms
  with three (p99 5.5 ms), each seat's encode about 2.4 ms: a tick under
  7 ms against 33. About 1.5–2 KB/s a seat while the party stands about;
  a whole snapshot, at a join or a resume, is about 210 KB.
- **Reconnect.** A 3 s cut left the socket standing and play flowing again
  18–78 ms after the network came back; an 8 s cut dropped it, and the page
  took the same seat (knight 3) back, playing 52–57 ms after.
- **The browser at 4×.** Its p95 of 20 ms is over the 8 ms target (p50
  1.4 ms, 8 frames dropped in 8 s): one frame in twenty is slow there, not
  yet looked into. The desktop run's p95 is 0.4 ms.
- **Not measured here:** a friend's angelX key to its knight moved (its
  client is the same `DelveClient` the probe measured), Funnel's `https`
  (the engine's own TLS tests cover `wss`; opening Funnel exposes the
  machine), and a real network.

### The command queue and a rolling knight (live)

Measured 2026-10-14 from the development machine to a rented host
(Maryland, about 55 ms away, reached over an ssh tunnel), with
`delve-dev-host` before and after the cushioned queue and the shared
prediction, side by side.

- **The link.** A 30 Hz stream of 9-byte frames, timed where it arrived
  against the best 30 Hz grid: over a tunnel with Nagle on its ssh link,
  p50 19 ms, p90 174 ms late; over one without (`ssh -tt`), p50 2 ms, p90
  11 ms; both with a stop near a second now and then. Later runs over the
  first kind were as clean as the second, so the bunching comes and goes.
  The cushion (2 ticks) covers the clean link with room for a Wi-Fi hop.
- **Bunched sending.** A client sending in bunches 166 ms apart (a busy
  machine, or Nagle) for 30 s, walking and stopping every 6 commands, the
  two hosts side by side:

  | queue | ticks played as no command of the seat's | wrong by a tick |
  |---|---|---|
  | before (FIFO of 3) | 346 repeats and 348 folds in 869 ticks | every one |
  | after | 367 guesses, 2 folds | 89 (keys changed on a guessed tick) |

  Bunches 100 ms apart, or a steady stream: nothing guessed, nothing
  folded, before or after.
- **A roll in the browser** (the probe's `--roll`: walk 0.3 s, Space,
  walk on; then back the other way; the own knight's drawn place on every
  frame, with a key held, in frame pixels, three runs each):

  | | frames | stood still | stepped back | step: usual / biggest |
  |---|---|---|---|---|
  | before | 81–82 | 3–5 | 5–6 | 2 / 24–42 |
  | after | 82 | 0 | 0 | 2 / 5 |

  Before, the knight stopped for the round trip at the Space press, then
  jumped in whole ticks and snapped back as the server's roll came in.

## Build order

1. **Contract.** `angel-net`'s protocol, codec and command types, with
   tests. Written first, because everything else talks through it.
2. In parallel:
   - **Server.** `angel-serve` and `angel-bots` and the muster's server side,
     proven at 200.
   - **The Delve as a crate.** `angel-delve` builds for native and wasm; the
     cockpit uses it, and every test passes.
   - **Browser runtime.** `angel-web` and the muster's client, proven with
     the Chromium probe.
3. **The Delve on the engine.**
   - The cockpit hosts it through `angel-serve`.
   - Browser seats run the wasm Delve.
   - The native join speaks wss.
   - The input model switches to commands.
4. **Delete what this replaces:** the picture stream, `guest.html`'s slide,
   the leases, `sub_tick`, `/play`, `/net`, `/stream`, `/frames` and the
   `Live` mirror.
5. **Doors.** The relay, once the operator decides.
6. **The host app.** A double-click app for Mac and Windows for whoever runs
   the session (a professor, not the students). It is the angelX binary with
   a window: start a session, show the link and its QR code, the roster,
   lock, kick and a new key. Students never install anything; they open the
   link.

Decided against (2026-10-13): an Electron app. It ships its own Chrome, so
the game runs exactly as it does in the browser, at 150 to 250 MB plus
signing and updates. If a game ever needs more than a browser gives, the
answer is a small native window client built from the same `GameClient`
code, offered beside the link and never instead of it.
