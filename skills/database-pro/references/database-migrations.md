# Database Migrations

Use this reference for production schema changes, data backfills, rollback plans, and zero-downtime releases. Read the target database version and the repository's migration-tool documentation before you choose an operation. Lock and rewrite behavior changes by database version.

## Invariants

- Put each production change in a migration. Do not make an untracked manual schema change.
- Keep a migration immutable after any environment has applied it. Repair it with a new migration.
- Separate schema changes from long data backfills. They have different lock, retry, and recovery needs.
- Keep old and new application versions compatible with the schema during a rolling deployment.
- Prefer forward recovery in production. A destructive reverse migration can lose data that the new version wrote.
- Test with representative data volume and distribution. A small test database does not expose production lock time or backfill duration.

## Plan the Change

1. Inspect the current schema, constraints, indexes, table size, write rate, and dependent code. Done when every reader and writer of the changed data is known.
2. Identify operations that can lock or rewrite the table. Verify their behavior for the exact database version. Done when each operation has a lock and duration expectation.
3. Define the deployment sequence, compatibility period, recovery path, and stop conditions. Done when each release can run safely with the schema present at that point.
4. Rehearse the migration with representative data and production-like settings. Done when duration, locks, replica lag, and application checks meet explicit limits.
5. Apply and observe one phase at a time. Done when the data checks pass and the database returns to its normal operating range.

## Expand, Migrate, Contract

Use this sequence when one release cannot make the full change safely.

### 1. Expand

Add the new column, table, or index without removing the old structure. New columns are usually nullable at this stage. Deploy application code that can work with both schema versions and, when necessary, writes both representations.

### 2. Migrate

Backfill existing rows in bounded batches. Make the job resumable and idempotent. Record progress with a stable key instead of repeatedly scanning from the start. Throttle or stop when lock waits, replication lag, database load, or application errors cross the planned limit.

After the backfill, compare old and new representations with an explicit consistency query. Then deploy code that reads the new representation.

### 3. Contract

Stop old writes first. Remove old application reads and deploy that change. Remove the old column, table, constraint, or index only after no supported application version uses it.

## Common Operations

### Add a required column

1. Add the column as nullable or use an engine-supported metadata-only default.
2. Deploy code that writes the new column.
3. Backfill old rows in bounded transactions.
4. Verify that no invalid or missing values remain.
5. Add the default for future writes if the database must own it.
6. Enforce the constraint with the lowest-lock method supported by the database version.

Do not assume that adding a default is metadata-only. Confirm the exact expression and database version.

### Build an index

For a large existing table, use the database's online or concurrent index operation when available. Confirm whether the migration tool wraps migrations in a transaction.

PostgreSQL `CREATE INDEX CONCURRENTLY` cannot run inside a transaction block. A failed concurrent build can leave an invalid index, so detect and remove that artifact before retrying.

MySQL online DDL support depends on the operation, version, and storage engine. Verify the selected `ALGORITHM` and `LOCK` behavior instead of assuming the statement is non-blocking.

### Rename or remove data

Treat a rename as add, copy, switch, and remove. Treat a deletion as a separate final release. Keep removed data for the agreed recovery window when policy permits it.

### Run a backfill

Use a stable indexed key for keyset batches. Keep each transaction small enough to limit locks, write-ahead log or redo growth, and replica lag. Make retries safe. Measure rows processed, rows remaining, batch duration, errors, and database pressure.

Avoid one large `UPDATE` for a high-volume table. Avoid `OFFSET` pagination because later batches become slower and concurrent writes can change page boundaries.

## Recovery

Choose recovery by failure mode:

- **Application failure:** stop the rollout or restore the earlier application while the expanded schema remains compatible.
- **Backfill failure:** stop, correct the job, and resume from recorded progress.
- **Bad additive schema:** leave unused structure in place until a later migration can remove it safely.
- **Bad destructive schema:** restore from a verified backup or repair with a forward migration. A `DOWN` script is not proof that lost data can return.

Write a reverse migration only when it is safe and useful. Mark an irreversible migration explicitly. Test backup restoration separately from migration rollback.

## Migration Deliverable

Report:

- the affected readers, writers, tables, constraints, and indexes;
- the ordered migration and application deployment phases;
- the expected locks, rewrite risk, duration, and resource limits;
- the rehearsal evidence and consistency queries;
- the monitoring, stop conditions, and forward recovery path.

The migration is ready when every supported application version is compatible with its deployment phase, the representative rehearsal stays inside the limits, and the recovery path has no untested data-restoration assumption.
