//! Komga-compatible user DTOs
//!
//! These DTOs match the exact structure Komic expects from Komga's `/api/v1/users/me` endpoint.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Komga user DTO
///
/// Response for GET /api/v1/users/me
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaUserDto {
    /// User unique identifier (UUID as string)
    pub id: String,
    /// User email address
    pub email: String,
    /// User roles (e.g., ["ADMIN"], ["USER"])
    pub roles: Vec<String>,
    /// Shared libraries access - list of library IDs user can access
    /// Empty means access to all libraries
    #[serde(default)]
    pub shared_libraries_ids: Vec<String>,
    /// Whether all libraries are shared with this user
    #[serde(default = "default_true")]
    pub shared_all_libraries: bool,
    /// Whether user can share content
    #[serde(default)]
    pub labels_allow: Vec<String>,
    /// Labels to exclude from sharing
    #[serde(default)]
    pub labels_exclude: Vec<String>,
    /// User's content restrictions
    #[serde(default)]
    pub content_restrictions: KomgaContentRestrictionsDto,
}

fn default_true() -> bool {
    true
}

impl Default for KomgaUserDto {
    fn default() -> Self {
        Self {
            id: String::new(),
            email: String::new(),
            roles: vec!["USER".to_string()],
            shared_libraries_ids: Vec::new(),
            shared_all_libraries: true,
            labels_allow: Vec::new(),
            labels_exclude: Vec::new(),
            content_restrictions: KomgaContentRestrictionsDto::default(),
        }
    }
}

impl KomgaUserDto {
    /// Create from Codex user data
    pub fn from_codex(id: uuid::Uuid, email: &str, role: &str) -> Self {
        let roles = match role.to_lowercase().as_str() {
            "admin" => vec!["ADMIN".to_string()],
            "maintainer" => vec!["USER".to_string(), "FILE_DOWNLOAD".to_string()],
            _ => vec!["USER".to_string()],
        };

        Self {
            id: id.to_string(),
            email: email.to_string(),
            roles,
            shared_all_libraries: true,
            ..Default::default()
        }
    }
}

/// Komga content restrictions DTO
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaContentRestrictionsDto {
    /// Age restriction (null means no restriction)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_restriction: Option<KomgaAgeRestrictionDto>,
    /// Labels restriction
    #[serde(default)]
    pub labels_allow: Vec<String>,
    /// Labels to exclude
    #[serde(default)]
    pub labels_exclude: Vec<String>,
}

/// Komga age restriction DTO
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KomgaAgeRestrictionDto {
    /// Age limit
    pub age: i32,
    /// Restriction type (ALLOW_ONLY, EXCLUDE)
    pub restriction: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_user_dto_serialization() {
        let user = KomgaUserDto {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            email: "test@example.com".to_string(),
            roles: vec!["ADMIN".to_string()],
            shared_libraries_ids: vec![],
            shared_all_libraries: true,
            labels_allow: vec![],
            labels_exclude: vec![],
            content_restrictions: KomgaContentRestrictionsDto::default(),
        };

        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("\"id\":\"550e8400-e29b-41d4-a716-446655440000\""));
        assert!(json.contains("\"email\":\"test@example.com\""));
        assert!(json.contains("\"roles\":[\"ADMIN\"]"));
        assert!(json.contains("\"sharedAllLibraries\":true"));
    }

    #[test]
    fn test_user_dto_camel_case() {
        let user = KomgaUserDto::default();
        let json = serde_json::to_string(&user).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"sharedLibrariesIds\""));
        assert!(json.contains("\"sharedAllLibraries\""));
        assert!(json.contains("\"labelsAllow\""));
        assert!(json.contains("\"labelsExclude\""));
        assert!(json.contains("\"contentRestrictions\""));
    }

    #[test]
    fn test_user_dto_from_codex_admin() {
        let id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let user = KomgaUserDto::from_codex(id, "admin@example.com", "admin");

        assert_eq!(user.id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(user.email, "admin@example.com");
        assert!(user.roles.contains(&"ADMIN".to_string()));
        assert!(user.shared_all_libraries);
    }

    #[test]
    fn test_user_dto_from_codex_maintainer() {
        let id = uuid::Uuid::new_v4();
        let user = KomgaUserDto::from_codex(id, "maintainer@example.com", "maintainer");

        assert!(user.roles.contains(&"USER".to_string()));
        assert!(user.roles.contains(&"FILE_DOWNLOAD".to_string()));
    }

    #[test]
    fn test_user_dto_from_codex_reader() {
        let id = uuid::Uuid::new_v4();
        let user = KomgaUserDto::from_codex(id, "reader@example.com", "reader");

        assert_eq!(user.roles, vec!["USER".to_string()]);
    }

    #[test]
    fn test_user_dto_deserialization() {
        let json = r#"{
            "id": "test-id",
            "email": "test@test.com",
            "roles": ["USER"],
            "sharedLibrariesIds": [],
            "sharedAllLibraries": true,
            "labelsAllow": [],
            "labelsExclude": [],
            "contentRestrictions": {
                "labelsAllow": [],
                "labelsExclude": []
            }
        }"#;

        let user: KomgaUserDto = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, "test-id");
        assert_eq!(user.email, "test@test.com");
        assert!(user.shared_all_libraries);
    }

    #[test]
    fn test_content_restrictions_dto() {
        let restrictions = KomgaContentRestrictionsDto {
            age_restriction: Some(KomgaAgeRestrictionDto {
                age: 18,
                restriction: "ALLOW_ONLY".to_string(),
            }),
            labels_allow: vec!["safe".to_string()],
            labels_exclude: vec!["nsfw".to_string()],
        };

        let json = serde_json::to_string(&restrictions).unwrap();
        assert!(json.contains("\"ageRestriction\""));
        assert!(json.contains("\"age\":18"));
        assert!(json.contains("\"restriction\":\"ALLOW_ONLY\""));
    }

    #[test]
    fn test_content_restrictions_without_age() {
        let restrictions = KomgaContentRestrictionsDto::default();
        let json = serde_json::to_string(&restrictions).unwrap();

        // ageRestriction should be skipped when None
        assert!(!json.contains("ageRestriction"));
    }
}
