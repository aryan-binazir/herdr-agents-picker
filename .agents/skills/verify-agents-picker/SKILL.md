---
name: verify-agents-picker
description: Verify picker rendering, search, numeric focus, refresh, and launch through the real release TUI on private tmux. Use after picker input or layout changes.
---

# Verify agents picker

Run from the repository root. Read the [feature map](features/README.md), then run the recipes covering the changed behavior. Each recipe sends real keys to the release executable. `HERDR_BIN_PATH` points to a strict fixture CLI; this proves requests at the executable boundary, not real Herdr focus.

```sh
cargo build --release --locked
python3 .agents/skills/verify-agents-picker/scripts/verify.py doctor
python3 .agents/skills/verify-agents-picker/scripts/verify.py run numbers --evidence /tmp/picker-numbers-001
```

The public recipes are `search`, `navigation`, `numbers`, `refresh`, and `launch`. Give each a new evidence directory. Use `--binary /absolute/path/agents-picker` to pin another build. `doctor` reports its path, SHA-256, help, and tmux version. Python 3 and tmux are required; no Python packages are needed.

The picker is one full-width list with number, status, thread, workspace, and agent columns. Thread gets most width. Herdr supplies the outer `Agents` title and 95% by 60% popup. The picker draws no interior border or preview. Explicit names take precedence over normalized terminal titles; terminal reads and observe requests fail verification.

Each case owns a fresh tmux socket, configuration, fixture, and plugin state directory. It never connects to user Herdr or tmux. Socket denial requires rerunning the same isolated command with suitable permissions, never substituting a live session. `fixture` and `child` are internal commands.

Evidence includes `doctor.json`, timestamped `actions.jsonl` with tmux commands and keys, paired `.txt`/`.ansi` screens, `calls.jsonl`, fixture/configuration, exit status, and `result.json` on success or `failure.json` on failure. Numbers has one subdirectory per case. Inspect focus requests for the exact workspace, tab, agent sequence. Missing coverage remains unverified.

Success and failure both stop only the owned child and tmux server, retain evidence, remove scratch, and record socket/server/scratch cleanup proof. Never kill by process name. Real-Herdr verification must likewise use a private HOME, XDG paths, and socket with synthetic agents; user sessions and configuration are outside this skill's scope.
