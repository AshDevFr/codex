---
id: 0001-workspace-split
slug: 0001-workspace-split
---

# ADR 0001: Workspace Split

- **Status:** Accepted
- **Date:** 2026-05-23
- **Authors:** Sylvain Cau

## Context

Codex had grown to roughly 200k lines of Rust in a single crate. Editing one subsystem (for example, a file in `src/parsers/`) forced the entire library to re-typecheck and the binary to relink, even when no API handler was affected.

A build-performance session on 2026-05-22 picked off the easy wins:

- Excluded `target/` from Spotlight indexing (large, immediate win on macOS).
- Adopted `sccache` as an opt-in `RUSTC_WRAPPER` (~10% on cold builds).
- Consolidated the integration test layout from 13 binaries down to 1.

With those in place, warm rebuilds still cost about **30 seconds** after a single-line edit, and the remaining structural lever was the single-crate cargo cache scope. Crate-level caching only helps when consumers of a changed crate are themselves in separate crates, so a workspace split was the next reasonable step.

### Dependency Graph at the Time

The audit on 2026-05-22 counted directional imports between the 12 top-level `src/` subdirectories. Rows import from columns, measured by `use crate::<col>` references:

```
FROM \ TO     api  commands  config  db   events  parsers  scanner  scheduler  search  services  tasks  utils
api            -      0        1     52     9       2        3         0        0        16       13     11
commands       2      -        3      4     1       1        1         0        0        2        1      1
config         0      0        -      0     0       0        0         0        0        0        0      0
db             3      0        2      -     4       0        0         0        0        7        1      7
events         0      0        0      0     -       0        0         0        0        0        0      0
parsers        0      0        0      0     0       -        0         0        0        0        0      7
scanner        0      0        0      3     2       4        -         0        0        1        2      3
scheduler      0      0        0      2     0       0        1         -        0        2        2      2
search         0      0        0      2     1       0        0         0        -        0        0      1
services       3      0        6     28     9       0        0         1        0        -        2      3
tasks          0      0        5     32    29       0        2         0        0        26        -     0
utils          1      0        0      0     0       0        0         0        0        0        0      -
```

Key observations from the matrix:

- **Pure leaves (zero outbound edges):** `config`, `events`. Trivially extractable.
- **Near-leaf:** `utils → api` (1 file).
- **Six small cycles, all ≤7 files:** `utils ↔ api`, `db ↔ api` (3 files), `services ↔ api` (3 files), `db ↔ services` (7 files), `services ↔ tasks` (2 files), `services ↔ scheduler` (1 file).
- **Top of the stack:** `commands` (binary orchestrator).

The cycles were drift, not structural fact: in every case the wrong-direction import was a shared type that had landed in the wrong layer. Eliminating them was independently valuable even if the workspace split never happened.

## Decision

Split the single `codex` library crate into a Cargo workspace of layered sibling crates, rolled out incrementally with explicit decision gates after the first measurement.

### Principles

- **Incremental, not big-bang.** Each phase is a separate commit set. Any phase can be the last one; the work done so far is never wasted.
- **Decision gates at Phase 2 (workspace mechanics work) and Phase 3 (measured win materializes).** Both have pass/fail criteria stated up front.
- **Phase 1 cleanup happens regardless.** Even if no further phase shipped, the drift cleanup would have been worth it.
- **Workspace-internal, not published.** All sibling crates use `version = "0.0.0"` and `publish = false`. Codex is not a library distributed via crates.io.
- **`migration/` stays as-is.** It was already a separate crate and is self-contained; `codex-db` simply depends on it.
- **DTOs live in `codex-models`** to break the `api ↔ db` cycles cleanly.
- **Tests stay in `tests/it.rs`.** Each new crate may grow its own `#[cfg(test)]` blocks, but the integration test binary stays consolidated.

### Final Layering

```
┌────────────────────────────────────────────────────────────┐
│ codex (bin)             main.rs + commands/                │
│ codex-cli-common        shared subcommand helpers          │
└────────────────────────────────────────────────────────────┘
                              │
┌────────────────────────────────────────────────────────────┐
│ codex-api               axum, OPDS, OPDS2, Komga, KOReader │
│                         observability, embedded frontend   │
└────────────────────────────────────────────────────────────┘
                              │
┌────────────────────────────────────────────────────────────┐
│ codex-scheduler         cron / interval scheduler          │
└────────────────────────────────────────────────────────────┘
                              │
┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐
│ codex-tasks      │  │ codex-scanner    │  │ codex-search │
└──────────────────┘  └──────────────────┘  └──────────────┘
                              │
┌────────────────────────────────────────────────────────────┐
│ codex-services          business logic, plugins, metadata  │
└────────────────────────────────────────────────────────────┘
                              │
┌────────────────────────────────────────────────────────────┐
│ codex-db                SeaORM entities + repositories     │
└────────────────────────────────────────────────────────────┘
                              │
┌────────────────────────────────────────────────────────────┐
│ codex-parsers           CBZ / CBR / EPUB / PDF             │
└────────────────────────────────────────────────────────────┘
                              │
┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐
│ codex-utils      │  │ codex-events     │  │ codex-config │
└──────────────────┘  └──────────────────┘  └──────────────┘
                              │
┌────────────────────────────────────────────────────────────┐
│ codex-models            shared DTOs + cross-layer types    │
└────────────────────────────────────────────────────────────┘
```

`migration/` is consumed by `codex-db` and depends on no other Codex crate.

### Rollout

| Phase | Outcome |
| ----- | ------- |
| 1 | Drift cleanup. Six cycles removed; single-crate build stayed green. |
| 2 | Workspace bootstrap: `codex-config` and `codex-events` extracted as leaf crates. |
| 3 | `codex-models`, `codex-utils`, `codex-parsers` extracted. **MAYBE gate (~7% warm-rebuild improvement)** — proceeded based on the structural argument that the leaves were too small to move the needle. |
| 4 | `codex-db` extracted. **GO gate (~26% cumulative warm-rebuild improvement vs Phase 2 baseline).** |
| 5 | Business layers extracted: `codex-services`, `codex-search`, `codex-scanner`, `codex-tasks`, `codex-scheduler`. **GO gate (~50% cumulative).** |
| 6 | `codex-api` extracted; root crate slimmed to `main.rs` + `commands/`. **GO gate (~62% cumulative warm-rebuild improvement).** |
| 7 | `CodexError` cleanup: moved to `codex-parsers::ParserError`, dropping `image`, `quick-xml`, `zip`, and `thiserror` from `codex-utils`'s dep list. |
| 8 | `codex-cli-common` extracted from `src/commands/common.rs` for architectural consistency. |

## Consequences

### Build Times

End-to-end measurements, using `cargo clean && cargo test --no-run` for cold and `touch <file> && cargo test --no-run` for warm. The warm-edit target is `src/api/routes/v1/handlers/auth.rs` (a representative API handler).

| Metric | Pre-split (Phase 2 baseline) | Post-split (Phase 6) | Δ |
| --- | --- | --- | --- |
| Cold (`cargo clean` + `cargo test --no-run`) | 191.8s | 133.3s | **−30.5%** |
| Warm (one-line edit in an API handler) | ~29.7s | ~11.3s | **−62.0%** |
| Warm (one-line edit in `src/commands/`) | n/a | ~2.8s (root binary only) | new fast path |

The dominant gain came from extracting `codex-db` (Phase 4, ~21% incremental) and the business layers (Phase 5, ~32% incremental). Phase 6 (`codex-api`) added a final ~24% on top. Leaf-only extractions (Phases 2–3) moved the needle by less than 10% combined, which matched the prediction that the leaves were too small to dominate the warm-rebuild cost.

The sea-orm and utoipa macro re-derivation cost (flagged as the dominant Phase 3 risk) did not materialize. Each crate pays the macro cost only when it is itself recompiled; the cost no longer cascades into the consumers.

### Crate Isolation (the structural payoff)

- Editing `crates/codex-api/src/routes/v1/handlers/auth.rs` recompiles only `codex-api` and the root binary. None of the other 11 workspace crates rebuild.
- Editing `src/commands/scan.rs` recompiles only the root binary in under three seconds. Even `codex-api` stays cached.
- Editing `crates/codex-scheduler/src/lib.rs` recompiles only `codex-scheduler` and the root binary; `codex-services`, `codex-scanner`, and `codex-tasks` (all dependencies of scheduler) stay cached.

This is the property `cargo test -p <crate>` exploits: TDD cycles for a specific subsystem can now skip the rest of the workspace entirely.

### Costs Accepted

- **Cold-build metadata overhead.** Each sibling crate adds dep-graph metadata. The +1.6% cold delta after Phase 2 was the most visible point; cumulative cold builds are still faster than pre-split because crate-level sccache hits offset the metadata cost.
- **rust-analyzer cold-index time.** Slightly longer the first time a checkout is opened. Acceptable.
- **Trait abstractions for would-be cycles.** `services → scheduler` was broken by introducing `SharedSchedulerReconciler` (a boxed-future trait); the scheduler crate provides the concrete impl, `commands/serve.rs` wires it up. The indirection adds one dyn dispatch per scheduler reconciliation, which is not a hot path.
- **`pub` audit churn.** Two `EpubParser` helpers had to be promoted from `pub(crate)` to `pub` to keep working across the crate boundary (Phase 3). All other phases hit zero visibility promotions.
- **Build-time version propagation.** `env!("CARGO_PKG_VERSION")` resolves to a sub-crate's `0.0.0` when called from inside `codex-api`. Fixed in two places: the `info::get_app_info` handler reads name/version from `AppState` (filled by the binary at startup), and the `utoipa::OpenApi` derive picks up `CODEX_BIN_VERSION` from a tiny `crates/codex-api/build.rs` that reads the root `Cargo.toml`.

### Tooling Impact

- **`cargo-dist`:** unchanged. `cargo dist plan` continues to emit only the `codex` binary across the same five targets after every phase.
- **`Makefile`:** `make test-fast` and friends now pass `--workspace` to `cargo nextest` so leaf-crate tests are not silently skipped. Discovered when Phase 4 surfaced ~540 missing `codex-db` tests in the nextest report.
- **OpenAPI generation:** `make openapi` works unchanged; the spec correctly reports the binary's version after the `build.rs` trick above.
- **CI:** no `.github/` workflow changes were required; CI already builds via `cargo build`/`cargo test` at the workspace root.

## Alternatives Considered

### Stay single-crate, lean harder on `sccache` and incremental compilation

This is what the 2026-05-22 perf session did. It captured the easy wins (Spotlight exclusion, `sccache`, test consolidation) and brought warm rebuilds from ~35s to ~30s. After those, there was no further single-crate lever: the warm rebuild was dominated by `rustc` re-typechecking the whole library before linking.

The 62% warm-rebuild improvement from the workspace split is roughly 6x what sccache alone delivered on this codebase. Staying single-crate was a real option, but the headroom was effectively zero.

### One giant `codex-core` crate with internal `mod`s

This would have been the smallest delta from the single-crate layout: keep one crate, but reorganize modules. It would not have helped build times at all, since cargo caches at crate granularity, not module granularity. Rejected on the grounds that the cost of the rename was non-trivial and the benefit was zero.

### Layered "internal API" crates (e.g., `codex-api` + `codex-api-impl`)

Sometimes used in larger Rust workspaces to give one crate's type-checking pressure a fast path. Rejected as premature: the simpler one-crate-per-subsystem layering already produced the warm-rebuild target with far less indirection. Worth reconsidering only if a specific crate later becomes a build-time bottleneck.

### Publish sibling crates to crates.io

A common reason to split a workspace. Not relevant here: Codex is a deployed binary, not a library, and there is no third-party consumer for individual subsystem crates. Keeping `publish = false` on every sibling avoids semver maintenance overhead.

## Follow-Ups

- A `codex-sdk` crate that re-exports `codex_models::plugin::*` plus minimal RPC framing helpers, intended for Rust plugin authors. Scope-gated; not yet started. See the implementation plan's Phase 10.
- The `commands/` orchestrators (`migrate.rs`, `serve.rs`, `worker.rs`, ...) stay in the binary crate. They are binary-glue and would not benefit from being moved to a sibling, but they remain a candidate for further structural splitting if the binary ever grows enough to warrant it.

## References

- Implementation plan: `tmp/implementation/planned/split-workspace.md` (local working doc, includes per-phase progress notes and measurement runs).
- Development build-time guide: [Development → Speeding Up Builds](../contributing/development.md#speeding-up-builds).
- Architecture overview: [Architecture → Workspace Architecture](../contributing/architecture.md#workspace-architecture).
