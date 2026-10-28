# Launch and close

Run `python3 .agents/skills/verify-agents-picker/scripts/verify.py run launch --evidence /tmp/picker-launch-001`.

q exits with code zero and no focus requests. Invoke the same exact executable with `--open`; it must request `plugin pane open --plugin yxhta.agents-picker --entrypoint picker --focus` successfully. The fixture does not render a Herdr popup.

Ctrl+C and navigation-mode Escape are additional close paths. The private-Herdr driver verifies the popup title, border ownership, geometry, and real focus with synthetic processes. Never link into a live user configuration for verification.
