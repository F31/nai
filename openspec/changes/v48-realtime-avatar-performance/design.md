# Design

## Boundaries
Introduce versioned renderer-neutral character capability, action request and performance status contracts before consumers. Bind every operation to workspace, session, epoch, current character package and license digests, content approval and output instance. Control messages contain metadata only; pixels and PCM use bounded media transports. The composition root resolves trusted character identity and current rights; page-provided paths, claimed grants and arbitrary actions do not grant output.

The 2D renderer runs under a declared media host, with explicit supported actions and a finite asset closure. The initial action set is idle, blink, nod, wave and product-display when actually supplied by the package; unsupported actions return unavailable. Predefined bounded clips and priorities arbitrate conflicts; stop, mute, takeover, epoch change, revoked rights or lease loss cancel pending and active actions without waiting for network/model locks.

Lip frames use the actual submitted source sample cursor and existing bounded RMS envelope as the initial quality level. Optional phoneme/audio-driven adapters are separate capabilities; missing timing never claims phoneme-accurate synchronization. Different hosts must report actual renderer/asset identity, generation, frame and sample positions, not inferred readiness. Preview is private and does not sign public media permits.

## Selection and future 3D
Selection is an audited, atomic workspace binding of a validated package version and rights record. Import validates bounded sizes, canonical paths, symlink/reparse handling and hashes and transfers assets outside JSON/Protobuf control. Revalidation occurs before preparation, approval and each bounded execution renewal. Changing selection or revoking rights invalidates outstanding output authority and clears the public frame. 3D may later implement the same metadata/cancellation contract using a separately declared renderer and GPU budget; no dependency from kernel to a specific engine or plugin is introduced.

## Rollout
First close `v48-avatar-management` end to end; then a 2D render adapter and action scheduler; then native composition, safety/fault tests and observable UI; finally Windows public output and OBS recording with authorized assets. Each step preserves existing built-in path until a replacement is independently verified. No schema migration or release activation without backup, recovery journal and maintenance window.
