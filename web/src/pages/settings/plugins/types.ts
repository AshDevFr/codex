import type { PluginHealthStatus } from "@/api/plugins";

// Health status badge color mapping
export const healthStatusColors: Record<PluginHealthStatus, string> = {
  unknown: "gray",
  healthy: "green",
  degraded: "yellow",
  unhealthy: "orange",
  disabled: "red",
};
