# B021-B1 — Mutation receipts, time adjustment, and acknowledgement

Status: COMPLETE / ACCEPTED. Schema 19; current source, not the 0.1.0 binary.

The first live-control slice extends protected OperatorClient attachment with
typed notification acknowledgement and signed current-session time adjustment.
The [accepted contract](m039-tranche-7-b021b-live-operator-controls-gate.md)
remains authoritative. B021-B is accepted as an internal slice; B-021 stays PARTIAL.

## Accepted foundation

- Initial hello uses nine baseline reads; authenticated minor-1.2 discovery
  negotiates control features without breaking older closed-enum peers.
- Six-read bootstrap remains read-only. All mutations require explicit grants;
  the bounded 32-capability profile represents the complete read/control set.
- CommandId, principal/generation-bound semantic SHA-256 fingerprint, and
  retained receipts prevent duplicate effects and resolve uncertain responses.
- Exact daemon/NodeId/SessionId/occupancy targets prevent stale node reuse.
- Dispatch-time revocation overrides earlier discovery or open UI state.
- Typed stale-target rejection retains a rejected receipt and durable audit
  outcome rejected with detail stale-target. Audit failures propagate safely.

## User behavior and authority

Notification acknowledgement checks expected version and retains its source
event. Time adjustment is signed -120..=120 minutes, excludes zero, and changes
only the current live allowance; sfmonitor provides +5/-5 presets. Preflight
and confirmation bind the exact caller. Permanent caller policy and factual
accounting are not rewritten. Earned upload allowance survives refresh and
accepted operator-chat pause remains coherent with time adjustments.

Actions/result feedback is localized and capability-aware. Semantic completion
refreshes daemon projections; response loss retains the original CommandId for
receipt lookup. No direct SQLite/log authority is placed in sfmonitor.

## Acceptance

Persisted-row, injected audit failure, replay/conflict/recovery, stale generation/
node reuse, concurrent adjustment, bootstrap/profile bounds, malformed grants,
live revocation, and monitor-availability regressions pass. Native macOS
acceptance includes explicit enrollment, +/-5, real notification acknowledgement,
receipt recovery, two callers/monitors, and terminal restoration.

Integrated B1/B2/B3 evidence is in the [B3 report](m039-tranche-7-b021b3-shutdown-integrated.md).
Live Windows B021-B mutations/TUI remain DEFERRED — REAL WINDOWS ENVIRONMENT
REQUIRED. No new Windows runtime claim follows from source portability.
