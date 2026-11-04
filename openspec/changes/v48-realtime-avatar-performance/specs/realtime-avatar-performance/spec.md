# Real-time avatar performance

## ADDED Requirements

### Requirement: Character capabilities and identity are explicit
The system SHALL bind output to an authorized workspace character version, package digest, rights digest and output instance. It SHALL expose only actions actually declared and available for that character.

#### Scenario: Unknown action is refused
- **WHEN** an operator or agent requests an undeclared action
- **THEN** the action is rejected with an actionable reason and no replacement action is represented as executed

#### Scenario: Character or rights change
- **WHEN** the selected version changes or its usage rights are revoked
- **THEN** old approvals and active output authority are invalidated and the affected public frame is cleared

### Requirement: Audio drives bounded mouth motion
The selected renderer SHALL derive mouth timing from the actual source-sample position and SHALL reject stale identity, malformed input, cancellation and expired execution authority. Simple amplitude-driven movement SHALL be identified as such and not reported as phoneme-accurate lip synchronization.

#### Scenario: Speech and frame progression
- **WHEN** approved speech samples are submitted while the execution lease remains valid
- **THEN** rendered frame metadata identifies the matching sample interval and selected character version

#### Scenario: Stop during speech
- **WHEN** stop, takeover, revocation or lease loss occurs
- **THEN** audio and animated public frames are independently gated and the system does not resume them without new authority

### Requirement: Actions have bounded scheduling and observable results
The system SHALL validate action identity, priority, expiry, resource limits and current session before admission and execution. The action queue SHALL be bounded and cancellation SHALL propagate to the renderer.

#### Scenario: Declared operator action
- **WHEN** an authorized operator requests a supported action with current session and character identity
- **THEN** the system reports accepted, executing, completed, cancelled or failed based on observed execution, without treating acceptance as completion

#### Scenario: Action interrupted
- **WHEN** an emergency stop or lease expiry occurs during an action
- **THEN** queued work is cancelled, the active output is gated and no prior action is automatically replayed

### Requirement: Raw media stays off the control bus
The system SHALL carry raw character assets, pixels and audio through bounded media paths rather than JSON or Protobuf control messages. Package paths and hashes supplied by a page SHALL NOT establish authority.

#### Scenario: Invalid or altered package
- **WHEN** a package is oversized, has unsafe paths, fails integrity or has revoked rights
- **THEN** it cannot be selected for public output and no media permit is issued

### Requirement: Acceptance uses real native output
Windows acceptance SHALL independently verify selected character identity, actual public frames, synchronized audio, action execution, stop/revocation and OBS capture. Test fixtures SHALL NOT establish production asset licensing or platform authorization.

#### Scenario: Selected 2D character in OBS
- **WHEN** a licensed selected 2D package performs approved speech and a supported action
- **THEN** Windows evidence shows the intended character and action in the public output and OBS recording, with audio and frame timing measured and limitations recorded
