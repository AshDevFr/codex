# Export, Import & Copy

Codex can move its entire dataset between databases and snapshot it to a
portable archive. Three commands cover it:

- **`export`** — write the database (and the on-disk artifacts it references) to
  a single `.tar.gz`.
- **`import`** — load an archive into an instance.
- **`copy`** — stream one database's rows directly into another (the "sync"
  between two instances), no file in between.

All three are driven by the database's own entity definitions, so
engine-specific representations (UUIDs, JSON, booleans, timestamps) are
translated correctly between SQLite and PostgreSQL — something a raw SQL dump or
generic converter cannot guarantee.

:::info Why not just copy the SQLite file or use `pg_dump`?
The SQLite file only works on SQLite, and `pg_dump` only reads PostgreSQL —
neither crosses engines. SQLite stores UUIDs and JSON differently from
PostgreSQL's native `uuid`/`jsonb`, so a byte-level copy would corrupt data. The
`export`/`import`/`copy` commands translate these correctly.
:::

:::note Not the same as "Data Exports"
This is database-level backup/transfer. The user-facing [Data Exports](../exports)
feature (exporting a series to JSON/CSV) is unrelated.
:::

## `export`

Writes the database and its on-disk artifacts to a `.tar.gz`.

```bash
codex export --config config/codex.yaml --output codex-backup.tar.gz
```

By default the archive bundles the database plus **thumbnails**, **uploaded
covers**, and **plugin data**. Flags:

| Flag | Effect |
|------|--------|
| `--include-cache` | Also bundle the rendered PDF page cache (reproducible, can be large) |
| `--db-only` | Bundle the database only; no on-disk artifacts |
| `--no-thumbnails` | Skip generated thumbnails |
| `--no-uploads` | Skip uploaded/extracted covers |
| `--no-plugins` | Skip plugin data |
| `--progress` | Log per-table progress while exporting |

The archive contains a `manifest.json` (format and schema version, per-table row
counts, bundled artifact groups), one `db/<table>.ndjson` per table, and the
bundled artifact directories.

## `import`

Loads an archive into the current instance, running migrations on the target
first, then validating and loading.

```bash
codex import --config config/codex.yaml --input codex-backup.tar.gz
```

Import **refuses to run** if:

- the archive's schema version does not match this instance's schema (import
  with a Codex build whose schema matches the archive), or
- the target database already contains user data (libraries, series, books, or
  users) — unless you pass `--replace`.

```bash
# Overwrite an existing instance with the archive's contents:
codex import --input codex-backup.tar.gz --replace
```

On import, file paths stored in the database are **re-rooted** to this
instance's configured directories, so an archive from an instance with different
`files.*_dir` paths still resolves its images.

## `copy`

Streams database rows directly from one database to another, without an
intermediate file — useful for pushing/pulling between two live instances.

```bash
# Run on the destination: pull the old SQLite database into the local (Postgres) config
codex copy --from "sqlite:///var/lib/codex/codex.db"

# Run on the source: push into a new instance
codex copy --to "postgres://codex:secret@db:5432/codex"

# Both sides explicit
codex copy --from "sqlite:///old/codex.db" --to "postgres://codex@db:5432/codex"
```

Each side resolves in this order: an explicit `--from` / `--to` URL →
`CODEX_SOURCE_DATABASE_URL` / `CODEX_TARGET_DATABASE_URL` → a `--from-config` /
`--to-config` file → the local instance config (`--config`) when that side is
omitted. At least one side must be non-local.

:::caution `copy` moves rows only
`copy` transfers database rows, not files. On-disk artifacts (thumbnails,
covers, plugin data) are **not** moved — sync them separately (e.g. `rsync` or a
volume copy). For a self-contained move including files, use `export` + `import`.
:::

To avoid leaking a password via the process list, prefer the env vars or
`--from-config` / `--to-config` over a `postgres://user:pass@…` URL on the
command line.

## Progress & verification

`import` and `copy` accept:

| Flag | Effect |
|------|--------|
| `--progress` | Log per-table progress (and a periodic row count for large tables) — reads the same in a terminal or in captured logs |
| `--no-verify` | Skip the row-count verification |
| `--full-verification` | Additionally compare every record's content (see below) |

**Row-count verification runs by default.** After the load, the source and
target are re-counted per table and compared; a mismatch **fails the command**.
This is your confirmation that no rows were dropped.

**`--full-verification`** is an opt-in deeper check: it compares every record's
*canonical* content on both sides and prints a **report** (it does not fail the
command). "Canonical" means representation differences that don't change meaning
are ignored — `1.0` equals `1`, JSON object key order is normalized (PostgreSQL
`jsonb` reorders keys), and timestamps are compared at microsecond precision
(PostgreSQL truncates). It streams rows, so it stays memory-cheap even on large
tables, but it does re-read everything, so it adds time.

## Setting up the target

You never create tables — `import` and `copy` run the migrations themselves.
What you need to prepare depends on the engine:

- **SQLite target — nothing to create.** Like `serve`, `import` writes a default
  config if none exists and creates the database file (and its parent
  directories) automatically. Just point `database.sqlite.path` (or
  `CODEX_DATABASE_SQLITE_PATH`) at the destination and run it.
- **PostgreSQL target — create the empty database and role first.** PostgreSQL
  won't create a database from a connection string, so provision it once (your
  Kubernetes chart / operator / an init job typically does this):

  ```sql
  CREATE DATABASE codex;
  CREATE USER codex WITH PASSWORD '...';
  ALTER DATABASE codex OWNER TO codex;   -- the role must own the database (see the note below)
  ```

  Then point the config/env at it and import — the schema and data are created
  for you.

## Backup & restore

The same tooling doubles as backup:

```bash
# Dated backup
codex export --output "backups/codex-$(date +%F).tar.gz"

# Restore into a fresh instance
codex import --input backups/codex-2026-07-04.tar.gz

# Restore over an existing instance (destructive)
codex import --input backups/codex-2026-07-04.tar.gz --replace
```

Because export/import are engine-agnostic, a backup taken from PostgreSQL can be
restored into SQLite (e.g. to pull production down to a local file for
debugging) and vice versa.

## What is and isn't included

**Included:** every database table, and (by default) generated thumbnails,
uploaded/extracted covers, and plugin data.

**Not included:** your **library files** themselves (the CBZ/EPUB/PDF on disk) —
those live on a volume the new instance is expected to mount. The reproducible
PDF page cache is excluded unless you pass `--include-cache`, and user-generated
export files are not bundled.

:::danger Carry over the encryption key
Encrypted values (such as plugin credentials) are copied as **ciphertext** and
are never decrypted. A destination instance must be configured with the **same
encryption key** as the source, or those values cannot be decrypted afterwards.
:::

:::note PostgreSQL privileges
`import` and `copy` suppress foreign-key enforcement during the bulk load by
dropping and recreating the FK constraints, which requires only that the target
role **owns the tables** — no superuser needed. This works on managed
PostgreSQL, where you typically get a database owner but not a superuser. (The
role owns the tables because it ran the migrations that created them.)
:::
