# angelY

angelY is angelX in Yukon competition posture. It lives on the `comp/angelY` branch only; dev stays free of Yukon mode.

- `scripts/angely/pin.sh` builds a source-bound release binary from this clean checkout into `.angelY/pin/` (gitignored; `ANGELY_HOME` moves it). Seats never run the live cargo target, so a rebuild cannot swap a binary under a running agent.
- `bin/angely [workspace] [driver]` launches one seat on that pin: `ANGEL_COMP_PACKAGE=yukon`, competition mode, lean frames, YOLO, in a memory-capped systemd scope, with wire logs and trajectories under `.angelY/logs/<seat>/`. Drivers: `glm` (GLM 5.3 high), `sol` (gpt-6.1-sol xhigh), `sol-low`, `kimi`, `deepseek`, `local`.
- `scripts/angely/seats.sh [--no-loop] [seat…]` opens the seats in `scripts/angely/seats.conf` as tabs of one Kitty window and arms each seat's `/loop` on `.angelY/ORDERS.md` in its workspace: `scripts/angely/ORDERS.md` plus `scripts/angely/boards/<seat>.md`.
- `scripts/angely/screen.sh <seat>` prints a seat's screen as text.
- Yukon submission and fleet status changes show in the overworld garden; a score without a verified baseline and metric direction is shown, not plotted.
