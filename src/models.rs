use serde::{Deserialize, Serialize};

// ============================================================================
// Scanning Strategy
// ============================================================================

/// Scanning strategy type for library organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanningStrategy {
    /// Default: Direct child folders = series (expandable in the future)
    Default,
}

impl ScanningStrategy {
    /// Convert to string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
        }
    }

    /// Parse from string representation
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "default" => Some(Self::Default),
            _ => None,
        }
    }
}
