import { makePlaceholder } from "./Placeholder";

export const FEATURE_IDS = [
  "host.disk", "host.mem", "host.journal", "host.services", "host.pkg",
  "host.aur", "host.btrfs", "host.smart", "host.reboot", "host.cron",
  "host.git", "host.firewall", "host.ssh", "host.uptime", "host.backup",
  "host.drift", "host.residue", "host.perms",
];

export default makePlaceholder({
  title: "Host",
  featureIds: FEATURE_IDS,
  milestone: "M1",
});
