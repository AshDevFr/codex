# Migrate SQLite → PostgreSQL

A step-by-step runbook for moving a single-node SQLite instance to a
PostgreSQL-backed, worker-separated deployment (e.g. on Kubernetes) with a
faithful 1:1 copy of all data — metadata, custom data, ratings, reading
progress, uploaded covers, and plugin state.

This uses the [`export` and `import`](./export-import-copy) commands; see that
page for full command and flag reference.

## Before you start

- Confirm the source instance is **fully migrated** (run `codex migrate` if it
  is behind). `import` refuses an archive whose schema version doesn't match the
  target.
- Use the **same Codex version** on both ends, so their schemas match.
- Take a quick safety copy of the source database file:
  ```bash
  cp data/codex.db data/codex.db.bak
  ```

## Steps

1. **Quiesce the source.** Stop writes to the running instance (scale it down or
   take it offline). The export reads a consistent snapshot; new writes during
   the export would be lost.

2. **Export on the source**, including artifacts (the default):

   ```bash
   codex export --config config/codex.yaml --output codex-migration.tar.gz
   ```

3. **Provision PostgreSQL.** Create the empty database and role (your chart /
   operator / an init job typically does this), and configure the new deployment
   to use it:

   ```sql
   CREATE DATABASE codex;
   CREATE USER codex WITH PASSWORD '...';
   ALTER DATABASE codex OWNER TO codex;
   ```

   **Carry over the encryption key** (see below) into the new instance's config.

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
`import` temporarily disables foreign-key enforcement during the bulk load
(`SET session_replication_role = replica`), which requires the target
connection to be a **superuser or the database owner**. This is normally the
case when you provision the database yourself.
:::

## Library files

The migration moves the database and the artifacts Codex generates (thumbnails,
uploaded covers, plugin data). It does **not** move your **library files** (the
CBZ/EPUB/PDF) — the new deployment is expected to mount the same library volume.
Point the new instance's libraries at those paths and it will match them up to
the imported metadata.
