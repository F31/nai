## ADDED Requirements
### Requirement: Device clock admission before completion integration
The implementation SHALL validate device position using the same stream identity and a stable nonzero clock frequency, converting position to frames with checked integer arithmetic. Regressions, frequency changes and positions beyond submitted frames SHALL be rejected. QPC and elapsed time SHALL NOT substitute device position.
#### Scenario: Invalid cursor
- **WHEN** a clock regresses, changes frequency or exceeds committed frames
- **THEN** it is rejected without producing completion evidence.
#### Scenario: Explicit native admission
- **WHEN** the Windows admission test is explicitly invoked
- **THEN** it drives only bounded silent buffers, observes actual clock progress and verifies stopped position without producing PlaybackAck or advancing the program.
