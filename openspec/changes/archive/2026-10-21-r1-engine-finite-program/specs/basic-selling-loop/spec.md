## ADDED Requirements

### Requirement: Engine owns one finite assisted program per epoch
The engine SHALL bind one four-segment assisted program to the current knowledge snapshot and epoch. Start SHALL perform no Provider calls. Explicit next SHALL generate each segment at most once, in selling/spec/scenario/after_sales order, using existing guards, approval and durable budgets. A failure SHALL terminate the program without automatic retry. The program SHALL expire within fifteen minutes. A second start in the same epoch SHALL be refused.

#### Scenario: Duplicate or premature next
- **WHEN** generation is in flight or the current segment has not been confirmed
- **THEN** another next is refused without another Provider dispatch; replay of the identical command returns its original ACK.

#### Scenario: Missing timed evidence
- **WHEN** a program segment retrieves evidence without declared active validity
- **THEN** generation fails before Provider dispatch and no automatic retry occurs.

### Requirement: Assisted confirmation cannot become hardware completion
The engine SHALL accept operator confirmation only for the current request's exact candidate hash after its local device submission. Preview rendering alone SHALL NOT qualify. Confirmation SHALL NOT produce PlaybackAck or completed-sentence evidence. Epoch, snapshot and program deadline SHALL be rechecked on every mutation; stale programs SHALL NOT resume automatically.

#### Scenario: Wrong candidate or preview only
- **WHEN** confirmation references another candidate or only an offline preview exists
- **THEN** confirmation is refused and the program does not advance.

#### Scenario: Takeover before next
- **WHEN** takeover changes the epoch
- **THEN** old program commands are refused and cannot dispatch another generation; no five-face Fresh result is fabricated.
