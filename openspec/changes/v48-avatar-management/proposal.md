# Proposal: V4.8 legal 2D avatar management

## Why
The workbench only exposes a fixed built-in avatar. Operators need a bounded, local workflow to admit a licensed 2D package, preview the admitted pixels, bind one immutable version to a workspace, and revoke that binding without granting media playback.

## What Changes
- Define a workspace avatar-profile document that references an immutable native media package by digest.
- Add native package admission for manifest, renderer config, three RGBA8 assets, and explicit license text/grant.
- Add avatar management UI for directory import, admitted preview, workspace selection, version history, and revocation.
- Keep the built-in avatar as the safe fallback; selection never grants public playback or platform authority.

## Out of Scope
Real-person cloning, training, 3D avatars, remote rendering, automatic platform start, and backward compatibility with legacy avatar formats.
