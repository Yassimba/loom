# Review port ledger

Oracle: `devdotfast/review` commit `8267620cd4aee9cc031ae705c4fed6eae0fc9dbe`.

| Review source | Loom destination | Disposition | Reason and parity check |
| --- | --- | --- | --- |
| `packages/progressive-review/app/src/diagrams.tsx` | `scripts/src/viewer.tsx` | Mechanical adaptation | Preserves participant topological ordering, lane measurement, message spacing, React Flow handles, straight/self routes, active scrolling, and interaction identities. Removes Review Desktop comments, telemetry, persisted threads, and session contexts; the offline source panel replaces the host-native peek. Protected by `test/code-diagram.test.mjs` and the canonical fixture. |
| `packages/progressive-review/app/src/styles.css` | `scripts/src/review-styles.css` | Verbatim | Copied whole to avoid selector/token drift during parity work. Bundled into `viewer.css`; bundle freshness test is byte-exact. |
| `packages/progressive-review/src/authoring.ts` | `scripts/authoring.mjs` | Intentional narrow port | Keeps strict unknown-field, non-empty label, endpoint, parallel-label, evidence, and range rules for Sequence Diagram only. Other Review authoring surfaces remain deferred. Protected by CLI rejection tests. |
| `packages/progressive-review/app/src/diagram-tour.tsx` and `CodePeek.tsx` | `scripts/src/viewer.tsx`, `scripts/src/viewer.css` | Intentional host adaptation | Review's tour assumes Desktop providers and native source editors. The standalone host keeps the full-screen stage, active stop, next/previous, Escape close, and exact source display without network or server state. Protected by the self-contained HTML fixture. |
| `packages/progressive-review/LICENSE` | `REVIEW_LICENSE` | Verbatim | Required MIT notice. |

This ledger covers the Sequence Diagram preview only. Call Stack Diff, Database Lens, and Software Map have not been ported and no full Review parity claim is made.
