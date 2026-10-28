# Launch and close

Run `python3 .agents/skills/verify-agents-picker/scripts/verify.py run launch --evidence /tmp/picker-launch-001`.

q exits with code zero and no focus requests. Invoke the same exact executable with `--open`; it must request `plugin pane open --plugin yxhta.agents-picker --entrypoint picker --focus` successfully. The fixture does not render a Herdr popup.

Ctrl+C and navigation-mode Escape are additional close paths. Use the manual check below for popup geometry and real focus. Never link into a live user configuration for verification.

## Real Herdr check

Use a throwaway harness or run these steps manually. Herdr and tmux are required.

1. Create an owned temporary directory. In a subshell, unset inherited `HERDR_*`, `TMUX*`, `BASH_ENV` and `ENV` variables. Set `HOME`, all `XDG_*` directories, and `HERDR_SOCKET_PATH` to paths inside that directory. Create those directories; give `XDG_RUNTIME_DIR` mode 700. Set `HERDR_BIN_PATH` to the installed Herdr executable. Record these paths before starting anything.
2. Copy the reviewed plugin manifest into an owned plugin directory. Symlink its `target/release/agents-picker` to the exact release binary under test. Disable update checks and sounds in the private Herdr config. Start Herdr inside `tmux -f /dev/null -S <owned-socket> new-session -d -s verify -x 160 -y 40 'exec env -u TMUX -u TMUX_PANE herdr'`. Every subsequent tmux command must use that same socket. Run `herdr plugin link <owned-plugin-dir>` with the private environment.
3. Copy `/bin/sleep` to `<owned-dir>/bin/codex`. Create two workspaces with `herdr workspace create --cwd <owned-dir> --label <distinct-label> --focus`, then a named tab in each with `herdr tab create --workspace <id> --label <distinct-tab> --focus`. Record the returned workspace/tab/pane IDs.
4. In each test pane, use `herdr pane run <pane-id> <command>` to set a distinct terminal title with an OSC 0 escape, print a unique marker and `exec <owned-dir>/bin/codex 300`. Record each PID before exec. These are sleeping synthetic processes, not AI calls. Wait for both rows in `herdr agent list`; assign names with `herdr agent rename <pane-id> <name>`.
5. Focus the first workspace, tab and agent using their CLI focus commands. Invoke the exact picker binary with `--open` under the private environment. Capture plain and ANSI screens with `tmux -S <owned-socket> capture-pane -p [-e] -t verify:0.0`.
6. Verify one outer `Agents` title/border, no inner frame or preview, both thread names, legible workspace/tab labels and visible numbers. The popup is 95% of Herdr's content width and 90% of its content height, excluding the sidebar/status bar. Allow one cell of rounding.
7. Read the second thread's number from the screen and send it with `tmux -S <owned-socket> send-keys -t verify:0.0 <digit>`. Require the popup to disappear and the second terminal marker to appear. Check `herdr pane current`, `workspace list` and `tab list` against the recorded IDs. Repeat cancellation with Escape and require focus to remain unchanged.
8. On success or failure, stop the private Herdr server and private tmux server. Verify their sockets refuse connections and recorded synthetic PIDs have exited before removing the owned directory. Retain captures, CLI results, binary hash and cleanup proof outside it. Never kill by process name or use a user's socket.
