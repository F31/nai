## ADDED Requirements
### Requirement: Desktop consumes authoritative finite program progress
The desktop SHALL start and advance the engine program explicitly, using the published four-part outline and current session identity. Progress SHALL come from engine reads, never local increments. Generation SHALL be submitted once and polled within a bounded deadline; timeout SHALL offer read-only refresh without automatic retry. Reconnect SHALL read progress without starting, generating or replaying.
#### Scenario: An ambiguous command result
- **WHEN** a start, next or confirm response is lost
- **THEN** the desktop disables repeat mutation until a successful state refresh and offers a read-only refresh action.
#### Scenario: Candidate association
- **WHEN** another candidate is approved or played, or an offline preview is rendered
- **THEN** the current program candidate is not replaced and its confirm action cannot advance locally.
### Requirement: Four-part editing preserves explicit migration and revocation boundaries
The desktop SHALL offer fixed selling/spec/scenario/after_sales outline fields with the engine's text limits. Legacy outlines SHALL require explicit conversion and publication before program start. Pause/end SHALL request mute, and an unconfirmed mute SHALL not be presented as success. Stale or failed programs SHALL not offer checkbox-based resume or automatic regeneration.
#### Scenario: Existing legacy outline
- **WHEN** the saved outline is not four parts
- **THEN** the desktop explains the conversion and does not silently truncate or change published content.
#### Scenario: Pause during generation
- **WHEN** the operator pauses while a task is pending
- **THEN** mute remains available and late task callbacks cannot update or advance the program display.
