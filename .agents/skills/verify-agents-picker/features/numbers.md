# Visible numbers

Run `python3 .agents/skills/verify-agents-picker/scripts/verify.py run numbers --evidence /tmp/picker-numbers-001`.

Each case starts a fresh real executable. Labels are read from the captured screen, never calculated from source or internal state. The first ten visible rows must read 1 through 9 then 0; further visible rows have no label. Each digit is tested separately against its exact shown target and must immediately exit through workspace, tab, agent focus.

Additional cases cover unavailable digits with two agents, all digits with an empty list, hidden targets in a small viewport, no list area, Alt and Ctrl digits, search query `10`, scrolling, resize after scrolling, status-driven priority reorder, and selected-agent identity across reorder. Ctrl digits use CSI-u because legacy terminal encodings cannot distinguish all Ctrl+digit combinations. Unavailable digits wait through a refresh and must neither focus nor exit.

Status evidence checks blocked ◉, done ●, idle ✓, unknown ○, and multiple working spinner frames. Thread evidence checks that explicit names replace automatic titles, with a terminal-title fallback for unnamed threads. All cases reject agent reads and terminal observe calls. These are list/input boundary checks; actual Herdr event delivery remains a separate integration check.
