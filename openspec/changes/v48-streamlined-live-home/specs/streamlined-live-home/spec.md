# Streamlined live home

## ADDED Requirements

### Requirement: Open public output directly
The live home SHALL provide an action that opens the native public output window without navigating to an intermediate configuration page. This action SHALL NOT claim that a platform broadcast has started.

#### Scenario: Operator opens the window
- **WHEN** the operator activates the output action while connected
- **THEN** the workbench opens the existing public standby window and reports errors if opening fails
