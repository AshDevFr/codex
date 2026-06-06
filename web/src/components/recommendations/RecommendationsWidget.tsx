import { useQuery } from "@tanstack/react-query";
import { recommendationsApi } from "@/api/recommendations";
import { userPluginsApi } from "@/api/userPlugins";
import { HorizontalCarousel } from "@/components/library/HorizontalCarousel";
import { RecommendationCompactCard } from "./RecommendationCompactCard";

const MAX_RECOMMENDATIONS = 20;

export function RecommendationsWidget() {
  const { data: pluginData } = useQuery({
    queryKey: ["user-plugins"],
    queryFn: userPluginsApi.list,
    staleTime: 5 * 60_000,
  });
  const hasRecommendationPlugin = pluginData?.enabled?.some(
    (p) => p.connected && p.capabilities?.userRecommendationProvider === true,
  );

  const { data } = useQuery({
    queryKey: ["recommendations"],
    queryFn: recommendationsApi.get,
    enabled: hasRecommendationPlugin === true,
    retry: false,
  });

  // Don't render anything if no data or no recommendations
  const recommendations = data?.recommendations ?? [];
  if (recommendations.length === 0) {
    return null;
  }

  const limited = recommendations.slice(0, MAX_RECOMMENDATIONS);
  const sourceNames = (data?.sources ?? []).map((s) => s.pluginName);
  const subtitle =
    sourceNames.length > 0 ? `Powered by ${sourceNames.join(", ")}` : undefined;

  return (
    <HorizontalCarousel title="Recommended For You" subtitle={subtitle}>
      {limited.map((rec) => (
        <RecommendationCompactCard key={rec.externalId} recommendation={rec} />
      ))}
    </HorizontalCarousel>
  );
}
