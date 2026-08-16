//! Deserializers that accept the input shapes operators already use.
//!
//! Environment variables are strings, but the config tree holds bools, lists
//! and maps. The v1 override layer hand-parsed each of those at the point of
//! use; plain serde would now reject the same input. These deserializers keep
//! the accepted spellings working through the figment env provider.
//!
//! One deliberate behaviour change: v1 swallowed parse failures
//! (`if let Ok(v) = s.parse()`), so `CODEX_KOMGA_API_ENABLED=ture` silently
//! meant `false`. These reject unrecognized input instead. A typo in a
//! deployment's environment should stop the process, not quietly flip a
//! setting.

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;

/// A bool from a bool, a number, or a string.
///
/// figment parses `true` into a bool and `1` into a number before serde sees
/// them, so a field that must accept both has to handle both.
pub fn truthy_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(TruthyBool)
}

struct TruthyBool;

impl<'de> Visitor<'de> for TruthyBool {
    type Value = bool;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a boolean: true/false, 1/0, yes/no or on/off")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<bool, E> {
        Ok(value)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<bool, E> {
        match value {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(E::custom(format!("expected 0 or 1, found {other}"))),
        }
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<bool, E> {
        match value {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(E::custom(format!("expected 0 or 1, found {other}"))),
        }
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<bool, E> {
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            other => Err(E::custom(format!(
                "expected true/false, 1/0, yes/no or on/off, found `{other}`"
            ))),
        }
    }
}

/// An optional bool with the same lenient spellings.
///
/// Needed because `Option<bool>` does not route through [`truthy_bool`]:
/// serde hands the inner value to the option's own deserializer, so
/// `CODEX_AUTH__COOKIE_SECURE=1` would otherwise fail with "expected a
/// boolean".
pub fn optional_truthy_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalTruthyBool;

    impl<'de> Visitor<'de> for OptionalTruthyBool {
        type Value = Option<bool>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a boolean, or nothing")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
            Ok(Some(value))
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            TruthyBool.visit_u64(value).map(Some)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.is_empty() {
                return Ok(None);
            }
            TruthyBool.visit_str(value).map(Some)
        }

        fn visit_some<D2: Deserializer<'de>>(self, inner: D2) -> Result<Self::Value, D2::Error> {
            inner.deserialize_any(TruthyBool).map(Some)
        }
    }

    deserializer.deserialize_option(OptionalTruthyBool)
}

/// A list of strings from a real list or a comma-separated string.
///
/// `CODEX_API__CORS_ORIGINS=https://a.example,https://b.example` has to mean
/// the same thing as a YAML sequence.
pub fn string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringList;

    impl<'de> Visitor<'de> for StringList {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of strings, or a comma-separated string")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Vec<String>, E> {
            Ok(split_csv(value))
        }

        // figment turns an all-digit value into a number before serde sees it,
        // so a single numeric entry would otherwise fail to deserialize.
        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Vec<String>, E> {
            Ok(vec![value.to_string()])
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Vec<String>, E> {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                out.push(item);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_any(StringList)
}

/// A string map from a real map or a `k1=v1,k2=v2` string.
///
/// Used for OTLP headers, where the whole set is conventionally supplied in
/// one variable.
pub fn string_map<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(StringMapVisitor)
}

struct StringMapVisitor;

impl<'de> Visitor<'de> for StringMapVisitor {
    type Value = HashMap<String, String>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a map, or a comma-separated list of key=value pairs")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        let mut out = HashMap::new();
        for entry in split_csv(value) {
            let (key, val) = entry
                .split_once('=')
                .ok_or_else(|| E::custom(format!("expected `key=value`, found `{entry}`")))?;
            let key = key.trim();
            if key.is_empty() {
                return Err(E::custom(format!("empty key in `{entry}`")));
            }
            out.insert(key.to_string(), val.trim().to_string());
        }
        Ok(out)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut out = HashMap::new();
        while let Some((key, value)) = map.next_entry::<String, String>()? {
            out.insert(key, value);
        }
        Ok(out)
    }
}

/// An optional string map, empty meaning absent.
///
/// `database.sqlite.pragmas` is `Option<HashMap<..>>`, so it needs the option
/// wrapper for the same reason `optional_truthy_bool` does: serde hands the
/// inner value to the option's deserializer, bypassing [`string_map`].
pub fn optional_string_map<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalStringMap;

    impl<'de> Visitor<'de> for OptionalStringMap {
        type Value = Option<HashMap<String, String>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map, `key=value` pairs, or nothing")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.is_empty() {
                return Ok(None);
            }
            StringMapVisitor.visit_str(value).map(Some)
        }

        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
            StringMapVisitor.visit_map(map).map(Some)
        }

        fn visit_some<D2: Deserializer<'de>>(self, inner: D2) -> Result<Self::Value, D2::Error> {
            inner.deserialize_any(StringMapVisitor).map(Some)
        }
    }

    deserializer.deserialize_option(OptionalStringMap)
}

/// A map whose values are themselves comma-separated lists.
///
/// OIDC role mapping is set one role at a time
/// (`CODEX_AUTH__OIDC__PROVIDERS__X__ROLE_MAPPING__ADMIN=grp-a,grp-b`), so the
/// values arrive as strings rather than sequences.
pub fn string_list_map<'de, D>(deserializer: D) -> Result<HashMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringListMap;

    impl<'de> Visitor<'de> for StringListMap {
        type Value = HashMap<String, Vec<String>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of names to string lists")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            #[derive(Deserialize)]
            struct Entry(#[serde(deserialize_with = "string_list")] Vec<String>);

            let mut out = HashMap::new();
            while let Some((key, Entry(value))) = map.next_entry::<String, Entry>()? {
                out.insert(key, value);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(StringListMap)
}

/// An optional string where empty means absent.
///
/// v1's `env_string_opt` filtered empty values, so `CODEX_LOGGING__FILE=`
/// meant "no log file" rather than "log to a file named empty string". Keeping
/// that matters because unsetting a variable and blanking it are the same
/// gesture in most deployment tooling.
pub fn optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalString;

    impl<'de> Visitor<'de> for OptionalString {
        type Value = Option<String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a string, or nothing")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            Ok(non_empty(value))
        }

        fn visit_some<D2: Deserializer<'de>>(self, inner: D2) -> Result<Self::Value, D2::Error> {
            let value = String::deserialize(inner)?;
            Ok(non_empty(&value))
        }
    }

    deserializer.deserialize_option(OptionalString)
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

/// Split on commas, trimming each entry and dropping empties, so a trailing
/// comma or padded list does not produce blank members.
fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct BoolHolder {
        #[serde(deserialize_with = "truthy_bool")]
        value: bool,
    }

    #[derive(Debug, Deserialize)]
    struct ListHolder {
        #[serde(deserialize_with = "string_list")]
        value: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct MapHolder {
        #[serde(deserialize_with = "string_map")]
        value: HashMap<String, String>,
    }

    #[derive(Debug, Deserialize)]
    struct OptHolder {
        #[serde(deserialize_with = "optional_string")]
        value: Option<String>,
    }

    fn parse_bool(yaml: &str) -> Result<bool, serde_yaml::Error> {
        serde_yaml::from_str::<BoolHolder>(yaml).map(|h| h.value)
    }

    #[test]
    fn bools_accept_the_spellings_v1_accepted() {
        for yaml in ["value: true", "value: 'true'", "value: 'TRUE'", "value: 1"] {
            assert!(parse_bool(yaml).unwrap(), "{yaml} should be true");
        }
        for yaml in [
            "value: false",
            "value: 'false'",
            "value: 'False'",
            "value: 0",
        ] {
            assert!(!parse_bool(yaml).unwrap(), "{yaml} should be false");
        }
    }

    #[test]
    fn bools_accept_yes_no_and_on_off() {
        assert!(parse_bool("value: 'yes'").unwrap());
        assert!(parse_bool("value: 'on'").unwrap());
        assert!(!parse_bool("value: 'no'").unwrap());
        assert!(!parse_bool("value: 'off'").unwrap());
    }

    /// v1 treated anything that was not `true`/`1` as `false`, so a typo
    /// silently disabled a feature. This must fail instead.
    #[test]
    fn a_misspelled_bool_is_an_error_not_a_silent_false() {
        let error = parse_bool("value: ture").unwrap_err().to_string();
        assert!(
            error.contains("ture"),
            "error should quote the input: {error}"
        );

        assert!(
            parse_bool("value: 2").is_err(),
            "only 0 and 1 are numeric bools"
        );
    }

    #[test]
    fn lists_accept_a_sequence_or_a_comma_separated_string() {
        let from_seq = serde_yaml::from_str::<ListHolder>("value:\n  - a\n  - b\n")
            .unwrap()
            .value;
        let from_csv = serde_yaml::from_str::<ListHolder>("value: 'a,b'")
            .unwrap()
            .value;
        assert_eq!(from_seq, vec!["a", "b"]);
        assert_eq!(from_csv, from_seq);
    }

    #[test]
    fn list_entries_are_trimmed_and_blanks_dropped() {
        let parsed = serde_yaml::from_str::<ListHolder>("value: ' a , b , '")
            .unwrap()
            .value;
        assert_eq!(parsed, vec!["a", "b"]);

        let empty = serde_yaml::from_str::<ListHolder>("value: ''")
            .unwrap()
            .value;
        assert!(empty.is_empty());
    }

    /// figment parses an all-digit value as a number, so a one-element list
    /// of digits must still work.
    #[test]
    fn lists_accept_a_bare_number() {
        let parsed = serde_yaml::from_str::<ListHolder>("value: 8080")
            .unwrap()
            .value;
        assert_eq!(parsed, vec!["8080"]);
    }

    #[test]
    fn maps_accept_a_mapping_or_key_value_pairs() {
        let from_map = serde_yaml::from_str::<MapHolder>("value:\n  a: '1'\n  b: '2'\n")
            .unwrap()
            .value;
        let from_pairs = serde_yaml::from_str::<MapHolder>("value: 'a=1,b=2'")
            .unwrap()
            .value;
        assert_eq!(from_map, from_pairs);
        assert_eq!(from_pairs.get("a").map(String::as_str), Some("1"));
    }

    #[test]
    fn a_pair_without_an_equals_sign_is_an_error() {
        let error = serde_yaml::from_str::<MapHolder>("value: 'a=1,oops'")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("oops"),
            "error should quote the entry: {error}"
        );
    }

    #[test]
    fn a_value_may_itself_contain_an_equals_sign() {
        let parsed = serde_yaml::from_str::<MapHolder>("value: 'authorization=Bearer a=b'")
            .unwrap()
            .value;
        assert_eq!(
            parsed.get("authorization").map(String::as_str),
            Some("Bearer a=b")
        );
    }

    #[test]
    fn an_empty_optional_string_is_absent() {
        assert_eq!(
            serde_yaml::from_str::<OptHolder>("value: ''")
                .unwrap()
                .value,
            None
        );
        assert_eq!(
            serde_yaml::from_str::<OptHolder>("value: null")
                .unwrap()
                .value,
            None
        );
        assert_eq!(
            serde_yaml::from_str::<OptHolder>("value: /var/log/codex.log")
                .unwrap()
                .value,
            Some("/var/log/codex.log".to_string())
        );
    }
}
