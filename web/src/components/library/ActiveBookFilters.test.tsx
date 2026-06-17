import { MantineProvider } from "@mantine/core";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { ActiveBookFilters } from "./ActiveBookFilters";

// Wrapper component that provides required context
function TestWrapper({
  children,
  initialRoute = "/",
}: {
  children: React.ReactNode;
  initialRoute?: string;
}) {
  return (
    <MemoryRouter initialEntries={[initialRoute]}>
      <MantineProvider>{children}</MantineProvider>
    </MemoryRouter>
  );
}

describe("ActiveBookFilters", () => {
  it("should not render when no filters are active", () => {
    render(
      <TestWrapper>
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.queryByText("Filters:")).not.toBeInTheDocument();
  });

  it("should render genre include filter chip", () => {
    // Book genre param key is bgf
    render(
      <TestWrapper initialRoute="/?bgf=any:Action">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText("Filters:")).toBeInTheDocument();
    expect(screen.getByText(/Genre: Action/)).toBeInTheDocument();
  });

  it("should render hasError filter chip", () => {
    render(
      <TestWrapper initialRoute="/?bef=include">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText("Has Error: Yes")).toBeInTheDocument();
  });

  it("should render bookType include filter chip", () => {
    // Book type param key is bbt
    render(
      <TestWrapper initialRoute="/?bbt=any:manga">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText("Filters:")).toBeInTheDocument();
    expect(screen.getByText(/Type: manga/)).toBeInTheDocument();
  });

  it("should render bookType exclude filter chip with NOT prefix", () => {
    render(
      <TestWrapper initialRoute="/?bbt=any::-manga">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText(/NOT Type: manga/)).toBeInTheDocument();
  });

  it("should render multiple bookType chips", () => {
    render(
      <TestWrapper initialRoute="/?bbt=any:manga,comic">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText(/Type: manga/)).toBeInTheDocument();
    expect(screen.getByText(/Type: comic/)).toBeInTheDocument();
  });

  it("should have remove button on bookType chip", () => {
    render(
      <TestWrapper initialRoute="/?bbt=any:manga">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    const removeButton = screen.getByRole("button", {
      name: /Remove manga filter/i,
    });
    expect(removeButton).toBeInTheDocument();
  });

  it("should render inReadList include filter chip", () => {
    // Book in-read-list param key is brlf
    render(
      <TestWrapper initialRoute="/?brlf=include">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText("Filters:")).toBeInTheDocument();
    expect(screen.getByText("In Read List")).toBeInTheDocument();
  });

  it("should render inReadList exclude filter chip with NOT prefix", () => {
    render(
      <TestWrapper initialRoute="/?brlf=exclude">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    expect(screen.getByText(/NOT In Read List/)).toBeInTheDocument();
  });

  it("should have remove button on inReadList chip", () => {
    render(
      <TestWrapper initialRoute="/?brlf=include">
        <ActiveBookFilters />
      </TestWrapper>,
    );

    const removeButton = screen.getByRole("button", {
      name: /Remove in read list filter/i,
    });
    expect(removeButton).toBeInTheDocument();
  });
});
