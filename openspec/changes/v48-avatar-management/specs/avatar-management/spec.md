# Avatar Management

## ADDED Requirements

### Requirement: Native admission is authoritative
The desktop owner SHALL accept only bounded local 2D packages whose manifest, renderer configuration, asset hashes, dimensions and explicit license grant pass the existing avatar renderer admission. Page-side preview alone SHALL NOT establish admission.

#### Scenario: Invalid package is rejected
- **WHEN** any required file, digest, schema field, license grant or byte bound is invalid
- **THEN** no immutable package directory or selectable workspace profile is created

### Requirement: Workspace binding is immutable and revocable
The system SHALL store only an immutable package identity and profile metadata in the workspace control database. Selecting and revoking SHALL create auditable CAS revisions and SHALL NOT grant public media playback.

#### Scenario: Revocation falls back safely
- **WHEN** the selected custom profile is revoked
- **THEN** the workbench reports the built-in avatar as fallback and no prior custom package is treated as authorized for a new execution

### Requirement: Preview uses admitted pixels
The workbench SHALL display a bounded neutral preview produced from the native-admitted package and clearly distinguish preview/selection from live output readiness.

#### Scenario: Operator previews an admitted package
- **WHEN** native admission succeeds
- **THEN** the returned preview corresponds to the admitted neutral frame and the UI identifies its package and license digests

### Requirement: Backup manifest admits bounded avatar files
The model migration backup manifest SHALL identify only the fixed installed avatar package filenames under a lowercase SHA-256 package directory, alongside the existing database and object paths. Accepting a manifest path SHALL NOT imply that the file exists, that the backup is complete, or that the avatar is licensed.

#### Scenario: Reject an escaped or undeclared avatar path
- **WHEN** an avatar path contains traversal, an invalid package digest, or a filename outside the fixed package set
- **THEN** the manifest validator rejects the backup
