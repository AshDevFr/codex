import type { ReactNode } from "react";
import { Navigate } from "react-router-dom";
import { usePermissions } from "@/hooks/usePermissions";
import type { Permission } from "@/types/permissions";

interface RequirePermissionProps {
  /** The permission the page's API requires */
  permission: Permission;
  children: ReactNode;
}

/**
 * Renders its page only for users holding `permission` and sends anyone else
 * home, so a user who types the URL does not land on a page whose requests
 * can only fail.
 */
export function RequirePermission({
  permission,
  children,
}: RequirePermissionProps) {
  const { hasPermission } = usePermissions();
  if (!hasPermission(permission)) {
    return <Navigate to="/" replace />;
  }
  return <>{children}</>;
}
