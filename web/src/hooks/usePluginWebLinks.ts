import { useQuery } from "@tanstack/react-query";
import { pluginsApi } from "@/api/plugins";

/**
 * Web-link providers: enabled plugins declaring the `webLinks` capability,
 * with `{config.*}` placeholders already resolved server-side. The series
 * detail page turns these into "open on <site>" buttons, filling the
 * runtime placeholders (`{title}`, `{externalId}`) per series.
 *
 * The response only changes when an admin edits plugin config or toggles a
 * plugin, so treat it as essentially static for a session (same reasoning
 * as `useReleaseTrackingApplicability`).
 */
export function usePluginWebLinks() {
  return useQuery({
    queryKey: ["plugin-web-links"],
    queryFn: () => pluginsApi.getWebLinks(),
    staleTime: 5 * 60 * 1000,
  });
}
