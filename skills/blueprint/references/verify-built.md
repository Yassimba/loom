# Verify promise against build

Read the locked `approved-plan.md`, `approval.json`, and `overlay.json`.
For legacy packages, use the existing `changes.json` and `evidence.json`.

1. Pin the actual implementation range. The approval identifies HEAD and the
   working-tree hash; reconstruct a dirty baseline from its preserved source
   if available, otherwise explicitly report that comparison limitation.
2. For every C-ID and acceptance criterion, inspect the relevant built source
   and record delivered, amended, missing, or additional drift, with source
   references and any acceptance decision.
3. Reuse unchanged approved figures. Update only figures affected by the build
   or needed to explain drift, following the shared atlas consumer procedure.
   Bind actual code and keep projected promises distinct from built behavior.
4. Write a separate PROMISED → BUILT review with dispositions, verification
   results, and the relevant implementation difference. Review it through
   Plannotator; fix surprises or obtain explicit acceptance.

Done when every promised change and acceptance criterion is accounted for and
unplanned structural changes are explained. Preserve the approved package.
