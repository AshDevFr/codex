import { useQuery } from "@tanstack/react-query";
import { recommendationsApi } from "@/api/recommendations";
import { HorizontalCarousel } from "@/components/library/HorizontalCarousel";
import { RecommendationCompactCard } from "./RecommendationCompactCard";

const MAX_RECOMMENDATIONS = 20;

export function RecommendationsWidget() {
  const { data } = useQuery({
    queryKey: ["recommendations"],
    queryFn: recommendationsApi.get,
    retry: false,
  });

  // Don't render anything if no data or no recommendations
  const recommendations = data?.recommendations ?? [];
  if (recommendations.length === 0) {
    return null;
  }

  const limited = recommendations.slice(0, MAX_RECOMMENDATIONS);
  const subtitle = data?.pluginName
    ? `Powered by ${data.pluginName}`
    : undefined;

  return (
    <HorizontalCarousel title="Recommended For You" subtitle={subtitle}>
      {limited.map((rec) => (
        <RecommendationCompactCard key={rec.externalId} recommendation={rec} />
      ))}
    </HorizontalCarousel>
  );
}
