# AI and contributor operating rules

These rules apply to every AI agent or human contributor working in this repository.

## Scope and honesty

- The active product is Voxel Engine, a clean-room Rust voxel engine.
- Do not describe the project as complete, optimized, Minecraft-compatible, or vanilla-parity compliant unless tests and evidence support that claim.
- Never claim an action was performed unless its result was actually verified.
- State blockers, failed commands, assumptions, and incomplete work clearly.

## Clean-room boundary

- Do not copy, decompile, reconstruct, or include Minecraft or other proprietary source code/assets.
- Do not bypass authentication, ownership checks, paid access, server rules, or licensing.
- Use original code, original assets, public specifications, and independently authored tests.
- Treat ordinary Minecraft-server compatibility and 100% vanilla parity as future research/acceptance goals, never current assumptions.

## Required workflow

1. Inspect the current tree and Git status before changing files.
2. Read `README.md`, `DECISIONS.md`, and `ACTION_LOG.md` before architectural work.
3. Make focused, reviewable changes.
4. Add or update tests for behavior changes.
5. Run formatting, tests, release builds, and relevant checks.
6. Record the action, result, decision, and next step in `ACTION_LOG.md`.
7. Record material architecture choices in `DECISIONS.md`.
8. Update user-facing instructions in `README.md`.
9. Commit and tag usable milestones; never commit credentials, generated builds, local worlds, or external source checkouts.

## Handoff requirements

Every AI handoff must state:

- What was changed.
- What was verified and the exact result.
- What remains incomplete.
- The next recommended step.
- Any assumptions or user testing required.

These rules guide agents operating in this repository; they cannot override platform, system, safety, or user instructions.
