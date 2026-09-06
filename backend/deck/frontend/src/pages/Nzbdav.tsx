import { makePlaceholder } from "./Placeholder";

export const FEATURE_IDS = ["nzbd.queue", "nzbd.history", "nzbd.stats", "nzbd.mount"];

export default makePlaceholder({
  title: "nzbdav",
  featureIds: FEATURE_IDS,
  milestone: "M1",
});
