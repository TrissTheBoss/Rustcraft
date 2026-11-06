# Rustcraft

Rustcraft is a small Rust utility that starts a user-configured Minecraft Java command immediately. It is intentionally not a game launcher, account manager, downloader, entitlement bypass, or distribution of Minecraft files.

## Quick start

1. Install Rust.
2. Copy `rustcraft.conf.example` to `rustcraft.conf`.
3. Set `executable=` to the command or executable you already use, and `args=` to its arguments.
4. Run `cargo run --release`.

Use `RUSTCRAFT_CONFIG` or `--config path/to/file` for a configuration outside the repository. The example uses `java -version` as a harmless smoke test; replace it with your own, already-authorized game command.

## Fabric trigger mode

The `fabric-trigger-mod` source scaffold lets a client-only Fabric mod trigger Rustcraft over loopback. Start Rustcraft with `--listen --config rustcraft.conf`, or run `start-rustcraft-listener.cmd` on Windows. Install the built mod in the Fabric instance, then open Minecraft Controls and assign `Rustcraft: Launch` to `J` (the default Minecraft binding may be `Home`) before pressing it in-game. The listener binds only to `127.0.0.1:39281` and requires a local token. It never handles Minecraft credentials.

## In-game performance profile

The Fabric mod includes an opt-in client-only profile. Set `RUSTCRAFT_PERFORMANCE=balanced` before starting Prism/Minecraft to disable entity shadows and use minimal particles after a world loads. This changes visual settings only; it does not change gameplay, server simulation, networking, or world data. Remove the variable to preserve the instance's normal settings.

## Scope and provenance

This project is a clean-room implementation. Horizon was inspected only for publicly documented behavior, repository structure, and license obligations. No Horizon source is included in this baseline. Horizon is MIT-licensed; if compatible code is later reused, its copyright and license notice will be retained in `THIRD_PARTY_NOTICES.md` and the exact source will be recorded in the decision log.

Rustcraft does not bypass Microsoft/Mojang authentication, ownership checks, paid access, or server rules. Users remain responsible for complying with Minecraft's terms and for supplying their own installation and command.

## Development

```text
cargo fmt --check
cargo test
cargo run -- --help
```

Releases use `vMAJOR.MINOR.PATCH` tags. Small, focused commits are preferred; generated binaries, game data, credentials, and local configs are ignored.
