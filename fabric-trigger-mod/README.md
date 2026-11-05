# Rustcraft Fabric trigger mod

This client-only Fabric mod is the in-game side of Rustcraft's local trigger protocol. It is designed for Minecraft 26.3 / Fabric Loader 0.20.1 and requires Fabric API. The default keybind is `J`; pressing it sends `RUSTCRAFT local-only` to `127.0.0.1:39281`.

The Rustcraft process must already be running in listener mode:

```text
rustcraft.exe --listen --config rustcraft.conf
```

The mod uses loopback only and contains no account, network-service, or file-discovery logic. Change the token in both the Rustcraft environment (`RUSTCRAFT_TRIGGER_TOKEN`) and the mod configuration before using a non-default token. Do not expose the listener beyond localhost.

Build from this folder with `gradle build`. The pinned target is Minecraft 26.3, Fabric Loader 0.20.1, Fabric API 0.162.0+26.3, and Loom 1.19-SNAPSHOT. Copy `build/libs/rustcraft-trigger-0.1.0.jar` to the instance's `minecraft/mods` directory; do not commit the resulting jar or local instance files.
