-- W132 (§47.4 / INV-32 / H17): a named degradation needs its own decision-log kind.
--
-- 0005 fixed the kind set as a closed CHECK; 0007 added `assembly` and 0011 adds `degradation`.
-- The Rust `DecisionKind` enum is the same closed set in a second place. A kind added to the enum
-- but not here is refused by the CHECK and — because such a kind is telemetry — silently falls
-- into the buffer, so the two MUST stay in lockstep. That failure mode is not hypothetical: it is
-- how W104 found the seam.
--
-- `degradation` is graded **critical** in the enum, so it is never buffered: a degradation the
-- interface never shows is the silent degradation INV-32 forbids.
--
-- SQLite cannot alter a CHECK in place, so the table is rebuilt column-for-column, the rows are
-- copied verbatim (hashes included), and the append-only triggers are recreated. The chain is
-- unchanged: `verify_decision_chain` recomputes exactly the same hashes afterwards, because the
-- preimage never mentions the table name.
CREATE TABLE decision_log_rebuilt(seq INTEGER PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces, session_id TEXT, kind TEXT NOT NULL CHECK(kind IN('event','observation','assembly','degradation','decision','guard','cue','approval','synthesis','playback_ack','cancellation','expiry','usage','duty_lease')), criticality TEXT NOT NULL CHECK(criticality IN('critical','telemetry')), payload_json TEXT NOT NULL CHECK(json_valid(payload_json)), payload_hash TEXT NOT NULL, prev_hash TEXT NOT NULL, entry_hash TEXT NOT NULL, created_at_ms INTEGER NOT NULL, trace_id TEXT NOT NULL) STRICT;
INSERT INTO decision_log_rebuilt SELECT seq,workspace_id,session_id,kind,criticality,payload_json,payload_hash,prev_hash,entry_hash,created_at_ms,trace_id FROM decision_log;
DROP TRIGGER decision_log_append_only_update;
DROP TRIGGER decision_log_append_only_delete;
DROP TABLE decision_log;
ALTER TABLE decision_log_rebuilt RENAME TO decision_log;
CREATE INDEX decision_log_workspace_seq ON decision_log(workspace_id,seq);
CREATE TRIGGER decision_log_append_only_update BEFORE UPDATE ON decision_log BEGIN SELECT RAISE(ABORT,'decision_log is append-only'); END;
CREATE TRIGGER decision_log_append_only_delete BEFORE DELETE ON decision_log BEGIN SELECT RAISE(ABORT,'decision_log is append-only'); END;
