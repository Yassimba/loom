// Same session-backed virtual authoring module used by Review documents.
import { calls, createReviewDefinitionSession } from "./authoring";

const session = createReviewDefinitionSession({});
session.begin();

export const defineActors = session.defineActors;
export const defineAnchors = session.defineAnchors;
export const defineStores = session.defineStores;
export const defineSoftwareActors = session.defineSoftwareActors;
export const defineSoftwareStores = session.defineSoftwareStores;
export { calls };
export const __reviewDefinitionsReady = session.ready;
