# Data Export & Import

The `export`, `import`, and `copy` commands are thin wrappers over the
`codex-migrate` crate, which moves data between any two SeaORM-supported
backends. This page covers how it works; for user-facing usage see the
[Export, Import & Copy](/docs/backup-migration/export-import-copy) guide.

This is distinct from [Database Migration Strategy](./migrations), which is
about *schema* migrations (SeaORM `Migrator`). `codex-migrate` moves *data* on
top of whatever schema the migrations produce.

## Why entity-driven, not raw SQL

Every table is transferred by streaming its rows as typed SeaORM `Model` values
and re-inserting them via `ActiveModel`. Because SeaORM performs the
engine-specific mapping when it materializes a `Model`, representation
differences are handled by construction:

- UUID: 16-byte blob (SQLite) ↔ native `uuid` (PostgreSQL)
- JSON: text (SQLite) ↔ `jsonb` (PostgreSQL)
- booleans: `0`/`1` ↔ native `bool`
- timestamps and the rest

A raw SQL dump or a generic converter would have to reimplement all of this and
would silently corrupt UUIDs and JSON across engines. The write side always
adapts to the destination, so any source→destination pairing yields a correct
result.

## The engine

- **Entity registry** — an x-macro lists every entity exactly once; all
  collective operations (count / copy / dump / load / truncate) are generated
  from that single list. A drift-guard test fails if a migration adds a table
  that isn't registered.
- **Foreign keys** — a 1:1 load inserts tables in arbitrary order, so FK
  enforcement is suppressed for the load. SQLite uses `defer_foreign_keys` (the
  commit re-validates). PostgreSQL **drops the FK constraints and recreates
  them** after the load — recreating revalidates the rows, and it needs only
  table ownership (not a superuser, unlike `session_replication_role`), so it
  works on managed PostgreSQL. The whole load runs in one destination
  transaction, so a failure rolls back cleanly.
- **Truncate before load** — migrations seed rows (e.g. `settings`), so a
  freshly-migrated target is *not* empty. A faithful mirror therefore truncates
  every table before loading. The CLI's fresh-target guard (refuse a target
  holding user data unless `--replace`) is separate from this always-on
  truncate.
- **Legacy UUID storage** — SeaORM reads a SQLite `Uuid` strictly as a 16-byte
  blob, but databases written by older toolchains may store some UUIDs as
  36-char hyphenated text. When the source is SQLite, each table is read through
  a query that coerces UUID columns back to blobs
  (`text → unhex(replace(col,'-',''))`, blobs pass through), so mixed storage
  decodes correctly and is canonicalized on the way out.
- **Batch sizing** — a multi-row insert binds `rows × columns` parameters, and
  PostgreSQL caps a statement at 65535 (SQLite at 32766). The batch size is
  capped per table so a wide table (e.g. `book_metadata`, ~66 columns) can't
  overflow the destination's limit.

## Verification

Two levels, both surfaced by the CLI:

- **Row-count parity** (`registry::count_all` + `verify::compare`) runs by
  default after `import`/`copy` and fails the command on any mismatch.
- **Full verification** (`full_verify`) is opt-in and reports (does not fail):
  each table is reduced to an order-independent digest of every row's
  *canonical* value — integer-valued floats normalize (`1.0` == `1`), JSON keys
  are sorted (jsonb reorders), and timestamps truncate to microseconds. Digests
  are computed identically from a connection or the archive's NDJSON, so it
  streams and stays O(1) memory. Genuine content differences (or a tampered
  target) change the digest; representation differences do not.

## Archive format

`export`/`import` use a gzip tar:

```text
manifest.json          format + schema version, per-table row counts, artifacts
db/<table>.ndjson       one NDJSON file per table
thumbnails/  uploads/  plugins/  [cache/]   bundled on-disk artifacts
```

`copy` skips the archive and streams rows directly between the two connections.

On import, file-path columns (`books.thumbnail_path`, `book_covers.path`,
`series_covers.path`) are **re-rooted** from the source base dirs recorded in the
manifest to the target instance's configured `files.*_dir`, so images resolve
even when the two instances use different directories.

## Testing

Coverage lives in `crates/codex-migrate` (engine + archive + reroot round-trips,
registry drift, legacy text-UUID reads) and `tests/migrate` (a SQLite↔PostgreSQL
round-trip and an export/import matrix across every engine pair). PostgreSQL
tests are `#[ignore]` and skip when no test database is reachable.
