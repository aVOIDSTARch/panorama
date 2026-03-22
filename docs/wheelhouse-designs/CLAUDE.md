# Wheelhouse — Agent instructions

**Meta guidance:** Apply **ai-docs/meta-agent.md** in every situation. It holds cross-project conventions, working style, coding standards, workflow, slash commands, and ideas system. This file holds only project-specific items.

## Project-specific

- **Wheelhouse** = governed resource layer between agents and the outside world; GitHub is one node. See **project-seed.md** for concept, architecture, design decisions, and immediate next step (resource manifest schema).
- **Stack (current):** PyGithub, private repo, PAT env (`GITHUB_TOKEN`), dedicated agent branch. New code: prefer TypeScript per meta-agent; if this repo stays Python for the broker, document why.
- **Paths:** Repo root = this directory; no prefix in this file means project root.
