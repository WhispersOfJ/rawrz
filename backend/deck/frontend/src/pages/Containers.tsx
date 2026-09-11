import { makePlaceholder } from "./Placeholder";

export const FEATURE_IDS = ["ctnr.lifecycle", "ctnr.images"];

export default makePlaceholder({
  title: "Containers",
  featureIds: FEATURE_IDS,
  milestone: "M1 (reads) / M3 (mutations)",
});
