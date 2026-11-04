## 1. Contracts and prerequisite
- [ ] 1.1 Finish `v48-avatar-management`: native media transfer, package/rights authority, atomic selection and revocation, real IPC and UI verification.
- [ ] 1.2 Define renderer-neutral CharacterProfile, capability, action request, state and explicit 3D extension boundaries in schemas; generate Rust/TS/C++ and verify no drift and interop.
- [ ] 1.3 Define bounded scheduling, identity, cancellation, lease, audit, conflicts and safe fallback invariants; add rejection and supported-action acceptance cases.

## 2. 2D media implementation
- [ ] 2.1 Implement isolated 2D package load and preview; enforce closure, digest, rights, size, path and cancellation checks.
- [ ] 2.2 Implement sample-cursor-driven lip movement for the selected 2D character and truthful quality/status reporting.
- [ ] 2.3 Implement bounded predefined action tracks, priorities, safe transitions and observable completion; test supported and unsupported actions.
- [ ] 2.4 Bind asset identity and rights to public preparation, approval, execution renewal and stop/revocation; fault-test stale epoch, forged identity and lost renderer.

## 3. Operator workflow
- [ ] 3.1 Add character selection, preview and available actions before opening public output; show unavailable capabilities and recoverable errors.
- [ ] 3.2 Add manual action commands and explicit feedback, then connect approved program/agent action proposals through the same authority path.
- [ ] 3.3 Verify UI, native IPC, workspace isolation, selection changes and revoke/stop without automatic replay.

## 4. Acceptance and closeout
- [ ] 4.1 Run targeted contract, component, state-machine, cancellation, IPC and media fault tests; document limitations and budget.
- [ ] 4.2 With authorized assets, validate Windows selected-character public frames, actual local audio/lip sync, each supported action, stop/revoke and OBS recording; measure timing and preserve failed evidence.
- [ ] 4.3 Update development plans and report; perform maintenance-window update and native UI verification, archive only completed scope. Keep product ACC pending absent full registered evidence.
