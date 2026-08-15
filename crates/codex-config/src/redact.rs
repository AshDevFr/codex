//! Renders a resolved [`Config`] with secrets removed.
//!
//! `codex config check` prints the config it resolved so an operator can see
//! what the process actually believes. That output is read from `docker logs`
//! and from Kubernetes initContainer logs, both of which are visible to anyone
//! with access to the workload, so the database password and the JWT signing
//! secret must not appear in it.
//!
//! Redaction keys off field names rather than a list of paths, so a secret
//! added under any section is covered without touching this module. It
//! deliberately errs towards hiding too much.

use crate::types::Config;
use serde_json::Value;

/// Stand-in for a secret that is set. Seeing this means the value is present.
pub const REDACTED: &str = "<redacted>";
/// Stand-in for a secret that is empty. Distinguishing the two matters: "set
/// but hidden" and "never configured" produce very different bugs, and an
/// operator debugging a failed SMTP login needs to tell them apart.
pub const UNSET: &str = "<unset>";

/// Field-name fragments that mark a value as secret.
const SECRET_MARKERS: &[&str] = &["password", "secret", "_token", "_key"];

/// Fields whose name matches [`SECRET_MARKERS`] but which hold no secret.
///
/// `client_secret_env` names the environment variable a secret is read from.
/// The name is not sensitive, and seeing it is precisely how an operator
/// debugs the indirection, so hiding it would remove the diagnostic value of
/// printing the config at all.
const NOT_SECRET: &[&str] = &["client_secret_env"];

/// Is a field with this name, holding a string, a secret?
fn is_secret_field(name: &str) -> bool {
    if NOT_SECRET.contains(&name) {
        return false;
    }
    SECRET_MARKERS.iter().any(|marker| name.contains(marker))
}

/// The config as a JSON value with every secret string replaced.
///
/// Only string-valued fields are considered. That is what keeps
/// `refresh_token_enabled` (a bool) and `verification_token_expiry_hours` (a
/// number) legible while still catching every field that can actually carry a
/// credential.
pub fn redacted_value(config: &Config) -> Value {
    let mut value = serde_json::to_value(config)
        .expect("Config is a plain serde struct and must always serialize");
    redact_in_place(&mut value, "");
    value
}

/// The config as YAML with every secret replaced.
pub fn redacted_yaml(config: &Config) -> anyhow::Result<String> {
    Ok(serde_yaml::to_string(&redacted_value(config))?)
}

fn redact_in_place(value: &mut Value, field_name: &str) {
    match value {
        Value::Object(fields) => {
            for (name, child) in fields.iter_mut() {
                let name = name.clone();
                redact_in_place(child, &name);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_in_place(item, field_name);
            }
        }
        Value::String(text) if is_secret_field(field_name) => {
            *text = if text.is_empty() { UNSET } else { REDACTED }.to_string();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OidcProviderConfig, PostgresConfig};
    use std::collections::HashMap;

    fn provider_with_secret() -> OidcProviderConfig {
        OidcProviderConfig {
            display_name: "Authentik".to_string(),
            issuer_url: "https://idp.example.com".to_string(),
            client_id: "codex".to_string(),
            client_secret: Some("super-secret-value".to_string()),
            client_secret_env: Some("CODEX_OIDC_AUTHENTIK_SECRET".to_string()),
            scopes: Vec::new(),
            role_mapping: HashMap::new(),
            groups_claim: "groups".to_string(),
            username_claim: "preferred_username".to_string(),
            email_claim: "email".to_string(),
            accepted_audiences: Vec::new(),
        }
    }

    fn config_with_secrets() -> Config {
        let mut config = Config::default();
        config.auth.jwt_secret = "jwt-signing-secret".to_string();
        config.email.smtp_password = "smtp-password".to_string();
        config.database.postgres = Some(PostgresConfig {
            password: "pg-password".to_string(),
            ..PostgresConfig::default()
        });
        config
            .auth
            .oidc
            .providers
            .insert("authentik".to_string(), provider_with_secret());
        config
    }

    #[test]
    fn no_secret_value_survives_rendering() {
        let rendered = redacted_yaml(&config_with_secrets()).unwrap();
        for secret in [
            "jwt-signing-secret",
            "smtp-password",
            "pg-password",
            "super-secret-value",
        ] {
            assert!(
                !rendered.contains(secret),
                "`{secret}` leaked into the rendered config:\n{rendered}"
            );
        }
    }

    #[test]
    fn set_and_unset_secrets_render_differently() {
        let mut config = Config::default();
        config.auth.jwt_secret = "something".to_string();
        config.email.smtp_password = String::new();

        let value = redacted_value(&config);
        assert_eq!(value["auth"]["jwt_secret"], Value::String(REDACTED.into()));
        assert_eq!(
            value["email"]["smtp_password"],
            Value::String(UNSET.into()),
            "an empty secret must be distinguishable from a hidden one"
        );
    }

    /// The variable *name* a secret is read from is a diagnostic, not a
    /// secret. Hiding it defeats the purpose of printing the config.
    #[test]
    fn client_secret_env_is_left_readable() {
        let value = redacted_value(&config_with_secrets());
        let provider = &value["auth"]["oidc"]["providers"]["authentik"];
        assert_eq!(
            provider["client_secret_env"],
            Value::String("CODEX_OIDC_AUTHENTIK_SECRET".into())
        );
        assert_eq!(provider["client_secret"], Value::String(REDACTED.into()));
    }

    /// Only strings are redacted, so numeric and boolean fields whose names
    /// contain a secret marker stay legible.
    #[test]
    fn non_string_fields_are_untouched() {
        let value = redacted_value(&Config::default());
        assert_eq!(value["auth"]["refresh_token_enabled"], Value::Bool(true));
        assert!(value["auth"]["refresh_token_expiry_days"].is_number());
        assert!(value["email"]["verification_token_expiry_hours"].is_number());
    }

    #[test]
    fn ordinary_settings_are_untouched() {
        let value = redacted_value(&Config::default());
        assert_eq!(
            value["application"]["host"],
            Value::String("0.0.0.0".into())
        );
        assert_eq!(value["api"]["base_path"], Value::String("/api/v1".into()));
    }

    #[test]
    fn secret_detection_covers_the_known_fields() {
        for name in [
            "password",
            "smtp_password",
            "jwt_secret",
            "client_secret",
            "encryption_key",
            "api_key",
        ] {
            assert!(
                is_secret_field(name),
                "`{name}` should be treated as secret"
            );
        }
        for name in [
            "client_secret_env",
            "host",
            "base_path",
            "issuer_url",
            "username",
        ] {
            assert!(!is_secret_field(name), "`{name}` should stay readable");
        }
    }
}
