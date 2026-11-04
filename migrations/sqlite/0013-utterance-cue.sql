-- The approved cue must be reconstructible from its hash. display_text alone
-- cannot re-derive the hash (it also covers evidence refs, epoch and expiry),
-- so the exact validated cue is stored beside the candidate hash.
ALTER TABLE utterances ADD COLUMN cue_blob BLOB;
