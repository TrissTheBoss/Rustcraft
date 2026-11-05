# Decision log

## 2026-11-05 — Baseline scope

The request was narrowed to a legal, auditable instant-start utility. Paywall removal, cracked/offline entitlement bypass, credential theft, and copying proprietary code are out of scope. The first implementation accepts an explicit executable and arguments so it can launch an installation the user already owns and configures.

## 2026-11-05 — Horizon review

The public Horizon repository states that it is MIT licensed, unofficial, contains no Minecraft assets/data, and requires a Microsoft account that owns Java Edition. Those constraints are preserved. No Horizon source was copied into v0.1.0.

## 2026-11-05 — Performance

The starter uses Rust's standard process API, has no background service, no network requests, no polling loop, and no runtime dependencies. Future work should measure startup time before adding UI, update checks, or telemetry.
