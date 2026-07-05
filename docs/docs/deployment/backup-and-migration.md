---
sidebar_position: 8
---

# Backup & Migration

Codex can export its entire dataset to a portable archive and load it back into
any supported database engine. This powers three workflows:

- **Backup / restore** — snapshot the database (and the on-disk artifacts it
  references) to a single `.tar.gz`, and restore it later.
- **SQLite → PostgreSQL migration** — move an existing single-node instance to a
  distributed, PostgreSQL-backed deployment with a faithful 1:1 copy of all
  data (metadata, custom data, ratings, reading progress, uploaded covers,
  plugin state).
- **Direct instance-to-instance copy** — stream one database's rows straight
  into another.

All three are driven by the database's own entity definitions, so
engine-specific representations (UUIDs, JSON, booleans, timestamps) are
translated correctly between SQLite and PostgreSQL — something a raw SQL dump or
generic converter cannot guarantee.

:::info Why not just copy the SQLite file or use `pg_dump`?
The SQLite file only works on SQLite. `pg_dump` only reads PostgreSQL. Neither
crosses engines: SQLite stores UUIDs as 16-byte blobs and JSON as text, while
PostgreSQL uses native `uuid` and `jsonb`. The `export`/`import`/`copy` commands
translate these correctly.
:::

## Commands

### `export`

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

The archive contains a `manifest.json` (format and schema version, per-table row
counts, bundled artifact groups), one `db/<table>.ndjson` per table, and the
bundled artifact directories.

### `import`

Loads an archive into the current instance. Runs migrations on the target first,
then validates and loads.

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

### `copy`

Streams database rows directly from one database to another, without an
intermediate file.

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

## Migrate SQLite → PostgreSQL (Kubernetes)

A step-by-step runbook for moving a single-node SQLite instance to a
PostgreSQL-backed, worker-separated deployment.

1. **Quiesce the source.** Stop writes to the running instance (scale it down or
   take it offline). The export reads a consistent snapshot; new writes during
   the export would be lost.

2. **Export on the source**, including artifacts (the default):

   ```bash
   codex export --config config/codex.yaml --output codex-migration.tar.gz
   ```

3. **Provision PostgreSQL** and configure the new deployment to use it. **Carry
   over the encryption key** (see the caution below) into the new instance's
   config.

4. **Import on the new instance.** The target is fresh, so no `--replace` is
   needed:

   ```bash
   codex import --config config/codex.yaml --input codex-migration.tar.gz
   ```

   Migrations run, rows load, artifacts unpack, and file paths are re-rooted to
   the new instance's directories.

5. **Verify.** Review the import summary (per-table row counts, re-rooted path
   counts). Bring up the server and worker, open the app, and confirm covers,
   thumbnails, and reading progress are intact.

:::danger Carry over the encryption key
Encrypted values (such as plugin credentials) are copied as **ciphertext** and
are never decrypted during migration. The destination instance must be
configured with the **same encryption key** as the source, or those values
cannot be decrypted after the move.
:::

:::note PostgreSQL privileges
`import` and `copy` temporarily disable foreign-key enforcement during the bulk
load (`SET session_replication_role = replica`), which requires the target
connection to be a **superuser or the database owner**. This is normally the
case when you provision the database yourself.
:::

## Backup & restore

The same tooling doubles as backup:

```bash
# Nightly backup
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
