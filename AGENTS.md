# Agents picker fork

Keep the plugin ID `yxhta.agents-picker`; existing Herdr keybindings use it.
Install this fork from `aryan-binazir/herdr-agents-picker`, pinned to a reviewed
commit. Update the dotfiles pin and scoped Herdr `AGENTS.md` when shipping.

Herdr owns the popup's border and `Agents` title. The TUI draws a full-width
list inside one untitled rounded panel, with keyboard hints below. Keep the
manifest's popup size versioned. Number shortcuts address the first ten visible
rows in navigation mode; search-mode digits are text.
Status comes from Herdr integrations, not terminal-output inference.

Read README.md for setup, keys and Rust checks. After UI/input changes, follow
`.agents/skills/verify-agents-picker/SKILL.md` for isolated PTY verification.
Use private Herdr/tmux servers and synthetic agents. Never link an installed
plugin to a temporary worktree. Report native macOS gaps separately from Linux
or fixture results.
