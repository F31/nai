# Real-time avatar performance

## Why
The operator needs to select an authorized character, speak with synchronized mouth movement, and trigger bounded actions during a live presentation. The current public output uses only the built-in 2D avatar; custom package admission is incomplete and does not authorize public rendering.

## Scope
Deliver a 2D-first real-time path: authorized package import and workspace selection; native preview; source-sample-driven lip movement; declared action clips and operator commands; supervised output with cancellation and revocation; and Windows/OBS acceptance. Define renderer-neutral capability and command contracts for a future 3D adapter without claiming a 3D implementation.

The existing `v48-avatar-management` change owns package admission and selection. This change consumes its verified identity and grants rather than duplicating or weakening that work.

## Non-goals
Single-photo animation, person cloning or training, 3D implementation, arbitrary generated motion, automatic platform authorization, or asserting third-party asset rights from a checkbox.
