import type { ReactNode } from "react";
import { Navigate } from "react-router-dom";
import { usePermissions } from "@/hooks/usePermissions";

/**
 * Renders its page only for admins and sends anyone else home, for pages
 * whose API is admin-only, so a non-admin who types the URL does not land on
 * a page that can only fail.
 */
export function AdminRoute({ children }: { children: ReactNode }) {
  const { isAdmin } = usePermissions();
  if (!isAdmin) {
    return <Navigate to="/" replace />;
  }
  return <>{children}</>;
}
