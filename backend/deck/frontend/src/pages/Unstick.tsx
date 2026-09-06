import { makePlaceholder } from "./Placeholder";

export const FEATURE_IDS = ["stick.queue", "stick.backlog", "stick.decide"];

export default makePlaceholder({
  title: "Unstick",
  featureIds: FEATURE_IDS,
  milestone: "M1 (queue views) / M3 (decisions)",
});
