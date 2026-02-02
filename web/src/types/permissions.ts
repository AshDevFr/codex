/**
 * Permission definitions matching the backend Permission enum.
 * These are the individual permissions that can be granted to users and API tokens.
 */
export const PERMISSIONS = {
  // Libraries
  LIBRARIES_READ: "libraries-read",
  LIBRARIES_WRITE: "libraries-write",
  LIBRARIES_DELETE: "libraries-delete",

  // Series
  SERIES_READ: "series-read",
  SERIES_WRITE: "series-write",
  SERIES_DELETE: "series-delete",

  // Books
  BOOKS_READ: "books-read",
  BOOKS_WRITE: "books-write",
  BOOKS_DELETE: "books-delete",

  // Pages (image serving)
  PAGES_READ: "pages-read",

  // Users (admin only)
  USERS_READ: "users-read",
  USERS_WRITE: "users-write",
  USERS_DELETE: "users-delete",

  // API Keys
  API_KEYS_READ: "api-keys-read",
  API_KEYS_WRITE: "api-keys-write",
  API_KEYS_DELETE: "api-keys-delete",

  // Tasks
  TASKS_READ: "tasks-read",
  TASKS_WRITE: "tasks-write",

  // System
  SYSTEM_HEALTH: "system-health",
  SYSTEM_ADMIN: "system-admin",
} as const;

export type Permission = (typeof PERMISSIONS)[keyof typeof PERMISSIONS];

/**
 * All available permissions as an array
 */
export const ALL_PERMISSIONS: Permission[] = Object.values(PERMISSIONS);

/**
 * Permission groups for UI display - organized by category
 */
export interface PermissionGroup {
  label: string;
  description: string;
  permissions: {
    value: Permission;
    label: string;
    description: string;
  }[];
}

export const PERMISSION_GROUPS: PermissionGroup[] = [
  {
    label: "Content Access",
    description: "Read access to libraries, series, books, and pages",
    permissions: [
      {
        value: PERMISSIONS.LIBRARIES_READ,
        label: "Read Libraries",
        description: "View library list and details",
      },
      {
        value: PERMISSIONS.SERIES_READ,
        label: "Read Series",
        description: "View series list and details",
      },
      {
        value: PERMISSIONS.BOOKS_READ,
        label: "Read Books",
        description: "View book list and details",
      },
      {
        value: PERMISSIONS.PAGES_READ,
        label: "Read Pages",
        description: "View book pages/images",
      },
    ],
  },
  {
    label: "Content Management",
    description: "Create, modify, and delete content",
    permissions: [
      {
        value: PERMISSIONS.LIBRARIES_WRITE,
        label: "Write Libraries",
        description: "Create and modify libraries",
      },
      {
        value: PERMISSIONS.LIBRARIES_DELETE,
        label: "Delete Libraries",
        description: "Delete libraries (admin only)",
      },
      {
        value: PERMISSIONS.SERIES_WRITE,
        label: "Write Series",
        description: "Modify series metadata",
      },
      {
        value: PERMISSIONS.SERIES_DELETE,
        label: "Delete Series",
        description: "Delete series",
      },
      {
        value: PERMISSIONS.BOOKS_WRITE,
        label: "Write Books",
        description: "Modify book metadata",
      },
      {
        value: PERMISSIONS.BOOKS_DELETE,
        label: "Delete Books",
        description: "Delete books",
      },
    ],
  },
  {
    label: "API Keys",
    description: "Manage your own API keys",
    permissions: [
      {
        value: PERMISSIONS.API_KEYS_READ,
        label: "Read API Keys",
        description: "View your API keys",
      },
      {
        value: PERMISSIONS.API_KEYS_WRITE,
        label: "Write API Keys",
        description: "Create and modify API keys",
      },
      {
        value: PERMISSIONS.API_KEYS_DELETE,
        label: "Delete API Keys",
        description: "Delete API keys",
      },
    ],
  },
  {
    label: "Tasks",
    description: "View and manage background tasks",
    permissions: [
      {
        value: PERMISSIONS.TASKS_READ,
        label: "Read Tasks",
        description: "View task queue and status",
      },
      {
        value: PERMISSIONS.TASKS_WRITE,
        label: "Write Tasks",
        description: "Trigger scans and manage tasks",
      },
    ],
  },
  {
    label: "System",
    description: "System health and administration",
    permissions: [
      {
        value: PERMISSIONS.SYSTEM_HEALTH,
        label: "System Health",
        description: "View system health status",
      },
      {
        value: PERMISSIONS.SYSTEM_ADMIN,
        label: "System Admin",
        description: "Full system administration (admin only)",
      },
    ],
  },
  {
    label: "User Management",
    description: "Manage users (admin only)",
    permissions: [
      {
        value: PERMISSIONS.USERS_READ,
        label: "Read Users",
        description: "View user list and details",
      },
      {
        value: PERMISSIONS.USERS_WRITE,
        label: "Write Users",
        description: "Create and modify users",
      },
      {
        value: PERMISSIONS.USERS_DELETE,
        label: "Delete Users",
        description: "Delete users",
      },
    ],
  },
];

/**
 * Role-based permission presets matching the backend
 */
export const ROLE_PERMISSIONS: Record<string, Permission[]> = {
  reader: [
    PERMISSIONS.LIBRARIES_READ,
    PERMISSIONS.SERIES_READ,
    PERMISSIONS.BOOKS_READ,
    PERMISSIONS.PAGES_READ,
    PERMISSIONS.API_KEYS_READ,
    PERMISSIONS.API_KEYS_WRITE,
    PERMISSIONS.API_KEYS_DELETE,
    PERMISSIONS.SYSTEM_HEALTH,
  ],
  maintainer: [
    // Reader permissions
    PERMISSIONS.LIBRARIES_READ,
    PERMISSIONS.SERIES_READ,
    PERMISSIONS.BOOKS_READ,
    PERMISSIONS.PAGES_READ,
    PERMISSIONS.API_KEYS_READ,
    PERMISSIONS.API_KEYS_WRITE,
    PERMISSIONS.API_KEYS_DELETE,
    PERMISSIONS.SYSTEM_HEALTH,
    // Additional maintainer permissions
    PERMISSIONS.LIBRARIES_WRITE,
    PERMISSIONS.SERIES_WRITE,
    PERMISSIONS.SERIES_DELETE,
    PERMISSIONS.BOOKS_WRITE,
    PERMISSIONS.BOOKS_DELETE,
    PERMISSIONS.TASKS_READ,
    PERMISSIONS.TASKS_WRITE,
  ],
  admin: ALL_PERMISSIONS,
};

/**
 * Common permission presets for quick selection in API key creation
 */
export const PERMISSION_PRESETS = [
  {
    value: "full",
    label: "Full Access",
    description: "All permissions your role allows",
    getPermissions: (role: string) => ROLE_PERMISSIONS[role] || [],
  },
  {
    value: "read-only",
    label: "Read Only",
    description: "View content only, no modifications",
    getPermissions: () => [
      PERMISSIONS.LIBRARIES_READ,
      PERMISSIONS.SERIES_READ,
      PERMISSIONS.BOOKS_READ,
      PERMISSIONS.PAGES_READ,
      PERMISSIONS.SYSTEM_HEALTH,
    ],
  },
  {
    value: "opds",
    label: "OPDS/Reader Apps",
    description: "For e-reader apps and OPDS clients",
    getPermissions: () => [
      PERMISSIONS.LIBRARIES_READ,
      PERMISSIONS.SERIES_READ,
      PERMISSIONS.BOOKS_READ,
      PERMISSIONS.PAGES_READ,
    ],
  },
  {
    value: "custom",
    label: "Custom",
    description: "Select specific permissions",
    getPermissions: () => [],
  },
] as const;

export type PermissionPreset = (typeof PERMISSION_PRESETS)[number]["value"];

/**
 * Get human-readable label for a permission
 */
export function getPermissionLabel(permission: Permission): string {
  for (const group of PERMISSION_GROUPS) {
    const found = group.permissions.find((p) => p.value === permission);
    if (found) return found.label;
  }
  return permission;
}

/**
 * Check if user's role has a specific permission
 */
export function roleHasPermission(
  role: string,
  permission: Permission,
): boolean {
  const rolePerms = ROLE_PERMISSIONS[role];
  return rolePerms ? rolePerms.includes(permission) : false;
}

/**
 * Get permissions available to a role
 */
export function getPermissionsForRole(role: string): Permission[] {
  return ROLE_PERMISSIONS[role] || [];
}

/**
 * Parse permissions from API response (handles JSON value)
 */
export function parsePermissions(permissions: unknown): Permission[] {
  if (Array.isArray(permissions)) {
    return permissions.filter((p): p is Permission =>
      ALL_PERMISSIONS.includes(p as Permission),
    );
  }
  return [];
}
