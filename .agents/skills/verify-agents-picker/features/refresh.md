# Refresh

Run `python3 .agents/skills/verify-agents-picker/scripts/verify.py run refresh --evidence /tmp/picker-refresh-001`.

Select Beta, remove it from the fixture, then press r. The count must become `1/1`. Restore Beta and wait for automatic refresh to produce `2/2`. q exits without focus calls. Poll expected screen state instead of accepting a fixed delay.

`numbers` checks priority reorder and selected identity. Search-mode Ctrl+R, persisted status-event ordering, and real Herdr event delivery remain additional cases. Use owned configuration and plugin state for them.
