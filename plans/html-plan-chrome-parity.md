# HTML plan review: chrome parity (diff badge, review actions, compact Versions)

## Context

HTML/SVG plans now open in Plan chrome (Approve, Send Feedback, Versions tab,
server-side `htmlDiff`). Three pieces of Markdown plan chrome are still gated
on `!isHtmlSurface` in `packages/editor/App.tsx` and so are missing for HTML:

1. Diff stats badge (`+N / -N`, click toggles diff, "vs vN") — Markdown gets
   it from `DocBadges` -> `PlanDiffBadge`; HTML only has the plain
   "Show changes" button inside `HtmlViewer`'s floating cluster.
2. Compact-touch review completion / actions (`CompactPlanCompletion`,
   line ~5685) — hidden for HTML, so on touch layouts there is no in-page
   Approve / Feedback entry point.
3. Versions tab in the compact navigator (line ~5485) — hidden for HTML, so
   touch layouts cannot browse versions or pick a diff base.

Desktop Annotate HTML (`annotateMode`) must stay untouched.

## Approach

Keep the split already in place: `isHtmlPlanReview = isHtmlSurface && !annotateMode`.
Replace the three `!isHtmlSurface` gates with `!isHtmlSurface || isHtmlPlanReview`
(i.e. only Annotate HTML stays gated), and feed HTML diff stats from the
server-produced `diffHtml`.

Stats: `htmlDiff` emits `<ins>`/`<del>` around changed text tokens. Count them.
One tiny helper, `htmlDiffStats(diffHtml): PlanDiffStats` (additions = `<ins`
count, deletions = `<del` count, modifications = 0). No tree walk.

Badge placement for HTML: `HtmlViewer` already floats the action cluster
top-right in `fullViewport` mode. Add an optional `planDiffBadge?: ReactNode`
slot rendered at the start of that cluster; App passes `<PlanDiffBadge …>`
with the computed stats and `baselineLabel={`vs v${base}`}` when a base
version is selected. Reuses `PlanDiffBadge` unchanged; no new banner component.

## Files to modify

- `packages/shared/html-diff.ts` — add `htmlDiffStats`.
- `packages/shared/html-diff.test.ts` — one test: 2 ins + 1 del -> `{2,1,0}`.
- `packages/ui/components/html-viewer/HtmlViewer.tsx` — `planDiffBadge` slot in the floating/normal action cluster.
- `packages/editor/App.tsx`:
  - `htmlDiffStats = useMemo(() => htmlDiffHtml ? htmlDiffStats(htmlDiffHtml) : null)`.
  - Pass `planDiffBadge` to `HtmlViewer` when `isHtmlPlanReview`.
  - Compact navigator tabs (~5485): `versions` when `usesPlanVersionChrome`.
  - `showCompactPlanCompletion` (~5685): drop `!isHtmlSurface`, use `!isHtmlAnnotateSurface`; in the HTML flex column it renders below the iframe as a fixed bottom strip (iframe is `flex-1`).
- `packages/editor/App.htmlChrome.test.tsx` — extend the HTML plan test: badge shows `+N`, click toggles diff; compact mount shows Versions in navigator.

## Reuse

- `PlanDiffBadge` — `packages/ui/components/plan-diff/PlanDiffBadge.tsx` (stats + toggle + baseline label).
- `PlanDiffStats` type — `packages/ui/utils/planDiffEngine.ts`.
- `usesPlanVersionChrome`, `isHtmlPlanReview`, `isHtmlAnnotateSurface`, `htmlDiffHtml`, `isPlanDiffActive`, `planDiff.diffBaseVersion` — already in `App.tsx`.
- `HtmlViewer` floating cluster (`fullViewport && !hideControls && hasActionButtons`) — `HtmlViewer.tsx:1064`.
- `CompactPlanCompletion` — already wired; only the gate changes.
- Existing test harness: `mountHtmlAnnotate(htmlPlanFetch)` / `mountCompactHtmlAnnotate` in `App.htmlChrome.test.tsx`.

## Steps

- [ ] `htmlDiffStats` helper + test in `packages/shared/html-diff.ts`.
- [ ] `HtmlViewer`: `planDiffBadge` prop rendered first in `actionButtons`.
- [ ] `App.tsx`: compute stats, pass badge for `isHtmlPlanReview`; base label from `planDiff.diffBaseVersion`.
- [ ] `App.tsx`: compact navigator `versions` tab + `showCompactPlanCompletion` use the annotate-only gate.
- [ ] Tests: extend `App.htmlChrome.test.tsx` (badge text, toggle, compact Versions tab).
- [ ] `bun run build:pi` + `pi install /Users/yassin/projects/personal/plannotator/apps/pi-extension`, restart Pi.

## Verification

1. `bun test packages/shared/html-diff.test.ts`
2. `DOM_TESTS=1 bun test packages/editor/App.htmlChrome.test.tsx` — all pass, Annotate tests unchanged.
3. `bunx tsc --noEmit -p packages/ui/tsconfig.json && bunx tsc --noEmit -p apps/pi-extension/tsconfig.json`
4. Manual: resubmit `plans/inline-html-plan.html` after editing one line. Expect top-right `+1 / -1` badge; click -> inline `<ins>/<del>` in the page, badge highlighted; pick v1 in Versions -> badge reads "vs v1".
5. Manual compact: narrow window / touch emulation. Expect Versions in the navigator and the review completion strip under the page.

~45 min. Time is mostly the two DOM tests.
