import { ActionIcon, Text } from "@mantine/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, within } from "@/test/utils";
import { ResponsiveTable, type ResponsiveTableColumn } from "./ResponsiveTable";

interface Row {
  id: string;
  name: string;
  email: string;
  role: string;
}

const ROWS: Row[] = [
  { id: "1", name: "Alice", email: "alice@example.com", role: "admin" },
  { id: "2", name: "Bob", email: "bob@example.com", role: "reader" },
];

const COLUMNS: ResponsiveTableColumn<Row>[] = [
  {
    key: "name",
    header: "Name",
    accessor: (row) => <Text fw={500}>{row.name}</Text>,
    mobilePrimary: true,
  },
  { key: "email", header: "Email", accessor: (row) => row.email },
  { key: "role", header: "Role", accessor: (row) => row.role },
];

function forceMobileViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: query.includes("max-width"),
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

function forceDesktopViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

describe("ResponsiveTable", () => {
  beforeEach(() => {
    forceDesktopViewport();
  });

  describe("desktop layout", () => {
    it("renders a Mantine Table with headers", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          data-testid="rt"
        />,
      );

      const table = screen.getByRole("table");
      expect(table).toBeInTheDocument();
      const headers = within(table).getAllByRole("columnheader");
      expect(headers).toHaveLength(3);
      expect(headers[0]).toHaveTextContent("Name");
      expect(headers[1]).toHaveTextContent("Email");
      expect(headers[2]).toHaveTextContent("Role");
    });

    it("renders cell content via the column accessor", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
        />,
      );

      expect(screen.getByText("Alice")).toBeInTheDocument();
      expect(screen.getByText("bob@example.com")).toBeInTheDocument();
      expect(screen.getByText("admin")).toBeInTheDocument();
    });

    it("renders rowActions as the last column with a default header", () => {
      const onDelete = vi.fn();
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          rowActions={(row) => (
            <button type="button" onClick={() => onDelete(row.id)}>
              Delete {row.name}
            </button>
          )}
        />,
      );

      expect(screen.getByText("Actions")).toBeInTheDocument();
      expect(screen.getByText("Delete Alice")).toBeInTheDocument();
      expect(screen.getByText("Delete Bob")).toBeInTheDocument();
    });

    it("respects a custom rowActionsHeader", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          rowActions={() => <span>x</span>}
          rowActionsHeader=""
        />,
      );

      expect(screen.queryByText("Actions")).not.toBeInTheDocument();
    });

    it("skips columns with hideOnDesktop", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={[
            ...COLUMNS,
            {
              key: "mobile-only",
              header: "Mobile only",
              accessor: () => "mobile-only-value",
              hideOnDesktop: true,
            },
          ]}
          getRowKey={(row) => row.id}
        />,
      );

      expect(screen.queryByText("Mobile only")).not.toBeInTheDocument();
      expect(screen.queryByText("mobile-only-value")).not.toBeInTheDocument();
    });

    it("renders emptyState in place of the table when data is empty", () => {
      renderWithProviders(
        <ResponsiveTable
          data={[]}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          emptyState={<div>No users</div>}
        />,
      );

      expect(screen.getByText("No users")).toBeInTheDocument();
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
    });

    it("renders an empty table (no emptyState) when data is empty and no fallback is given", () => {
      renderWithProviders(
        <ResponsiveTable
          data={[]}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
        />,
      );

      const table = screen.getByRole("table");
      expect(table).toBeInTheDocument();
      // Header row only; no body rows.
      expect(within(table).getAllByRole("row")).toHaveLength(1);
    });
  });

  describe("mobile layout", () => {
    beforeEach(() => {
      forceMobileViewport();
    });

    it("renders a stack of Cards instead of a Table", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          data-testid="rt"
        />,
      );

      expect(screen.queryByRole("table")).not.toBeInTheDocument();
      expect(screen.getByTestId("rt")).toBeInTheDocument();
    });

    it("renders the primary column without a label on the card", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
        />,
      );

      // Primary column ("name") renders the value; the literal header text
      // should not also appear because mobilePrimary suppresses the label.
      expect(screen.getByText("Alice")).toBeInTheDocument();
      expect(screen.queryByText("Name")).not.toBeInTheDocument();
    });

    it("renders non-primary columns as label/value pairs", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
        />,
      );

      // Both columns + both rows = labels appear once per row.
      expect(screen.getAllByText("Email")).toHaveLength(2);
      expect(screen.getAllByText("Role")).toHaveLength(2);
      expect(screen.getByText("alice@example.com")).toBeInTheDocument();
      expect(screen.getByText("reader")).toBeInTheDocument();
    });

    it("uses mobileLabel when provided instead of header", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={[
            ...COLUMNS,
            {
              key: "label-override",
              header: "Long Desktop Header",
              mobileLabel: "Short",
              accessor: () => "value",
            },
          ]}
          getRowKey={(row) => row.id}
        />,
      );

      expect(screen.queryByText("Long Desktop Header")).not.toBeInTheDocument();
      expect(screen.getAllByText("Short")).toHaveLength(2);
    });

    it("skips columns with hideOnMobile", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={[
            ...COLUMNS,
            {
              key: "internal",
              header: "Internal",
              accessor: () => "should-not-render",
              hideOnMobile: true,
            },
          ]}
          getRowKey={(row) => row.id}
        />,
      );

      expect(screen.queryByText("Internal")).not.toBeInTheDocument();
      expect(screen.queryByText("should-not-render")).not.toBeInTheDocument();
    });

    it("renders rowActions inside each card", () => {
      const onDelete = vi.fn();
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          rowActions={(row) => (
            <ActionIcon
              aria-label={`delete ${row.name}`}
              onClick={() => onDelete(row.id)}
            >
              x
            </ActionIcon>
          )}
        />,
      );

      expect(screen.getByLabelText("delete Alice")).toBeInTheDocument();
      expect(screen.getByLabelText("delete Bob")).toBeInTheDocument();
    });

    it("uses renderMobileCard to override the card body", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          renderMobileCard={(row) => (
            <div data-testid={`custom-${row.id}`}>{row.name} custom</div>
          )}
        />,
      );

      expect(screen.getByTestId("custom-1")).toHaveTextContent("Alice custom");
      expect(screen.getByTestId("custom-2")).toHaveTextContent("Bob custom");
      // The default label/value rows must not appear.
      expect(screen.queryByText("Email")).not.toBeInTheDocument();
    });

    it("keeps rowActions footer when renderMobileCard is used", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          renderMobileCard={(row) => <div>{row.name}</div>}
          rowActions={(row) => <span>action-{row.id}</span>}
        />,
      );

      expect(screen.getByText("action-1")).toBeInTheDocument();
      expect(screen.getByText("action-2")).toBeInTheDocument();
    });

    it("renders emptyState in place of cards when data is empty", () => {
      renderWithProviders(
        <ResponsiveTable
          data={[]}
          columns={COLUMNS}
          getRowKey={(row) => row.id}
          emptyState={<div>Nothing here</div>}
        />,
      );

      expect(screen.getByText("Nothing here")).toBeInTheDocument();
    });

    it("renders mobileFullWidth columns as label + full-width value block", () => {
      renderWithProviders(
        <ResponsiveTable
          data={ROWS}
          columns={[
            {
              key: "long",
              header: "Description",
              accessor: () => (
                <span data-testid="long-value">
                  A very long string that should occupy the full card width.
                </span>
              ),
              mobileFullWidth: true,
            },
          ]}
          getRowKey={(row) => row.id}
        />,
      );

      expect(screen.getAllByText("Description")).toHaveLength(2);
      expect(screen.getAllByTestId("long-value")).toHaveLength(2);
    });
  });
});
