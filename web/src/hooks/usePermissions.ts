import { useAuthStore } from "@/store/authStore";
import {
  type Permission,
  ROLE_PERMISSIONS,
  roleHasPermission,
} from "@/types/permissions";

/**
 * Hook for checking user permissions based on their role and custom permissions.
 *
 * Effective permissions = role permissions ∪ custom permissions
 */
export function usePermissions() {
  const user = useAuthStore((state) => state.user);

  /**
   * Get the user's effective permissions (role + custom)
   */
  const getEffectivePermissions = (): Permission[] => {
    if (!user) return [];

    const rolePerms = ROLE_PERMISSIONS[user.role] || [];
    const customPerms = (user.permissions || []) as Permission[];

    // Union of role and custom permissions
    const permSet = new Set([...rolePerms, ...customPerms]);
    return Array.from(permSet);
  };

  /**
   * Check if the user has a specific permission
   */
  const hasPermission = (permission: Permission): boolean => {
    if (!user) return false;

    // Check role permissions first
    if (roleHasPermission(user.role, permission)) {
      return true;
    }

    // Check custom permissions
    const customPerms = (user.permissions || []) as Permission[];
    return customPerms.includes(permission);
  };

  /**
   * Check if the user has any of the specified permissions
   */
  const hasAnyPermission = (permissions: Permission[]): boolean => {
    return permissions.some((p) => hasPermission(p));
  };

  /**
   * Check if the user has all of the specified permissions
   */
  const hasAllPermissions = (permissions: Permission[]): boolean => {
    return permissions.every((p) => hasPermission(p));
  };

  /**
   * Check if the user is an admin
   */
  const isAdmin = user?.role === "admin";

  /**
   * Check if the user is at least a maintainer (maintainer or admin)
   */
  const isMaintainer = user?.role === "maintainer" || user?.role === "admin";

  return {
    user,
    isAdmin,
    isMaintainer,
    hasPermission,
    hasAnyPermission,
    hasAllPermissions,
    getEffectivePermissions,
  };
}
