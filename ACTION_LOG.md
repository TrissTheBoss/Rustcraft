# Action log and operating procedure

Each entry records date, action, result, and next step. Changes should be made in focused commits, tested locally, documented here, and tagged when a usable build exists. Keep generated binaries, Minecraft data, secrets, and personal configs out of Git.

## 2026-11-05

- Inspected Horizon's public repository and MIT license; result: license permits reuse with notices, but ownership/authentication constraints remain. Next: use only documented behavior or separately verified compatible code.
- Checked Rustcraft's GitHub URL; result: no visible refs were returned. Next: initialize the local repository and publish when GitHub credentials permit.
- Created the v0.1.0 clean-room baseline; result: minimal immediate-start CLI with explicit configuration. Next: run formatting, tests, and smoke launch.
- Attempted local verification; result: blocked because `cargo` is not installed or on PATH. Next: install Rust/Cargo or run CI before publishing a binary release.
- Installed Rust stable 1.99.1 through rustup. `cargo fmt` passes; compilation then found the Windows MSVC linker was missing.
- Started Visual Studio 2022 Build Tools with the C++ workload through winget. The installer was still active during this session, so `cargo test` remains pending until `link.exe` is available.
- Verified the installation completed: MSVC `link.exe` is available from Visual Studio 2022 Build Tools. `cargo fmt --check`, `cargo test`, and `cargo run -- --help` all pass. Tagged this verified state as `v0.1.1`.
- Ran the release binary with `rustcraft.conf.example`; it launched Java 21 successfully. Fixed argument quote handling discovered during that run, then reran `cargo fmt --check`, `cargo test`, and the release smoke test successfully. Tagged `v0.1.2`.
- Located the Prism instance `Rustcraft testing` in the `Rustcraft` group. Added quote-aware argument parsing, then launched Prism through Rustcraft with `--launch "Rustcraft testing"`; Prism accepted the exact instance ID and prepared Minecraft 26.3. No Java process remained at the follow-up check, indicating an instance-side startup/authentication exit rather than a Rustcraft handoff failure. Tagged `v0.1.3`.
- Added the loopback trigger protocol and a Fabric client-mod source scaffold for Minecraft 26.3 / Fabric Loader 0.20.1. Rustcraft now supports `--listen`; the mod sends a tokenized trigger from key `J`. Rust tests pass. Fabric API dependency/build packaging remains the next step because the converted instance does not yet expose a selected Fabric API version.
- Selected official Fabric API `0.162.0+26.3` and added a clean Gradle/Loom project configuration. Gradle 9.8.2 is installed. The current 26.3 non-obfuscated dependency resolution still needs the official template coordinates; no unverified jar was installed.
- Finished the Fabric build using the official 26.3 source-set layout and modern names (`KeyMapping`, `InputConstants.Type.KEYBOARD`). Gradle 9.8.2 build passes. Installed the untracked `rustcraft-trigger-0.1.0.jar` into the local `Rustcraft testing` instance. Started Rustcraft listener and verified a loopback `RUSTCRAFT local-only` request returned `OK` and invoked Prism.
- Began performance optimization: moved trigger socket I/O off Minecraft's client tick onto a Java virtual thread, added an in-flight guard to prevent duplicate launches, and added a 250 ms connection timeout. Rebuilt successfully and refreshed the local instance jar.
- Added Rust release optimization settings (thin LTO, one codegen unit, stripping, abort-on-panic), enabled Gradle build/configuration caching, and documented the no-gameplay-change performance boundary. Next: user hardware testing of frame time and startup latency is required before changing client rendering or memory settings.
- Added Rustcraft's built-in logger: millisecond UTC epoch timestamps, severity, session/process ID, event names, stderr mirroring, `RUSTCRAFT_LOG` override, 5 MiB rotation to `.log.1`, and redacted trigger audit events that never record tokens or game arguments. `cargo fmt`, tests, release build, and logger smoke test pass. Tagged `v0.2.5`.
- Reviewed the launched Fabric instance: `rustcraft_trigger 0.1.0` loaded successfully with no errors. No Rustcraft runtime log existed because the listener was not running. Added mod-side structured logs for initialization, queued triggers, in-flight suppression, responses, and unavailable-listener warnings; rebuilt and refreshed the local jar. User trigger testing is now required.
- Reproduced the reported launch issue: a Windows argument-list invocation truncated the config path at `C:\Users\Triss\Desktop\Rust`, causing `config_read_failed`. Verified environment-based startup, listener bind, and an authorized trigger returning `OK`. Added `start-rustcraft-listener.cmd` for quoting-safe Windows startup.
- Reviewed the next in-game J press: the Fabric initialization log was present, but no `Trigger queued` event appeared, so the key event was not reaching the queued-input path. Added a direct key-state edge detector while retaining queue draining, rebuilt the Fabric mod, and refreshed the local Prism instance jar. Next: restart the Minecraft instance and press J once for verification.

## Rough timeline

- Baseline: complete, about 1 hour.
- v0.2: 1–2 sessions for robust argument parsing, platform command quoting, process logging, and integration tests.
- v0.3: 2–4 sessions for optional discovery of user-owned installations, only where documented and opt-in.
- 1.0: after cross-platform testing, security review, reproducible release builds, and user documentation.
