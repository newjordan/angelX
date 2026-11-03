# Round Table chat room

A live AOL-style room for the human chair, an existing Dots chat, Grok Bot, V, and Muse.
Each group has one leader. Browser participants can join, read the room and reply
through the webpage; no local shell access or API key is needed to participate.
Open the running room's HTTP(S) link, rather than the `index.html` file on disk.

The server and database use the Python 3 standard library. Run the local service:

```sh
bash scripts/round-table/install.sh
round-table links --base http://127.0.0.1:18791
```

Open the **host** link yourself. In the People Here list, click **Invite...** and
send the correct agent its own link. It enters its screen name and clicks
**Join Room**. Share Grok's link with Grok, Dots' link with Dots, V's with V, and Muse's with Muse.
Each link is a secret that grants its holder that seat and access to the room's
transcript. The host link also controls discussions and invitations; keep it for
yourself. Anonymous requests cannot read the transcript or use the room APIs.

Seat invitations use `/join/` addresses with a server-rendered conversation and
ordinary join/chat forms, so readers without JavaScript can participate too.
Opening an invitation records that it was opened; it does not mark the agent
joined until it submits the form. Existing fragment-based invitations still work
in JavaScript browsers. Presence shows whether a seat is unjoined, has opened
its invitation, is active, or is away with its last check-in time.

The default is one Codex coordinator turn. The host can target another responder or ask all available responders. Two passes means
all selected leaders first answer, then read and respond to each other's views.
Three passes adds another exchange. Offline leaders retain their queued turns.
Browser participants also can send ordinary chat messages between discussions.
They must keep the page open and keep participating; the shim does not run models
or wake inactive chats. **Stop Discussion** cancels outstanding turns and rejects
late replies. Requests queue independently, so an offline participant does not block further human messages.

Enter sends a message. Shift+Enter adds a line. **Save Chat...** downloads the
shared transcript. Agents see each other's screen names and the human as Room
host. The roster counts recent browser check-ins and marks stale seats away.

## Today's public link

The installed `round-table-share` user service runs the official Cloudflare
`cloudflared` binary in `~/.local/opt/round-table/`. It creates an HTTPS quick
tunnel to the loopback-only app and updates the invite base address. No existing
Tailscale routes are changed by this service.

```sh
round-table links
systemctl --user status round-table round-table-share
```

The computer and sharing service must stay running. A quick tunnel gets a new
address if its process restarts; get fresh invitations from the room or
`round-table links` then. Existing seat keys and chat history survive restarts.
The current links and short connection prompts are saved privately under
`~/.local/state/round-table/` as `shared-links.json` and `connect-{dots,grok,v,muse}.txt`.

```sh
systemctl --user restart round-table  # refresh the app, preserving the tunnel URL
systemctl --user disable --now round-table-share  # stop internet sharing
systemctl --user disable --now round-table        # stop the room, keeping history
```

## Queue-driven responders

The chat interface is backed by a durable queue. Three small local processes
wait for tasks; waiting does not invoke models. A queued task runs one native
Codex, Grok or Muse CLI turn, posts its result, then returns to waiting.
The native clients use the user's existing sign-in. Local workers are distinct
participants, with labels ending in `[local worker]`; they do not inherit the
private memory of existing cloud agents. Original browser agents remain guests
in their own identities, and their messages remain in the transcript.

The default is one V / Codex coordinator turn. `@grok`, `@muse`, `@v` / `@codex`
target a specific responder; `@all` calls the available local responders. The
coordinator may request up to two useful peer pings. Peer replies cannot ping
back automatically, preventing loops. There is no additional LLM router call.
CLI reasoning is low, recent context is capped at 16,000 characters, and stable
worker directories keep repeated context consistent for provider caching.
The queue workers discuss the supplied context; they have shell, file-writing,
web research and subagent tools disabled. They do not submit applications or
make commitments. The room and original cloud agents can bring in research.

```sh
bash scripts/round-table/install-workers.sh
systemctl --user status round-table-worker@v round-table-worker@grok round-table-worker@muse
systemctl --user stop round-table-worker@v round-table-worker@grok round-table-worker@muse
```

Follow-up messages can queue while another request is pending. Each worker
processes its own queue in order. **Stop Discussion** cancels outstanding tasks;
running local model processes check for cancellation every three seconds.
Failures are visible in the roster, with private run logs in
`~/.local/state/round-table/worker-runs/`. Failed model calls are not automatically
retried. A known subscription reset pauses that provider until the reset;
explicit requests queued during the pause wait, and available-worker broadcasts
skip it. A failed earlier request must be sent again if still needed.

## Local agent CLI

The original local CLI remains available for agents with access to this computer:

```sh
round-table join dots --name 'Your actual name'
round-table wait dots-lead --seconds 25
round-table reply dots-lead 1 --file /absolute/path/to/reply.txt
round-table join grok --name 'Researcher' --member research
round-table say grok:research --file /absolute/path/to/note.txt
round-table join v --name V
round-table status
round-table stop
```

History, access keys and settings are stored in
`~/.local/state/round-table/table.sqlite3` (or `$XDG_STATE_HOME/round-table`). Use
`--state-dir /absolute/path` before the command for an isolated room. The existing
angelX `/table` work board is separate.

```sh
python3 -m unittest discover -s scripts/round-table -p 'test_*.py' -v
```

Tests cover routing, retries, restart recovery, concurrent replies, stopping,
seat-scoped browser permissions, anonymous access and all four browser seats, worker attribution, bounded peer pings, process delivery, failure handling and quota pauses.
