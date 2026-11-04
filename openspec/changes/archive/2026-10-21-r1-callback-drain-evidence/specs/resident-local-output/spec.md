## ADDED Requirements
### Requirement: Native submission requires a subsequent callback tail observation
The resident output SHALL replace fixed post-copy waiting with evidence from a subsequent callback crossing the predicted end of the final audio sample. The observation SHALL require valid timing, unchanged revocation/epoch signals and unexpired authorization. Missing callbacks, stream errors, cancellation or invalid timing SHALL NOT be promoted to complete drain evidence. Callback work SHALL remain bounded, without locks, allocation or IO.
#### Scenario: Copied is not drained
- **WHEN** all samples were copied but no subsequent callback has crossed the tail horizon
- **THEN** the full drain count is not reported and the cue is not submitted successfully.
#### Scenario: Revoke before the next callback
- **WHEN** mute or expiry occurs before a tail observation
- **THEN** the callback cannot record a full drain receipt for that cue.
### Requirement: Host binds callback diagnostics to the current descriptor
The private status SHALL identify the exact media descriptor digest and carry optional drained sample counts. Successful submission SHALL require copied and drained counts equal the independently expected resampled frame count and the descriptor identity to match. These diagnostics SHALL keep actual_local_playback_completed false and SHALL NOT create PlaybackAck, completed sentences or automatic program advancement.
#### Scenario: Foreign or incomplete receipt
- **WHEN** the child reports another descriptor, missing drain evidence, or mismatched sample counts
- **THEN** the host refuses successful submission.
#### Scenario: Real device callback observation
- **WHEN** a native cue completes the bounded callback horizon observation
- **THEN** its stored diagnostic includes the exact descriptor and full count while hardware completion remains unclaimed.
