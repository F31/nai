## ADDED Requirements
### Requirement: Resident audio submission requires actual device position
The resident backend SHALL maintain a single safe WASAPI stream and derive played frames from IAudioClock position and frequency. Successful writes SHALL bind each cue tail to absolute stream frames and a denial-only revocation/epoch/deadline fence. Only subsequent valid device position at or beyond that tail SHALL produce the cue played count. Buffers and queues SHALL be bounded, and P0 SHALL remain independent of model/storage locks.
#### Scenario: Copied or predicted time alone
- **WHEN** samples are copied but device position has not reached their committed tail
- **THEN** full device progress is not reported and submission is not successful.
#### Scenario: Cancellation or replacement
- **WHEN** revocation, expiry, epoch change or cue failure invalidates the pending tail
- **THEN** later device progress cannot produce its receipt or transfer it to a new cue.
### Requirement: Host verifies native progress identity
The private status SHALL carry optional device played frames, an observation flag and backend identity. Both host success exits SHALL require exact descriptor identity and independently expected full counts from the known backend. Missing or predicted-only evidence SHALL be rejected. Actual audio progress SHALL NOT create full A/V PlaybackAck or automatic program advancement.
#### Scenario: Old or forged status
- **WHEN** backend identity, observed flag or expected full played count is missing or mismatched
- **THEN** the host rejects successful submission.

## REMOVED Requirements
### Requirement: Native submission requires a subsequent callback tail observation
**Reason**: Production submission now requires actual device position rather than predicted callback time.
**Migration**: Existing callback fields remain optional legacy diagnostics; require the cue-bound actual device progress fields and known backend for submission.
### Requirement: Host binds callback diagnostics to the current descriptor
**Reason**: Descriptor binding now applies to actual device progress; callback drain cannot independently authorize successful submission.
**Migration**: Use Host verifies native progress identity, reject old predicted-only receipts, and retain actual_local_playback_completed=false.
