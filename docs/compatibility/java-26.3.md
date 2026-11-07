# Java Edition 26.3 compatibility baseline

Status: **baseline selected; per-feature conformance not started**

## Reference identity

- Edition: Minecraft: Java Edition
- Release: **26.3 stable** (not a snapshot, pre-release, or release candidate)
- Official release announcement: [Minecraft Java Edition 26.3](https://www.minecraft.net/en-us/article/minecraft-java-edition-26-3)
- Announcement date: 2026-09-15
- Distribution/build identity: official Launcher release entry `26.3`; capture the resolved version manifest, client/server artifact hashes, and Java runtime for each black-box run before marking a row verified.
- Reason selected: this is the stable release currently identified by the project’s Java 26.3 movement experiments, and the official announcement confirms it is released. The earlier experiment did not establish a project-wide baseline.

## Initial conformance cell

- Host: Windows x86-64 desktop; the project’s initial target platform.
- Game context: unmodified single-player world, including its integrated server.
- Content: built-in 26.3 data and resources, default resource/data packs, no mods, no shaders, no user pack overrides.
- Preferences: default game rules and world options unless a scenario states otherwise; English (US); keyboard and mouse.
- Account/network-dependent features: exclude from this cell and track as separate authorized-flow scenarios. This exclusion prevents an unqualified complete-parity claim.
- Other platforms, server contexts, input devices, languages, and non-default configurations: separate support cells; none are verified by this profile.

## Evidence rules

The official release announcement establishes the release identity and release date; it does not establish detailed behavior. Each compatibility-matrix case must cite the public reference or lawful black-box observation used to author it, capture the exact installed build/runtime and effective settings, and include setup, actions, expected state/feedback, observed result, tolerances, and reviewer. Use original synthetic fixtures. Do not commit game files, proprietary assets, credentials, or reference worlds.

No behavior has been marked verified by selecting this baseline. Artifact hashes and runtime details are required in individual run records; they must be read from the user's authorized installation rather than downloaded or checked into this repository.

## First conformance work

Create domain-specific behavior cases beginning with the P1 player/world loop, then add every catalog entry and interaction required by PRD sections 7 and 8. Maintain the full inventory in a machine-reviewable matrix; a sample or a family-level row cannot establish coverage.
