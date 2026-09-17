use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    #[default]
    Auto,
    None,
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub fn api_value(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::None => Some("none"),
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Auto => Self::None,
            Self::None => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Auto,
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "auto",
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    Reasoning,
    Error,
}

impl Role {
    pub fn api_name(self) -> Option<&'static str> {
        match self {
            Self::User => Some("user"),
            Self::Assistant => Some("assistant"),
            Self::Reasoning => None,
            Self::Error => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::ReasoningEffort;

    #[test]
    fn reasoning_effort_cycles_and_maps_to_api_values() {
        let values = [
            ReasoningEffort::Auto,
            ReasoningEffort::None,
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
        ];
        let api_values = [
            None,
            Some("none"),
            Some("low"),
            Some("medium"),
            Some("high"),
        ];

        for (index, effort) in values.into_iter().enumerate() {
            assert_eq!(effort.api_value(), api_values[index]);
            assert_eq!(effort.cycle(), values[(index + 1) % values.len()]);
        }
    }
}
