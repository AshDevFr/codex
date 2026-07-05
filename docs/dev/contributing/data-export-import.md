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
  enforcement is disabled for the duration of the load (SQLite
  `defer_foreign_keys`; PostgreSQL `session_replication_role = replica`, which
  needs owner/superuser) and the whole load runs in one destination
  transaction. On SQLite the commit re-validates FKs; on PostgreSQL integrity
  rests on source consistency plus a post-load row-count check.
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
