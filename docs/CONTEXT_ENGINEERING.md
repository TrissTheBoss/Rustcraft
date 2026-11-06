# Hermes context reduction

Hermes uses a small durable memory file instead of replaying the entire conversation on every request.

## Rules

- Keep `CONVERSATION_MEMORY.md` as a concise current-state summary.
- Keep `ACTION_LOG.md` append-only and summarize old milestones rather than duplicating source code.
- Load only the files relevant to the current task.
- Separate stable decisions, current implementation, and next steps.
- Store transient prompts/responses under ignored Hermes folders.
- Prefer tests and Git history as evidence instead of repeating old conversation text.

These are practical context-reduction techniques; Hermes does not claim a universal optimal compression algorithm.
