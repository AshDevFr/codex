import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { BehindByBadge } from "./BehindByBadge";

const navigateMock = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual =
    await vi.importActual<typeof import("react-router-dom")>(
      "react-router-dom",
    );
  return {
    ...actual,
    useNavigate: () => navigateMock,
  };
});

const SERIES_ID = "00000000-0000-0000-0000-000000000001";

describe("BehindByBadge", () => {
  it("renders translation chapter badge with delta and unit", () => {
    renderWithProviders(
      <BehindByBadge
        variant="translation"
        axis="chapter"
        delta={3}
        seriesId={SERIES_ID}
        provider="MangaUpdates"
        language="en"
      />,
    );
    expect(
      screen.getByTestId("behind-by-translation-chapter"),
    ).toHaveTextContent("+3 ch (translation)");
  });

  it("renders upstream volume badge with grey/informational variant", () => {
    renderWithProviders(
      <BehindByBadge
        variant="upstream"
        axis="volume"
        delta={1}
        seriesId={SERIES_ID}
        provider="MangaBaka"
      />,
    );
    expect(screen.getByTestId("behind-by-upstream-volume")).toHaveTextContent(
      "+1 vol (upstream)",
    );
  });

  it("hides when delta is zero or negative", () => {
    renderWithProviders(
      <BehindByBadge
        variant="translation"
        axis="chapter"
        delta={0}
        seriesId={SERIES_ID}
      />,
    );
    expect(
      screen.queryByTestId("behind-by-translation-chapter"),
    ).not.toBeInTheDocument();
  });

  it("translation badge navigates to series Releases on click", async () => {
    const user = userEvent.setup();
    navigateMock.mockReset();
    renderWithProviders(
      <BehindByBadge
        variant="translation"
        axis="chapter"
        delta={2}
        seriesId={SERIES_ID}
      />,
    );
    await user.click(screen.getByTestId("behind-by-translation-chapter"));
    expect(navigateMock).toHaveBeenCalledWith(`/series/${SERIES_ID}#releases`);
  });

  it("upstream badge does not navigate (informational only)", async () => {
    const user = userEvent.setup();
    navigateMock.mockReset();
    renderWithProviders(
      <BehindByBadge
        variant="upstream"
        axis="chapter"
        delta={5}
        seriesId={SERIES_ID}
      />,
    );
    await user.click(screen.getByTestId("behind-by-upstream-chapter"));
    expect(navigateMock).not.toHaveBeenCalled();
  });
});
