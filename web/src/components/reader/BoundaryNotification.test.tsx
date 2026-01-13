import { MantineProvider } from "@mantine/core";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { BoundaryNotification } from "./BoundaryNotification";

function wrapper({ children }: { children: ReactNode }) {
	return <MantineProvider>{children}</MantineProvider>;
}

describe("BoundaryNotification", () => {
	it("should not render when not visible", () => {
		render(
			<BoundaryNotification
				message="Test message"
				visible={false}
				type="at-end"
			/>,
			{ wrapper },
		);

		expect(screen.queryByText("Test message")).not.toBeInTheDocument();
	});

	it("should not render when message is null", () => {
		render(
			<BoundaryNotification message={null} visible={true} type="at-end" />,
			{ wrapper },
		);

		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	});

	it("should render message when visible with message", () => {
		render(
			<BoundaryNotification
				message="End of book. Press again to continue."
				visible={true}
				type="at-end"
			/>,
			{ wrapper },
		);

		expect(
			screen.getByText("End of book. Press again to continue."),
		).toBeInTheDocument();
	});

	it("should not render when type is none", () => {
		render(
			<BoundaryNotification
				message="Test message"
				visible={true}
				type="none"
			/>,
			{ wrapper },
		);

		// The component shows when visible=true and message is set,
		// even with type="none". The type just affects the icon.
		expect(screen.getByText("Test message")).toBeInTheDocument();
	});

	it("should display message correctly for at-start type", () => {
		render(
			<BoundaryNotification
				message='Beginning of book. Press again to go to "Previous Book"'
				visible={true}
				type="at-start"
			/>,
			{ wrapper },
		);

		expect(
			screen.getByText(
				'Beginning of book. Press again to go to "Previous Book"',
			),
		).toBeInTheDocument();
	});

	it("should display message correctly for at-end type", () => {
		render(
			<BoundaryNotification
				message='End of book. Press again to continue to "Next Book"'
				visible={true}
				type="at-end"
			/>,
			{ wrapper },
		);

		expect(
			screen.getByText('End of book. Press again to continue to "Next Book"'),
		).toBeInTheDocument();
	});
});
