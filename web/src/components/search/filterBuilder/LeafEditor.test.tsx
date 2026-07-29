import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { SeriesCondition } from "@/types/filters";
import {
  defaultOperator,
  isLeafComplete,
  newLeaf,
  normalizeForEmit,
  OPERATOR_LABELS,
  operatorsForField,
  updateLeafOperator,
} from "./conditionUtils";
import { findField } from "./fieldCatalog";
import { LeafEditor } from "./LeafEditor";

const USER_RATING = findField("series", "userRating")!;
const COMMUNITY_RATING = findField("series", "communityRating")!;
const LIBRARY_ID = findField("series", "libraryId")!;

function renderLeaf(condition: SeriesCondition, fieldKey: string) {
  const onChange = vi.fn();
  renderWithProviders(
    <LeafEditor
      condition={condition}
      target="series"
      fieldKey={fieldKey}
      onChange={onChange}
      onRemove={vi.fn()}
    />,
  );
  return onChange;
}

describe("LeafEditor rating input", () => {
  it("defaults a fresh rating leaf to 'at least'", () => {
    expect(defaultOperator(USER_RATING)).toBe("gte");
    expect(defaultOperator(COMMUNITY_RATING)).toBe("gte");
  });

  // The condition carries the stored 1-100 scale but the box shows 1-10, which
  // is the scale every other rating surface in the app uses.
  it("shows a stored 85 as 8.5", () => {
    renderLeaf({ userRating: { operator: "gte", value: 85 } }, "userRating");
    expect(screen.getByLabelText("My rating")).toHaveValue("8.5");
  });

  it("submits a typed 8.5 as a stored 85", async () => {
    const user = userEvent.setup();
    const onChange = renderLeaf(
      { userRating: { operator: "gte", value: 0 } },
      "userRating",
    );

    const input = screen.getByLabelText("My rating");
    await user.clear(input);
    await user.type(input, "8.5");

    const last = onChange.mock.calls.at(-1)![0] as {
      userRating: { value: number };
    };
    expect(last.userRating.value).toBe(85);
  });

  it("round-trips 8.5 through store and display", async () => {
    const user = userEvent.setup();
    const onChange = renderLeaf(
      { userRating: { operator: "gte", value: 0 } },
      "userRating",
    );

    const input = screen.getByLabelText("My rating");
    await user.clear(input);
    await user.type(input, "8.5");

    const stored = onChange.mock.calls.at(-1)![0] as SeriesCondition;
    expect(stored).toEqual({ userRating: { operator: "gte", value: 85 } });

    // Re-render from what we emitted; the box must read 8.5 again.
    renderWithProviders(
      <LeafEditor
        condition={stored}
        target="series"
        fieldKey="userRating"
        onChange={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    const inputs = screen.getAllByLabelText("My rating");
    expect(inputs.at(-1)).toHaveValue("8.5");
  });

  it("renders two bounds for a between range on the display scale", () => {
    renderLeaf(
      { communityRating: { operator: "between", min: 70, max: 90 } },
      "communityRating",
    );
    // 70 and 90 stored are 7 and 9 on the display scale. `decimalScale` caps
    // precision rather than padding, so a whole number shows without a decimal.
    expect(screen.getByLabelText("Community rating minimum")).toHaveValue("7");
    expect(screen.getByLabelText("Community rating maximum")).toHaveValue("9");
  });

  it("leaves an open bound blank rather than showing 0", () => {
    renderLeaf(
      { communityRating: { operator: "between", min: 85 } },
      "communityRating",
    );
    expect(screen.getByLabelText("Community rating minimum")).toHaveValue(
      "8.5",
    );
    expect(screen.getByLabelText("Community rating maximum")).toHaveValue("");
  });

  it("renders no value input for the nullability operators", () => {
    renderLeaf({ userRating: { operator: "isNull" } }, "userRating");
    expect(screen.queryByLabelText("My rating")).not.toBeInTheDocument();
  });
});

describe("LeafEditor library list operators", () => {
  const LIB_A = "11111111-1111-1111-1111-111111111111";
  const LIB_B = "22222222-2222-2222-2222-222222222222";

  it("offers the list operators for uuid fields", () => {
    expect(operatorsForField(LIBRARY_ID)).toEqual([
      "is",
      "isNot",
      "in",
      "notIn",
    ]);
    expect(OPERATOR_LABELS.uuid.in).toBe("is any of");
    expect(OPERATOR_LABELS.uuid.notIn).toBe("is none of");
  });

  it("renders a multi-select for a library list operator", () => {
    renderLeaf(
      { libraryId: { operator: "in", values: [LIB_A, LIB_B] } },
      "libraryId",
    );
    // Without library data loaded the picker falls back to the raw-UUID input,
    // which still has to show both values rather than dropping one.
    expect(screen.getByDisplayValue(`${LIB_A}, ${LIB_B}`)).toBeInTheDocument();
  });

  // Switching operators must not silently discard the library already chosen.
  it("carries a single value into the list when switching to `in`", () => {
    const next = updateLeafOperator(
      { libraryId: { operator: "is", value: LIB_A } },
      LIBRARY_ID,
      "in",
    );
    expect(next).toEqual({
      libraryId: { operator: "in", values: [LIB_A] },
    });
  });

  it("carries the first list value back when switching to `is`", () => {
    const next = updateLeafOperator(
      { libraryId: { operator: "in", values: [LIB_A, LIB_B] } },
      LIBRARY_ID,
      "is",
    );
    expect(next).toEqual({ libraryId: { operator: "is", value: LIB_A } });
  });

  it("does not invent a value when switching from an empty `is`", () => {
    const next = updateLeafOperator(
      { libraryId: { operator: "is", value: "" } },
      LIBRARY_ID,
      "in",
    );
    expect(next).toEqual({ libraryId: { operator: "in", values: [] } });
  });

  // An empty `in` matches nothing and an empty `notIn` matches everything;
  // neither is what a half-finished selection means, so it must not be sent.
  it("treats an empty list as incomplete", () => {
    expect(isLeafComplete({ libraryId: { operator: "in", values: [] } })).toBe(
      false,
    );
    expect(
      isLeafComplete({ libraryId: { operator: "notIn", values: [] } }),
    ).toBe(false);
    expect(
      isLeafComplete({ libraryId: { operator: "in", values: [LIB_A] } }),
    ).toBe(true);
  });

  it("prunes an empty list leaf before emitting", () => {
    expect(
      normalizeForEmit(
        { allOf: [{ libraryId: { operator: "in", values: [] } }] },
        "series",
      ),
    ).toBeUndefined();

    expect(
      normalizeForEmit(
        { allOf: [{ libraryId: { operator: "in", values: [LIB_A] } }] },
        "series",
      ),
    ).toEqual({ libraryId: { operator: "in", values: [LIB_A] } });
  });

  it("keeps `is` as the default for a fresh library leaf", () => {
    expect(newLeaf(LIBRARY_ID)).toEqual({
      libraryId: { operator: "is", value: "" },
    });
  });
});
