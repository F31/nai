-- W142 (N1): `utterances.status` was declared in 0001 as a bare `INTEGER NOT NULL DEFAULT 1`
-- with no CHECK, so anything that spoke SQL — including a human with the `sqlite3` shell — could
-- write 7, 0, -1 or 16 into the column. W141 named the domain in Rust
-- (`crates/nai-storage/src/utterance_status.rs`) and made every SQL statement derive its guards
-- from there, but the table itself still accepted any integer. This migration puts the same
-- domain in the schema, so the refusal happens at write time and not at the next read.
--
-- The list is the set `utterance_status::classify` does **not** call `Unassigned`: the pending
-- band 1..=5, the approved value 6, the protected-only value 11 and the unusable band 12..=15.
-- `Unassigned` (0, 7..=10, 16.., negatives) becomes **unrepresentable in storage** — which is the
-- point: a value nobody declared is data corruption, not a state the program has to model.
-- SQLite cannot call Rust, so the literal appears in both places; the lockstep is pinned by
-- `crates/nai-storage/src/tests/status_schema.rs`, whose expected value is derived from
-- `classify` and compared against this table's stored DDL (not against a second hand-written list).
--
-- The table is rebuilt column-for-column because SQLite cannot add a CHECK in place. Two pragmas
-- around the rebuild are load-bearing and were measured, not assumed:
--
--   * `defer_foreign_keys=ON` is required because `evidence_links` has a foreign key into this
--     table. `store::open` turns `foreign_keys` on **before** `migration::migrate` runs, and
--     `PRAGMA foreign_keys` is a no-op inside a transaction (measured: it stays 1), so the
--     migration cannot turn enforcement off. Without deferral the `DROP TABLE` below fails with
--     `FOREIGN KEY constraint failed` on any database that already has evidence links.
--   * `defer_foreign_keys=OFF` is required **after** the rows are back, because the deferred
--     violations are counted rather than re-evaluated: with the flag left on, `COMMIT` fails with
--     `FOREIGN KEY constraint failed` even though `PRAGMA foreign_key_check` is empty (measured).
--     Turning it off runs the check at a point where the parent rows are present again.
--
-- Both failures only appear on a **populated** database, so the upgrade path is covered by a
-- judgment that inserts a real child row before migrating. A rebuild tested on empty tables
-- would have passed with either pragma missing.
PRAGMA defer_foreign_keys=ON;
CREATE TABLE utterances_rebuilt(utterance_id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, session_id TEXT NOT NULL, generation_epoch INTEGER NOT NULL CHECK(generation_epoch>=0), status INTEGER NOT NULL DEFAULT 1 CHECK(status IN(1,2,3,4,5,6,11,12,13,14,15)), text_hash TEXT NOT NULL, expires_at_ms INTEGER NOT NULL, actual_played INTEGER NOT NULL DEFAULT 0 CHECK(actual_played IN(0,1)), display_text TEXT NOT NULL DEFAULT '', FOREIGN KEY(workspace_id,session_id) REFERENCES sessions(workspace_id,session_id), UNIQUE(workspace_id,utterance_id)) STRICT;
INSERT INTO utterances_rebuilt(utterance_id,workspace_id,session_id,generation_epoch,status,text_hash,expires_at_ms,actual_played,display_text) SELECT utterance_id,workspace_id,session_id,generation_epoch,status,text_hash,expires_at_ms,actual_played,display_text FROM utterances;
DROP TABLE utterances;
ALTER TABLE utterances_rebuilt RENAME TO utterances;
PRAGMA defer_foreign_keys=OFF;
