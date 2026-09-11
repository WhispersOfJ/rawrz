import { makePlaceholder } from "./Placeholder";

export const FEATURE_IDS = ["lib.health", "lib.lists", "lib.prune", "lib.watchable"];

export default makePlaceholder({
  title: "Library",
  featureIds: FEATURE_IDS,
  milestone: "M3 / M5",
});
