import { forwardRef } from "react";
import { Link, type LinkProps } from "react-router-dom";

export interface AppLinkProps extends LinkProps {
	/** Stop click propagation (use when inside clickable parents like cards) */
	stopPropagation?: boolean;
}

/**
 * A wrapper around React Router's Link that provides:
 * - Proper anchor tag semantics (shows URL on hover, right-click menu, CMD+Click)
 * - Optional click propagation stopping for use inside clickable parents
 * - Forward ref support for Mantine component integration
 */
export const AppLink = forwardRef<HTMLAnchorElement, AppLinkProps>(
	({ stopPropagation, onClick, style, children, ...props }, ref) => {
		const handleClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
			if (stopPropagation) {
				e.stopPropagation();
			}
			onClick?.(e);
		};

		return (
			<Link
				ref={ref}
				onClick={handleClick}
				style={{
					color: "inherit",
					textDecoration: "none",
					...style,
				}}
				{...props}
			>
				{children}
			</Link>
		);
	},
);

AppLink.displayName = "AppLink";
