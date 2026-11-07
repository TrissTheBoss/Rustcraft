# Product Requirements Document

**Product:** Voxel Engine (working name)
**Status:** Product direction and end-state requirements; implementation is incremental
**Owner:** Repository owner
**Last updated:** 2026-11-07
**Primary target:** Desktop Java Edition-style voxel sandbox, built as an original Rust product
**Compatibility ambition:** Eventual, evidence-backed parity with a version-pinned Java Edition release

## 1. Product vision

Build a polished, dependable voxel sandbox that can eventually reproduce the observable gameplay, world, interface, rendering, resource, and multiplayer behavior of a specifically identified Minecraft: Java Edition release. The engine, code, specifications, tests, and bundled assets are to be independently authored. Java Edition parity is the long-term acceptance destination, not a claim about the current prototype and not permission to copy proprietary implementation or distribute game assets.

The product should be usable as a standalone game with original default content. Where a user owns a local Java Edition installation, an optional local-only resource import path may use user-selected or locally discovered assets for that user's runtime. Imported material must remain local, must not enter Git, releases, CI artifacts, or shared test fixtures, and must not bypass account, authentication, or ownership controls.

## 2. Product principles

1. **Clean-room implementation.** Write original code and independently authored specifications. Use public documentation and observable behavior as references. Do not copy, decompile, port, or incorporate proprietary source or copied protocol implementations.
2. **Evidence before claims.** Name the exact reference release, platform, mode, and test coverage for every compatibility statement. A partial implementation is described as partial.
3. **Standalone usefulness.** The engine must launch and play with original content without requiring a Java installation, account, downloaded proprietary asset, or external server.
4. **Data-driven content.** Blocks, items, entities, recipes, dimensions, and world-generation inputs should be represented through stable, validated data definitions where practical.
5. **Deterministic foundations.** Simulation, persistence, replay, and tests should have explicit tick and ordering rules, with determinism where the reference behavior and platform allow it.
6. **User control.** Settings are visible, explainable, resettable, persisted safely, and distinguish local preferences from world/server rules.
7. **Incremental delivery.** Each milestone is independently buildable and reviewable; unfinished features are not presented as complete.

## 3. Users and primary use cases

- **Solo player:** create, configure, save, reopen, pause, and continue a world; explore, build, mine, craft, fight, and manage inventory.
- **Creative builder:** choose a mode, access a complete item catalog, place and remove blocks, fly, and tune presentation controls.
- **Survival player:** manage health, hunger, equipment, resources, crafting, death, respawn, and progression.
- **Multiplayer player:** discover or enter a server, understand connection/security failures, play with server-authoritative state, and leave safely.
- **Accessibility-conscious player:** remap controls, tune readable UI and visual effects, and use supported accessibility options without losing gameplay information.
- **Developer/tester:** inspect worlds, capture deterministic replays, view diagnostics, profile performance, and run conformance suites without shipping private assets or world data.

## 4. Product scope and boundaries

### 4.1 End-state in scope

- A desktop game client and the supporting engine systems needed for single-player and multiplayer play.
- World creation, loading, saving, backup/recovery, dimensions, terrain, structures, blocks/states, fluids, lighting, entities, items, crafting, combat, user interface, sounds, and progression.
- Single-player and LAN/direct multiplayer; eventual interoperability with the selected Java Edition release only after separate protocol and security review.
- A complete settings experience, input remapping, accessibility, diagnostics, and developer-facing validation tools.
- An independently authored default resource set, plus a clear user-owned local resource import workflow where lawful and technically supportable.
- Long-term Java Edition behavioral parity as defined by the versioned matrix in Section 8.

### 4.2 Explicit constraints

- No Minecraft source, decompiled code, proprietary implementation, copied protocol implementation, credentials, authentication bypass, or ownership-check bypass.
- No redistribution or commitment of extracted proprietary assets. The local extractor is read-only against the source JAR and writes only to ignored local storage.
- No claim of complete parity, server compatibility, or vanilla equivalence until the relevant release, test matrix, and results are documented and reviewed.
- No guarantee that the project can use a particular name, logo, exact UI artwork, sounds, fonts, or other protected presentation materials. Product branding and legal review remain release requirements.
- Mods and mod-loader compatibility are not part of the initial parity gate. Their support requires a separately reviewed extension API and compatibility scope.

### 4.3 Current state (orientation only)

Repository documentation describes a Rust prototype with world/chunk storage, deterministic terrain, fixed-step simulation and collision foundations, movement, CPU rendering, and a native Windows window path with save/create/open/copy/delete/backup recovery plus editable name/seed/mode/difficulty/lock metadata. Save format v8 persists inventory, selected hotbar slot, bounded dropped-item state, immutable terrain-generation seed, generator version, health, fall tracking, and the `keepInventory` world rule; v1-v7 remain readable with defaults for fields introduced later. The Windows runtime generates and renders a 5x5 horizontal chunk region around the player from versioned immutable world inputs. It evicts generated chunks outside that region, preserves edited and legacy chunks, and serializes provenance so clean terrain can be regenerated after reload. A GPU presentation path, local-only texture extraction, and a prototype loot/drop system exist. Survival held-click mining tracks a target and fixed-tick progress, cancels on release/retarget, and completes after configured prototype durations; Creative breaking remains immediate. Mining profiles accept tool speed and harvest tiers, but active gameplay currently supplies only hand stats. Survival block breaking spawns configured prototype loot atomically. Number keys and mouse-wheel notches select the persisted hotbar slot. Creative middle-click picks registered prototype blocks into the hotbar. The Windows inventory screen has a case-insensitive searchable catalog of the ten prototype blocks; choosing a result selects it into the hotbar only in Creative mode. Selected hotbar items now drive mapped prototype block placement; Survival consumes one item on success and Creative preserves stacks. Empty Creative slots still fall back to the legacy three-block selector; the catalog only covers ten prototype blocks and is not complete. Fixed-step updates advance gravity, pickup, partial inventory transfer, and expiry, while CPU and GPU paths render block-colored item meshes. A modal native title form opens at startup; Resume World enters the active save, Singleplayer opens the existing Worlds menu, and Options returns to the title. Pause can save and return to title. A prototype health value and fall tracker now drive Survival/Adventure fall damage, a death screen, respawn, and save migration. Health, threshold, damage formula, death-drop pickup timing, spawn selection, and persistence timing are provisional implementation choices and have not been compared with the pinned reference; `keepInventory` can be toggled in world creation/editing, is persisted, and controls death drops, while other game rules remain unimplemented. Hunger, other damage types, healing items, and respawn anchors are also missing. A native in-world overlay draws an original crosshair, hotbar with prototype item swatches/counts, and health pips for Survival/Adventure. HUD scale settings, hunger and other status bars, effects, notifications, and reference screen behavior remain unimplemented.

The block registry includes prototype collision shapes for slabs, carpets, snow layers, farmland, paths, fences, and panes, and live player movement plus block ray selection query those shapes (including shapes extending above their owning cell). These are currently fixed prototype block IDs: upper/lower slab and snow-layer count states and four-direction fence connection masks are persisted and queryable through engine APIs, but are not yet editable in-game or integrated with a complete catalog. Inner world payload v4 adds fence masks while v1-v3 remain readable. The renderer emits simple block-local boxes and culls shared/partially covered faces. Fence masks control prototype rail collision and visual boxes. Placement and breaking recompute nearby connections under a prototype full-cube/fence rule; terrain streaming refreshes connections at newly loaded horizontal chunk perimeters; release-specific connection behavior remains unimplemented. Prototype fences and panes have separate collision, selection, and visual boxes; these definitions are not reference models and do not implement the PRD block/state catalog.

A native inventory prototype displays all main, armor, and offhand slots, cursor contents, and hotbar selection. In-world Q drops one selected item, Ctrl+Q drops its stack, and F swaps the selected hotbar slot with offhand using validated inventory transactions. It uses the inventory transaction model for primary/secondary slot clicks, can drop a cursor stack through the persisted world-drop lifecycle, and refuses to close if the remaining cursor stack cannot be returned safely. Escape opens a native pause prototype; it pauses local simulation, resumes with prior cursor-capture state, and saves before Save & Quit. A save failure leaves the screen open. Its Options screen applies and separately persists field of view, mouse sensitivity, and mouse-Y inversion; it also rebinds the seven currently implemented movement actions with conflict detection, confirmation for intentional shared keys, and Escape cancellation. Sprint and sneak each support Hold or Toggle. Settings format v3 migrates earlier preferences; v1/v2 use Hold defaults. Corrupt, future, and invalid data falls back safely. The settings screen, its defaults, and menu behavior have not been compared against reference 26.3. Other video/input categories, accessibility, audio, language, privacy, disconnect, focus/accessibility polish, and final visual presentation remain incomplete. The inventory still lacks crafting, item use, and recipe access. These features and the mining/drop values are prototypes, not reference-release behavior. Seed regeneration, difficulty gameplay effects, and tool inventory integration also remain missing. This is not a complete game loop, world editor, content catalog, settings system, multiplayer client, or compatibility implementation. See `README.md`, `DECISIONS.md`, and `ACTION_LOG.md` for the maintained implementation record; this PRD is the destination specification, not evidence that requirements are already met.

## 5. Product experience and information architecture

### 5.1 Launch and main menu

The first-run flow must explain the product, offer a short control guide, and allow play with original assets. The main menu provides:

- **Singleplayer** → world list, create world, edit world, delete with confirmation, backup/recovery, and open-folder action.
- **Multiplayer** → server list, direct connect, add/edit/remove server, refresh, connection status, and disconnect.
- **Options** → the settings groups in Section 6.
- **Accessibility** → a direct route to high-impact accessibility settings.
- **Resource Packs** → inspect, enable/disable, order, and validate supported local packs.
- **Credits / Legal / About** → version, licenses, third-party notices, compatibility status, and project links.
- **Quit** → close cleanly and flush settings and world writes.

All destructive actions require a clear target and reversible recovery where feasible. Keyboard navigation, focus visibility, escape/back behavior, and screen-reader labels for supported controls must be consistent.

### 5.2 World selection and creation

World list entries show name, last-played time, game mode, difficulty, seed visibility preference, data version, and health/upgrade status. Users can create, rename, copy, open the save folder, export a user-owned backup, recover a prior backup, or delete a world after confirmation. Corrupt or newer-version worlds must be identified before mutation; upgrades must preserve a restorable backup.

World creation exposes:

- World name and optional seed, with an explicit random-seed action.
- Game mode: Survival, Creative, Adventure, and Spectator, subject to the chosen reference release's behavior.
- Difficulty: Peaceful, Easy, Normal, Hard; lock/unlock behavior where applicable.
- World type/generator and available dimensions, only when implemented and clearly labeled.
- Structures, bonus chest, and other generator options when supported by the target release.
- Cheats/commands and data-rule options, with their effect on progression/advancements explained.
- Advanced options for experiments or unsupported features, isolated from ordinary defaults and recorded in world metadata.

### 5.3 In-world HUD and screens

The in-world screen includes a configurable crosshair, hotbar, health, hunger, armor, air, experience, mount health, status effects, boss bars, subtitles, chat/system messages, and contextual interaction feedback. Each element appears only when relevant under the selected mode/rules and must not obstruct core interaction. HUD scale and accessibility options apply consistently.

Required screen families include pause, inventory, crafting, container, furnace/smelting, anvil, enchanting, brewing, trading, beacon, command/configuration screens where available, death/respawn, chat, advancements/progression, statistics, player list, and multiplayer disconnect/error screens. Each screen must define focus order, input capture, safe close behavior, and server rejection handling.

### 5.4 Screen inventory and behavior

The following screen inventory is required for the end product. Screen presence, exact layout, labels, available actions, and transitions are pinned to the chosen reference version; this list defines coverage and does not authorize copying protected artwork.

| Screen | Required content and actions | Required behavior |
|---|---|---|
| Title/main menu | Primary destinations, product/build information, accessibility entry, credits/legal, quit | Opens at a predictable focus target; disabled destinations explain why they are unavailable. |
| World list | World identity, mode, last-played, version/upgrade warning, create/edit/copy/delete/play | Selection survives refresh; destructive actions name the world and provide a recovery path where possible. |
| Create/edit world | Name, seed, mode, difficulty, generator options, rules, advanced settings | Shows effective defaults; validates input; records all generation-affecting settings before first chunk creation. |
| Multiplayer list | Saved server entries, address/name, status, add/edit/remove, refresh, direct connect | Does not connect on selection alone; explains protocol, timeout, refusal, and authentication errors without exposing secrets. |
| Connecting/disconnecting | Progress, cancel, timeout, retry, server identity | Cancellation closes sockets and pending state; failure returns to a usable prior screen. |
| In-world HUD | Crosshair, hotbar, relevant status bars, effects, boss/title/action-bar messages, chat, subtitles | Reflects authoritative game state; elements respect mode, rules, and user visibility settings. |
| Pause menu | Resume, options, accessibility, save/quit, disconnect, feedback/report route if enabled | Pauses local simulation according to reference rules; does not imply a remote server is paused. |
| Player inventory | Equipment slots, crafting area, inventory, offhand, cursor/held stack, recipe access | Stack limits, drag/split, shift-transfer, swap, drop, and close-return behavior are consistent and atomic. |
| Creative inventory | Search, item categories/tabs, item stacks, inventory, saved hotbars if in scope | Search and category changes preserve clear focus; item selection follows the target release's mode rules. |
| Crafting/container | Inputs, outputs, recipe book, storage slots, transfer/close behavior | Server rejection or changed contents cannot duplicate, delete, or locally invent items. |
| Processing station | Input/fuel/output, progress, fuel state, recipes, take-output action | Progress and outputs persist correctly; invalid inputs produce no phantom result. |
| Anvil/enchanting/brewing | Inputs, costs/requirements, candidate results, active effects | Cost, resource consumption, and result are committed as one validated transaction. |
| Trading | Offers, costs, stock/lock state, villager identity/profession where applicable | Offer refresh, stock, experience, and inventory effects follow world/server authority. |
| Death/respawn | Cause/summary where available, respawn, return-to-title | Applies drop/keep-inventory and respawn rules exactly once; reconnect cannot duplicate a death transition. |
| Chat/command | Input, history, suggestions, command feedback, filtering, links | Chat visibility and permissions follow the world/server; untrusted text is escaped and links require explicit activation. |
| Advancements/statistics | Tree/list, progress, criteria, search/filter where applicable | Progress is persisted, synchronized, and attributed to the correct world/player. |
| Debug/help | Supported diagnostics, key hints, accessible descriptions | Developer-only information is opt-in and never required for ordinary play. |
| Error/confirmation dialogs | Clear cause, affected object, safe next action, cancel/back | No destructive default focus; keyboard and controller navigation cannot accidentally confirm. |

For every screen, the interaction specification must record: entry conditions, allowed input, focus order, escape/back semantics, pause semantics, network authority, state transaction boundary, localization behavior, failure recovery, and reference-version scenarios.

### 5.5 End-to-end player journeys

1. **First launch:** accept/read legal notices as applicable → choose language and accessibility preferences → view controls → start with original resources. No local game installation is required.
2. **Start a world:** choose singleplayer → create world → review mode/difficulty/seed/rules → create → spawn with a short, skippable control hint → pause and save/quit → reopen the same world with the same state.
3. **Survival loop:** observe a resource → target and mine it → collect the correct drop → craft or use it → receive clear success/failure feedback → survive environmental/entity threat → save and reload without losing state.
4. **Creative loop:** choose Creative → find an item through catalog/search → place, orient, replace, and remove blocks → fly and inspect the build → save and reopen.
5. **Join a server:** add or direct-connect → review host and pack prompts → connect → see server-authoritative state/chat → handle lag or disconnect → return to menu without corrupting local worlds.
6. **Recover a problem:** detect a damaged/newer save or invalid pack → explain the issue → offer a backup, disable, or safe read-only path → allow recovery without silently overwriting the only copy.
7. **Change settings:** open settings from menu or pause → search or browse → read the effect → preview/apply → confirm or automatically revert risky display changes → persist → restart if a change explicitly requires it.

## 6. Settings requirements

Settings are grouped, searchable, localized, and individually described. Every setting has a default, valid range/options, persistence scope, live-apply behavior, and reset path. Invalid or unsupported saved values fall back safely and are logged without preventing launch. “Reset” offers current section and all settings, with confirmation for the latter. Settings changes must not silently alter world rules or server authority.

### 6.1 Video and graphics

- Display selection, windowed/borderless/fullscreen mode, resolution, refresh rate, VSync, frame-rate cap, and UI scale.
- Render distance and simulation distance, with supported ranges and estimated resource impact.
- Graphics quality preset and explicit per-feature controls: smooth lighting, clouds, particles, entity distance, biome colors, weather, mipmaps/filtering, anti-aliasing, shadows if implemented, and transparency quality.
- Brightness/gamma within a documented range; fullscreen brightness never overrides OS display calibration.
- View bobbing, camera shake, attack indicator, field of view, FOV effects, and distortion effects.
- View distance fog/visual effects where supported; show chunk borders and debug overlays only as opt-in diagnostics.
- GPU/device selection when the platform API permits; renderer selection and fallback status are visible in diagnostics.
- Apply/revert workflow for settings that can make the display unusable, with an automatic timeout rollback.

### 6.2 Controls and input

- Rebind every gameplay, UI, chat, debug, and accessibility action; detect conflicts and allow intentional shared bindings.
- Keyboard/mouse sensitivity, invert axes, raw input option where supported, acceleration policy, mouse capture behavior, and cursor release shortcut.
- Toggle/hold choices for sprint, sneak, and other eligible actions; repeat delay/rate for UI navigation. The prototype currently supports separate Hold/Toggle choices for sprint and sneak only; key repeat settings and matching reference defaults are outstanding.
- Primary/secondary interaction and attack bindings; scroll behavior; hotbar slot bindings; inventory, drop, swap, and offhand actions.
- Gamepad support is a planned end-state capability: device selection, remapping, dead zones, sensitivity, vibration, glyphs, and disconnect/reconnect behavior. Do not expose controls until implemented.
- Input rebinding must be keyboard operable; Escape cancels an in-progress bind, and defaults can be restored per category or globally.

### 6.3 Audio

- Master, music, weather, blocks, hostile creatures, friendly creatures, players, ambient/environment, and UI volumes. Voice chat, if added as an original extension, is outside the reference parity profile.
- Output device selection where supported, subtitles, directional audio behavior, and mute-on-focus-loss.
- Audio device loss/reconnect must not crash the game; volume controls must update without relaunch where possible.

### 6.4 Accessibility and interface

- UI scale, text size where independent, high-contrast/readability options, color-vision options that do not convey information by color alone, reduced flashing, reduced camera shake, and reduced motion.
- Subtitle enablement, size/background/opacity, speaker labels, and directional cues.
- Chat text size, background opacity, line spacing, timestamp/filter options, and safe link handling.
- Narration/screen-reader integration where supported by the platform; all menus expose names, values, state, and actionable descriptions.
- Hold/toggle alternatives for repeated actions, interaction timing assistance where consistent with game rules, and clear warnings when accessibility options affect multiplayer policy.
- Avoid time-critical UI-only information; provide equivalent text/audio cues for supported events.

### 6.5 Language, chat, and privacy

- Display language selection with missing translation fallback; text must not overflow or clip under longer translations.
- Chat visibility, opacity, size, links, profanity/filter policy where applicable, and notification choices.
- Telemetry/crash-report controls must state exactly what is sent, be opt-in unless separately approved, minimize data, and provide deletion/retention information.
- Do not collect authentication secrets or upload worlds/logs by default. Diagnostic bundles are user initiated and previewable.

### 6.6 Resource packs and local assets

- List available packs with name, version, source path, validation result, and compatibility status.
- Enable/disable and reorder packs; preview changes; detect missing/invalid resources; restore defaults.
- User-supplied packs remain local unless the user deliberately exports them. Importing a local Java client archive is explicit or a clearly disclosed local-only convenience, read-only, and restricted to documented paths/formats.
- Extracted assets are stored under ignored local data, never in source control, CI, releases, crash uploads, or distributed test fixtures. Do not invoke network access or authentication to obtain them.

### 6.7 Chat, multiplayer, and safety

- Server list controls, saved address privacy, connection timeout, status display, chat visibility, and server resource-pack prompt behavior.
- Clearly distinguish client preference from server-enforced rules. Never weaken authentication, certificate, ownership, or trust checks for convenience.
- Server-provided links/text are sanitized and cannot execute local commands or open external content without an explicit user action.

### 6.8 Storage and diagnostics

- Data directory display/open action, save backup interval and retention, log verbosity, crash dump option, and cache management.
- Storage tools report estimated size and distinguish deletable cache from worlds/settings. Cache clearing cannot delete worlds.
- Diagnostic overlay toggles for FPS/frame time, memory, chunk counts, position, velocity, and renderer/backend. Sensitive local paths are hidden from screenshots/reports unless the user chooses to include them.

### 6.9 Settings contract and initial menu map

Top-level settings categories are **Video**, **Controls**, **Audio**, **Language**, **Accessibility**, **Chat**, **Resource Packs**, **Multiplayer/Privacy**, **Storage**, and **Diagnostics**. Category names may be adapted to the pinned reference version, but every setting below must have one obvious home and be searchable by label and common synonym.

Each setting specification must include an identifier, user-facing name and help text, control type, legal values/range, default, platform capability, effective value, apply timing, persistence scope, migration behavior, reset scope, and one acceptance scenario. Enumerated values must have a safe fallback. Numeric settings must clamp or reject out-of-range values, not wrap. Changes must be saved atomically and surfaced as pending restart when live application is impossible.

Current implementation subset: local preferences store field of view, mouse sensitivity, mouse-Y inversion, bindings for the seven movement actions, and separate Hold/Toggle choices for sprint and sneak in a versioned, checksummed settings file. These bindings can be changed from the Pause → Options screen; conflicts are confirmed before sharing, while Escape, inventory, and hotbar keys are reserved. Versions 1 and 2 migrate with Hold defaults; v1 also receives default bindings. The field-of-view/sensitivity ranges and defaults are engine prototype values; they are not verified against Java Edition 26.3. All other settings in this section remain required and unimplemented.

Settings are divided into **account-independent local preferences**, **per-world rules**, **server-controlled values**, and **temporary session state**. The UI marks the latter three distinctly. Local preferences never silently mutate world data. A setting unsupported by the current renderer/device remains visible with an explanation or is hidden consistently; it is never silently accepted as active.

The versioned preference catalog must include at least these setting behaviors, with exact labels, values, and defaults taken from the chosen reference release when matching a reference option:

- **Video:** display/output, window mode, resolution/refresh, GUI scale, brightness, VSync, frame cap, FOV and FOV effects, view bobbing, camera effects, render and simulation distances, graphics preset, clouds, particles, entity distance, biome blend, smooth lighting, transparency, mipmap/texture filtering, anti-aliasing, and supported renderer-specific quality controls.
- **Controls:** forward/back/left/right, jump, sneak, sprint, attack/destroy, use/place, pick block, drop, inventory, swap hands, hotbar slots and selection, chat, command, player list, advancements, perspective, screenshot, narrator/accessibility shortcuts, debug actions, and every screen's navigation/confirm/cancel actions.
- **Audio:** master and each reference category, output device, subtitles, directional cues, and focus-loss behavior.
- **Accessibility:** narration scope, subtitles, chat text/background, UI scale, high contrast/readability, color filters, reduced motion/flashing/camera effects, and hold/toggle alternatives where permitted.
- **Language and chat:** language, chat visibility/opacity/scale, links, filtering/reporting controls, notification behavior, and narration.
- **Resource/content:** pack enable/order, compatibility warnings, prompt policy for remote packs, and fallback to original bundled resources.
- **Privacy/network:** telemetry/crash data, server list visibility, local discovery, remote resource-pack prompts, and credential/session status without displaying secrets.

Where the reference has a slider or mode, the matching profile uses the same meaningful range and semantics. Added accessibility or quality settings are marked as product extensions and kept out of the reference profile unless the reference has an equivalent. Hidden developer controls are not mixed into ordinary player settings.

## 7. Functional requirements by system

### 7.1 Runtime, platform, and lifecycle

- Launch, update settings, enter/leave worlds, recover from recoverable renderer/audio/network errors, and exit without corrupting saves.
- Window resize, focus loss, alt-tab, monitor change, device loss, and sleep/resume have defined behavior.
- Initial target is desktop Windows; platform support beyond Windows is a separate verified milestone. Architecture must avoid unnecessary platform coupling.
- Use a fixed simulation tick with a separate presentation cadence. Frame stalls must not cause unbounded catch-up or silently change world time.

### 7.2 World and simulation

- Stable integer world/chunk/block coordinates, explicit grid contracts, validated block states, and deterministic update ordering.
- Chunk generation, streaming, loading, unloading, lighting, meshing, block updates, fluid propagation, ticking, and persistence have bounded work and observable diagnostics.
- World generator inputs are versioned; a seed plus generator version recreates the same generated result where deterministic behavior is specified.
- Save data is versioned, crash-safe, recoverable, and never silently downgraded. Autosave and backups are bounded and configurable.
- Dimensions, portals, structures, biomes, weather, time, and environmental rules must match the selected reference release at the parity gate.

### 7.3 Blocks, items, and interaction

- Catalog all target-release block and item identifiers, states, shapes, collision, selection, placement, breaking, drops, durability, sounds, light, fluid behavior, and tool requirements.
- Model partial collision/selection shapes and state-dependent geometry; do not assume every block is a full cube.
- Support contextual interaction, placement orientation, replaceability, reach checks, mining progress, tool effects, inventory insertion, and failure feedback.
- Every catalog entry needs a provenance note for independently authored behavior, source reference, and conformance fixture. Unknown definitions fail safely and are surfaced in diagnostics.

### 7.4 Player, entities, and game modes

- Player movement, collision, camera, status, health, hunger, armor, effects, equipment, inventory, death, respawn, and spectator transitions are server-authoritative in multiplayer.
- Implement all selected-release entity types and behavior, including AI goals, navigation, damage, drops, breeding/spawning, persistence, passenger/vehicle rules, and interaction.
- Implement mode-specific abilities and restrictions for Survival, Creative, Adventure, and Spectator; difficulty and world rules affect behavior as specified.
- Movement conformance covers acceleration, friction, gravity, jump, sprint, sneak, swimming, climbing, flying, knockback, collision edges, step behavior, status effects, and tick ordering.

### 7.5 Crafting, progression, and commands

- Implement recipes and recipe discovery, crafting interfaces, furnace-family processing, trading, enchantment, brewing, repair/anvil, and all supported progression systems.
- Implement commands and permission/feedback behavior only when supported by the selected release and execution context. Parse and validate safely; never execute shell commands.
- Advancements, statistics, achievements/progression notifications, and scoreboard-like features require versioned persistence and multiplayer authority.

### 7.6 Rendering and content presentation

- Correct camera projection, frustum clipping, chunk visibility, block face/state geometry, transparency ordering, lighting, ambient effects, particles, sky, weather, entities, held items, UI, and animation.
- Support high-DPI display, resize, variable refresh, and GPU fallback; provide stable minimum and recommended hardware targets before release.
- Default art, UI, audio, fonts, and icons are original or properly licensed. User-owned local assets may be used only through the constrained local workflow in Section 6.6.
- Performance settings must not change simulation outcomes or world data.

### 7.7 Sound and feedback

- Provide original/licensed sounds with spatial attenuation, category volumes, subtitles, and deterministic event triggers where relevant.
- Interaction feedback must make successful and rejected actions legible across sound, text, animation, or other accessible cues.

### 7.8 Multiplayer and networking

- First deliver an original protocol and an engine-owned server with explicit protocol versioning, server authority, input validation, rate limits, timeouts, encryption/authentication design, and secure persistence boundaries.
- Client prediction/reconciliation, interpolation, chunk/entity replication, inventory actions, chat, disconnect, reconnect, and lag behavior have testable contracts.
- Java server interoperability is a separate compatibility track: independently implement only from lawful public specifications and observed conformance, with legal/security review. No copied protocol code and no authentication bypass.
- Server list and connection UI clearly show pending, connected, incompatible version, rejected, timed out, and disconnected states. Do not lose unsaved single-player data due to a network failure.

### 7.9 Persistence, migration, and recovery

- Settings and worlds use separate schemas and migrations. Each migration is tested against fixtures authored without redistributing proprietary data.
- Writes use atomic replacement/journaling or an equivalent crash-safe scheme; save failures are visible and actionable.
- Backup recovery supports preview and restores to a new copy before replacement where feasible.
- User can export/import their own worlds in a documented format, with path traversal and oversized-input protections.
- World interoperability must explicitly state whether user-owned Java Edition world saves from the pinned baseline can be imported, edited, and exported. The end-state parity plan includes independently implemented import/export for documented, user-provided save formats where lawful and technically supportable. Import starts from a copy, reports unsupported content, retains a recoverable original, and never downloads or redistributes content. Do not claim world-format compatibility from visual similarity alone.

### 7.10 Developer and support tooling

- World/chunk inspector, block/state inspector, collision/shape visualizer, resource validator, replay capture, deterministic seed fixtures, benchmark runner, and structured logs.
- Conformance reports identify reference release/build, platform, scenario, expected/observed result, tolerance, pass/fail, and known exclusions.
- Crash reports redact secrets and local asset content by default. Logs rotate and have explicit size limits.

### 7.11 Resource and data pack behavior

Support the pinned vanilla resource/data pack versions and their documented ordering, dependency, incompatibility, metadata, fallback, and error behavior. A pack is validated before activation; invalid packs are isolated and can be disabled without blocking startup. Resource packs may replace supported presentation resources but cannot execute arbitrary native code. Data packs may define only explicitly supported data-driven content; scripts, commands, or functions follow the target game's permission and execution rules and must be sandboxed. Pack prompts from servers explain source, size, scope, and whether content persists after disconnect. User-provided pack files stay local unless the user explicitly exports them.

### 7.12 Complete vanilla feature inventory

The selected reference release's complete feature catalog is the authority for the final content inventory. For every released feature, the project must create a traceable entry with one of: **not started**, **in progress**, **implemented**, **verified**, **not applicable**, or **blocked by rights/platform**. “Implemented” requires code and a defined scenario; only “verified” counts toward parity. At minimum, the inventory must cover the following feature families and every member present in the pinned release:

- **World model:** all dimensions, world borders, coordinate limits, chunk lifecycle, terrain layers, cave systems, biome/region transitions, structures, structure placement rules, spawn rules, world seed behavior, time/day cycle, weather, sky, ambient environment, and dimension transitions.
- **Blocks and states:** every block ID and legal state combination; placement, orientation, connection, growth, age, power, waterlogging, support, collision, selection, occlusion, drops, tool requirements, hardness/resistance, light, sound, piston/movement behavior, and neighbor/tick updates.
- **Fluids and simulation:** water/lava or other reference fluids, source/flow/falling states, spread rules, mixing, extinguishing, freezing/evaporation if present, fluid containers, bubble/current effects, and interaction with blocks/entities.
- **Logic and mechanisms:** every redstone-like component, signal strength, power propagation, directional/side rules, delay, pulse, clocks, observers, comparators, switches, doors, pistons, rails, detectors, target blocks, and deterministic update order; also non-electrical mechanics such as item transport, storage, activation, and automation.
- **Items and equipment:** every item, stack limit, durability, use action, cooldown, consumption, projectile behavior, inventory location restrictions, armor/equipment slot, enchantment/effect, repair, and entity/block interaction.
- **Player modes and survival:** movement on every relevant surface and medium, collision, health, damage types, hunger/saturation, exhaustion, armor, status effects, experience, equipment, inventory, death, drops, respawn, spawn points, mode transitions, and game rules.
- **Crafting and progression:** all recipes, recipe unlock/display rules, crafting grids, processing stations, fuel rules, repair/combination costs, trading, advancements/criteria, statistics, milestones, and progression feedback.
- **Entities:** every passive, neutral, hostile, utility, projectile, vehicle, display, and boss entity in the target release; attributes, AI, pathing, senses, target selection, spawn/despawn, breeding, taming, equipment, passenger rules, damage, death, drops, persistence, and player interaction.
- **Combat and encounters:** attack timing, reach, cooldowns, critical/knockback behavior, blocking, ranged weapons, projectiles, damage immunity, effects, armor calculation, environmental hazards, boss phases, encounter triggers, and drops.
- **World interaction:** mining progress and cancellation, placement validity, reach/raycast selection, entity/block interaction precedence, containers, beds/respawn, signs/books/maps, portals, vehicles, item pickup/throw, and command interactions.
- **Client presentation:** all HUD states, camera effects, block/entity animation, particles, sky/fog/weather, lighting, transparency, maps, held items, status icons, contextual text, sounds, subtitles, and screen transitions.
- **Commands and administration:** all commands in declared contexts, permissions, syntax, suggestions, parser errors, selector behavior, feedback, rule changes, operator/admin controls, and dedicated-server configuration where supported.
- **Multiplayer:** join/leave, identity/session requirements, player synchronization, chunk and entity replication, inventory transactions, combat authority, chat, teams/scoreboards, permissions, server resource packs, latency behavior, reconnect/error states, and server-version negotiation, if Java interoperability is declared in scope.
- **Accessibility and options:** every user-visible setting, control mapping, language fallback, narration/subtitles, reduced effects, UI scaling, and behavior when preferences conflict with server policy.

The inventory is release-specific. New reference-release features are added to the matrix before that release is declared supported. Removed/changed features are retained as migration or compatibility cases when old worlds or servers remain supported.

### 7.13 Block, item, and state catalog record

Each catalog record must include, as applicable: stable internal key; reference identifier/state dimensions; provenance and source links; feature status; default and legal states; placement and rotation; collision/selection/occlusion geometry; support and replaceability; hardness and resistance; mining time/tool rules; drops and loot conditions; light emission/opacity; fluid containment; sound/particle behavior; ticking/update triggers; interaction action; persistence encoding; rendering/material inputs; and conformance scenarios.

State combinations must be validated before world mutation. Invalid states loaded from files or networks must produce a controlled diagnostic and preserve recoverability. State transition logic must be independently implemented and ordered consistently; tests must distinguish equal visual appearance from equal behavior.

### 7.14 Entity and item lifecycle

- Entities have stable identity, spawn/despawn reasons, versioned serialized state, attributes, dimensions, collision/selection bounds, tick/update rules, movement, damage, effects, interactions, and ownership/authority.
- Entity creation, pickup, inventory insertion, item use, crafting result, trade, death drop, and save/load are atomic transactions. Retries cannot duplicate rewards or lose items.
- AI behavior is modeled as observable goals and transitions, with explicit target acquisition/loss, navigation failure, attack range, cooldown, and persistence rules.
- Item stacks have explicit limits and merge rules; splitting, moving, equipping, consuming, dropping, and transferring across UI/network boundaries preserve total counts except where the reference behavior consumes or creates items.
- Stable ordering is defined for simultaneous events, including damage, death, drops, pickup, block changes, and save snapshots.

### 7.15 Technical gameplay systems

Redstone/logic, fluids, block entities, scheduled ticks, neighbor notifications, pistons, containers, and automation are treated as first-class simulation systems, not visual approximations. Each has explicit state ownership, update ordering, cancellation rules, serialization, and chunk-boundary behavior. The implementation must cover edge cases such as simultaneous updates, unloaded neighbors, recursive chains, state changes during a tick, and server/client disagreement. Limits may be introduced only where the reference permits them or where the product is explicitly operating in a non-parity mode.

### 7.16 Mod and extension boundary

The 100% parity target applies to the declared unmodified reference release and configurations. Mods, mod loaders, plugins, shaders, and custom server extensions are separate products/features and are excluded unless a later decision defines an API and a named compatibility commitment. User-provided resource/data packs are not code execution; pack parsers must remain sandboxed, validate paths and sizes, and cannot load arbitrary native code.

## 8. Java Edition parity definition and acceptance

### 8.1 Reference baseline

Parity is measured against one exact stable Java Edition release at a time. The first target is the official stable Java Edition 26.3 release, announced 2026-09-15; its initial Windows x86-64 single-player test cell and evidence rules are recorded in `docs/compatibility/java-26.3.md`. Each run must capture the resolved launcher manifest, artifact hashes, Java runtime, platform, and effective configuration from an authorized local installation. The baseline is not “latest” by implication and must be deliberately advanced. Existing movement experiments mentioning 26.3 do not establish complete or even movement conformance.

The target is the complete unmodified **game runtime** for that release: title-to-world flow, every built-in game mode and dimension, all built-in content and mechanics, the user-facing client, and supported vanilla multiplayer behavior. The parity claim must separately enumerate platform services and account-dependent features that cannot be lawfully or technically reproduced; such an exclusion prevents an unqualified “100% Java Edition parity” claim. Any integration that requires an account or online service must use authorized flows and preserve the service's authentication and ownership checks. This project does not distribute a launcher or game files.

For each baseline, the compatibility manifest records: exact release and distribution build; supported OS/GPU/audio combinations; singleplayer/local-server/dedicated-server context; default and non-default game rules; game modes and difficulty; resource configuration; language; input device; privacy/security preconditions; and any external service prerequisite. If results differ by platform, each support cell is separately verified or listed as an exclusion.

### 8.2 Observable parity domains

The parity matrix must include every applicable observable behavior in these domains, including configuration screens and edge cases. Requirements are decomposed to an independently testable behavior or finite catalog entry; broad family-level rows alone cannot pass:

1. Menus, settings, input handling, HUD, localization fallback, and accessibility-relevant behavior.
2. World creation, seeded generation, dimensions, biomes, structures, time, weather, chunk boundaries, save/load, and migration.
3. Block/item/state catalog, models/shapes, placement, mining, drops, collision, fluids, light, redstone/logic systems, and update ordering.
4. Player movement, modes, status, inventory, crafting, damage, equipment, effects, death, respawn, and progression.
5. Every entity type, AI, interaction, combat, spawning, persistence, and audiovisual cues.
6. Rendering and sound behavior, allowing only documented platform/hardware tolerances and lawful asset-source constraints.
7. Multiplayer protocol and gameplay behavior only for separately named server versions and configurations.

Each matrix row carries a unique ID, domain, feature/content identifier, preconditions, action sequence, expected state and visible feedback, persistence/synchronization expectation, target build/platform, evidence provenance, test fixture, tolerance, result, known issue link, and reviewer. Matrix state is tracked per declared support cell. A pass on one block, entity, setting, or OS does not stand in for the rest of its catalog.

The phrase **100% parity** is reserved for a completed, reviewed matrix with no known behavior gaps in the declared scope. It does not mean byte-identical executable, source, or proprietary assets. The product may not describe itself as “100% vanilla” if any required domain, configuration, mode, or supported platform remains untested or excluded.

### 8.3 Evidence and gate

- Build independently authored black-box scenarios from public documentation and lawfully accessible reference behavior. Record provenance for every scenario.
- Compare state transitions and user-visible outputs with exact setup metadata; use deterministic fixtures where possible and explicit numeric tolerances where platform effects vary.
- Cover normal, boundary, error, save/reload, and multiplayer-authority cases. Include regression tests for every discovered discrepancy.
- Keep test data original or synthetic. Do not commit reference-game worlds, proprietary assets, source, credentials, or copied protocol captures that are not approved for distribution.
- Publish a compatibility report listing passed, failed, untested, excluded, and platform-specific cases. “No known failures” is not equivalent to “all cases passed.”
- Require engineering, security, and legal review before making broad interoperability or full-parity claims.

### 8.4 Definition of complete parity

The eventual target is **100% verified coverage of the declared, version-pinned vanilla game-runtime scope**. Completion requires all of the following at once:

- The feature inventory is reconciled against the exact reference release and contains no unexplained missing categories, IDs, states, settings, screens, commands, entities, recipes, rules, or supported configurations.
- Every in-scope matrix row is verified on each supported platform/context, or is explicitly excluded with its effect on the claim. There are no “unknown,” “not started,” “in progress,” “implemented but unverified,” or open discrepancy rows.
- Catalog completeness is checked against an independent authoritative inventory, while behavior is checked through independently authored black-box scenarios. Neither inventory comparison nor a sample of tests alone proves parity.
- Save/reload, upgrade, multiplayer authority, boundary/error handling, and interactions among systems are covered, not only isolated feature demos.
- Known nondeterminism and hardware differences have documented reference behavior and tolerances; tolerances cannot mask changed outcomes, missing content, or gameplay-affecting differences.
- The full report names remaining platform/service limitations. If any required vanilla game behavior or declared platform is omitted, report partial parity and do not use “100%.”

Additional original features may be offered as clearly identified options, but the default reference profile must remain free of behavior-changing additions. A compatibility mode must state what it changes, be selectable before a world/session is entered, and not be represented as the unmodified reference behavior.

### 8.5 Compatibility matrix record format

Each row is independently reviewable and uses this minimum record:

| Field | Required value |
|---|---|
| Requirement ID | Stable domain-prefixed ID, such as `COMP-MOVE-0001` or `COMP-UI-0001` |
| Reference target | Exact game release/build, OS/runtime, mode, rules, server context, and resource configuration |
| Feature identity | Exact screen, setting, item/block/entity/recipe/command/system ID or interaction |
| Preconditions | World/player state, inventory, coordinates, timing, network state, and other required setup |
| Scenario | Ordered player/system actions, including relevant input and timing |
| Expected result | State transition, visible UI, persistence, sound/accessibility cue, and network effect as applicable |
| Evidence | Public source/observation provenance, independently authored fixture, and reviewer |
| Variance | Allowed numeric or platform tolerance with rationale; “none” for exact cases |
| Status | Not started / in progress / implemented / verified / not applicable / blocked by rights or platform |
| Gap | Link to discrepancy, implementation work, known issue, or explicit exclusion |

Use one row for every finite content entry and one or more rows for every behavior. A setting can need multiple rows for menu access, default/effective value, persistence, and effect on gameplay. A feature with several state combinations needs coverage of every legal combination or a documented equivalence partition justified against the reference.

## 9. Quality attributes and release criteria

### Performance

- Establish minimum/recommended hardware and performance budgets through measurement before public release. Report median and tail frame time, simulation tick time, memory, load/save time, and chunk throughput on named hardware.
- Maintain responsive menus and bounded memory/CPU use under maximum supported settings. Slow devices receive safe defaults and graceful quality fallbacks.
- No performance optimization may change deterministic simulation or persistence semantics without a documented compatibility decision.

Performance targets are set after profiling a declared hardware tier. The release requirement is to publish the tier, scene/workload, settings, observed distributions, and thermal/long-session behavior. A headline average FPS is insufficient; the report includes 1% and 0.1% low frame rates or equivalent tail-frame measurements, tick overruns, memory high-water mark, chunk-generation latency, and save/load latency. Parity correctness must be checked with presentation settings both low and high.

### Reliability and security

- No known data-loss path for normal shutdown, recoverable crash, save interruption, malformed resource, or disconnect.
- Validate all untrusted world, pack, network, and server inputs; bound decompression, allocation, parsing, and per-tick work.
- Secure network credentials and secrets; avoid logging them. Authentication is never bypassed.
- Release builds are reproducible to the extent supported by the toolchain, signed/published through a documented process, and accompanied by checksums and license notices.

### Accessibility and usability

- All essential menu actions are keyboard accessible and have visible focus.
- Text remains readable at supported UI scales; color is not the sole carrier of critical state.
- Onboarding, error messages, and settings explain consequences in plain language.

### Maintainability

- Each system has module boundaries, documented contracts, focused tests, and a changelog/action-log entry for material work.
- CI runs formatting, tests, and release compilation. Optional platform-specific checks are identified separately.
- Keep architectural decisions in `DECISIONS.md`; keep this PRD as product intent and implementation status in `README.md`/`ACTION_LOG.md`.

### Release acceptance checklist

A public release candidate is not accepted until each answer is supported by a named artifact or report:

1. Can a clean install launch, create a world, play, save, close, and resume using only bundled original content?
2. Do all supported settings persist, migrate, reset, and recover from malformed values safely?
3. Do all catalog entries and screen families have traceable status and acceptance evidence for the release scope?
4. Are save corruption, interrupted writes, resource-pack failures, device loss, network loss, and incompatible versions handled without silent data loss?
5. Are licensing, asset provenance, privacy, security, account/service integration, and branding reviewed?
6. Are supported platforms, hardware tiers, known limitations, and recovery instructions published?
7. If parity is claimed, does the completed Section 8 matrix show no in-scope gap and has the exact claim received engineering, legal, and security review?

An unanswered item blocks the corresponding release claim. It does not block a clearly labeled prototype or preview release if its limitations are shown before play and no data-loss/security gate is violated.

## 10. Delivery roadmap and exit conditions

Dates are intentionally omitted until scope, capacity, and risk are estimated. Milestones may overlap only when their dependencies are satisfied.

### P0 — Engine foundation (active)

Stable coordinates and block registry, deterministic world/simulation boundaries, safe persistence design, renderer/platform abstraction, logging, CI, and original prototype content. Exit: documented contracts and repeatable automated checks; no claim of finished game play.

### P1 — Playable vertical slice

Main menu, world creation/list, load/save, controllable player, basic terrain, block break/place, inventory/hotbar, pause/options, death/recovery, original assets, and stable Windows release. Exit: a new user can create and resume a small world without developer tools.

### P2 — Survival and creative core

Modes, health/hunger, complete first content slice, crafting/containers, lighting/fluids, entity basics, settings persistence, audio, accessibility foundations, world backup/migration, and performance baselines. Exit: documented end-to-end gameplay loop with data-loss and security review.

### P3 — Content and systems breadth

Complete the pinned release's block/item/entity/world systems, advanced mechanics, progression, dimensions, resource-pack pipeline, UI families, and diagnostics. Exit: domain inventory is mapped and every supported feature has a defined implementation state.

### P4 — Original multiplayer

Engine-owned server, versioned protocol, server-authoritative gameplay, prediction/reconciliation, chat, server management, and security review. Exit: multi-client conformance suite and documented operating model.

### P5 — Versioned Java Edition conformance

Pin a baseline, complete black-box matrix, resolve discrepancies, complete legal/security review, and publish a transparent report. Exit: all requirements in the declared parity scope pass; remaining exclusions prevent a 100% claim.

### P6 — Release hardening

Cross-platform decision and QA, accessibility review, save migration/recovery review, performance and compatibility reports, licensing/brand review, support documentation, installer/update path, and release rollback plan. Exit: signed release candidate meets published support targets.

## 11. Risks, dependencies, and open decisions

- **Parity scope size:** Java Edition changes over time and has a very broad behavior surface. Mitigation: pin exact baselines and maintain a gap matrix; advance versions deliberately.
- **Clean-room and rights constraints:** assets, branding, protocol details, and presentation may have legal limits. Mitigation: original defaults, provenance tracking, constrained local imports, and legal review.
- **Unspecified current requirements:** target operating systems, minimum hardware, release cadence, and desired server interoperability are undecided. Resolve before release commitments.
- **Rendering and dependency availability:** graphics backend and supported hardware need ongoing verification and fallback planning.
- **Multiplayer security:** public server compatibility materially expands attack surface. Complete security review before exposing a server or client to untrusted networks.
- **Asset parity tension:** exact audiovisual identity may depend on protected assets. Product acceptance must distinguish behavioral parity from asset redistribution; never bundle assets without documented rights.
- **Version choice:** choose and document the first conformance baseline before the P5 work starts. No claim follows from the movement experiment's target version.

## 12. Requirements traceability and change control

Every implementation issue should reference one or more requirement IDs or clearly identify a prototype-only task. Suggested IDs: `UX-*` for interface, `SET-*` for settings, `SYS-*` for systems, `NET-*` for networking, `COMP-*` for compatibility, `QUAL-*` for quality, and `REL-*` for release. Add IDs as requirements are split into trackable work; do not use this document as evidence that work is implemented.

Changes to product direction, parity definition, legal boundary, or release gates are recorded in `DECISIONS.md`. Completed work and verification belong in `ACTION_LOG.md`; current user-facing scope belongs in `README.md`. This document is reviewed whenever a milestone closes or the chosen compatibility baseline changes.

## Appendix A. Implementation record templates

### A.1 Player-facing requirement

```text
ID:
User / context:
Requirement:
Preconditions:
Expected behavior:
Failure and recovery behavior:
Persistence / authority:
Accessibility and localization:
Acceptance scenarios:
Dependencies / exclusions:
Implementation status:
Evidence:
```

### A.2 Setting record

```text
ID and menu path:
User need / help text:
Control type and legal values:
Reference default and product default:
Persistence scope:
Effective behavior and apply timing:
Platform/device availability:
Reset and migration behavior:
Accessibility/localization needs:
Acceptance scenarios:
```

### A.3 Content catalog record

```text
Stable internal ID / reference identity:
Content family and legal states:
Behavior and interactions:
Visual/audio resources and provenance:
Persistence/network authority:
Known combinations and edge cases:
Acceptance scenarios:
Compatibility status and gaps:
```
