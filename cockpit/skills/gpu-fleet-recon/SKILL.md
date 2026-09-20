---
name: gpu-fleet-recon
description: Triage GPUs and fleet boxes read-only — utilization, VRAM squatters, offline rigs, idle paid pods — and report; never kill, restart, or destroy.
---

# GPU / fleet recon (read-only)

Recon **reports**; the operator **acts**. Nothing in this playbook kills a
process, reboots a rig, or destroys an instance — if the fix requires any of
those, present the finding and the exact command, and stop.

1. **Local box first: `gpu_stat`.** Read utilization, VRAM used/total, temp,
   power, and the compute-process list. Interpretation:
   - High VRAM + ~0% util → a loaded-but-idle server squatting memory. Find
     which (`proc_status` if we launched it; otherwise match the pid).
   - Util pinned at 100% with low power draw → likely stuck, not working.
   - Thermals near limit → note it; sustained throttling ruins benchmarks.
2. **Fleet: `fleet_status`.** Which rigs are online and their tailnet IPs.
   A box that should be up and isn't is a finding, not a thing to fix from
   here. Probe a rig's inference endpoint with `llm_probe http://<ip>:<port>`.
3. **Rented pods: `vast_instances`.** These burn money while running. An
   instance that's `running` with nothing on its GPU is the most expensive kind
   of idle — flag it with its $/hr, but destroying it is the operator's call,
   always.
4. **Deep-dive read-only extras** (via `shell`, all non-mutating):
   `nvidia-smi topo -m` (NVLink/PCIe topology), `dmesg | tail` for Xid errors
   after a crash, `ss -ltnp` to see what's listening where.
5. **Report shape:** one line per box/GPU — state, headroom, anomaly — then a
   short "actions for you" list of the commands you did *not* run.
