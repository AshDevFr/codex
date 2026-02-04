import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { SubjectsCount, SubjectsList } from "./SubjectsList";

describe("SubjectsList", () => {
  const mockSubjects = ["Science Fiction", "Space Opera", "Military"];

  it("renders nothing when subjects is null", () => {
    renderWithProviders(<SubjectsList subjects={null} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders nothing when subjects is undefined", () => {
    renderWithProviders(<SubjectsList subjects={undefined} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders nothing when subjects array is empty", () => {
    renderWithProviders(<SubjectsList subjects={[]} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders subjects from array", () => {
    renderWithProviders(<SubjectsList subjects={mockSubjects} />);
    expect(screen.getByText("Science Fiction")).toBeInTheDocument();
    expect(screen.getByText("Space Opera")).toBeInTheDocument();
    expect(screen.getByText("Military")).toBeInTheDocument();
  });

  it("parses and renders subjects from JSON array string", () => {
    const json = JSON.stringify(mockSubjects);
    renderWithProviders(<SubjectsList subjects={json} />);
    expect(screen.getByText("Science Fiction")).toBeInTheDocument();
    expect(screen.getByText("Space Opera")).toBeInTheDocument();
  });

  it("parses and renders subjects from comma-separated string", () => {
    renderWithProviders(<SubjectsList subjects="Fantasy, Adventure, Magic" />);
    expect(screen.getByText("Fantasy")).toBeInTheDocument();
    expect(screen.getByText("Adventure")).toBeInTheDocument();
    expect(screen.getByText("Magic")).toBeInTheDocument();
  });

  it("limits displayed subjects when maxDisplay is set", () => {
    renderWithProviders(
      <SubjectsList subjects={mockSubjects} maxDisplay={2} />,
    );
    expect(screen.getByText("Science Fiction")).toBeInTheDocument();
    expect(screen.getByText("Space Opera")).toBeInTheDocument();
    expect(screen.queryByText("Military")).not.toBeInTheDocument();
    expect(screen.getByText("+1 more")).toBeInTheDocument();
  });

  it("calls onSubjectClick when clickable and clicked", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    renderWithProviders(
      <SubjectsList
        subjects={mockSubjects}
        clickable
        onSubjectClick={onClick}
      />,
    );

    await user.click(screen.getByText("Science Fiction"));
    expect(onClick).toHaveBeenCalledWith("Science Fiction");
  });

  it("handles empty strings in comma-separated values", () => {
    renderWithProviders(<SubjectsList subjects="Fantasy, , Adventure,  " />);
    expect(screen.getByText("Fantasy")).toBeInTheDocument();
    expect(screen.getByText("Adventure")).toBeInTheDocument();
    // Empty strings should be filtered out
    const badges = screen
      .getAllByRole("generic")
      .filter((el) => el.classList.contains("mantine-Badge-root"));
    expect(badges).toHaveLength(2);
  });
});

describe("SubjectsCount", () => {
  it("renders nothing when subjects is null", () => {
    renderWithProviders(<SubjectsCount subjects={null} />);
    expect(screen.queryByText(/subjects/)).not.toBeInTheDocument();
  });

  it("renders nothing when subjects is empty", () => {
    renderWithProviders(<SubjectsCount subjects={[]} />);
    expect(screen.queryByText(/subjects/)).not.toBeInTheDocument();
  });

  it("displays subject count", () => {
    renderWithProviders(<SubjectsCount subjects={["A", "B", "C"]} />);
    expect(screen.getByText("3 subjects")).toBeInTheDocument();
  });

  it("parses JSON string input", () => {
    const json = JSON.stringify(["A", "B"]);
    renderWithProviders(<SubjectsCount subjects={json} />);
    expect(screen.getByText("2 subjects")).toBeInTheDocument();
  });

  it("parses comma-separated string input", () => {
    renderWithProviders(<SubjectsCount subjects="A, B, C, D" />);
    expect(screen.getByText("4 subjects")).toBeInTheDocument();
  });
});
