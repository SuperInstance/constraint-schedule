//! Agent profiles for constraint-satisfaction scheduling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Profile describing an agent's capabilities, capacity, and preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Unique agent identifier.
    pub id: String,
    /// Skills this agent possesses (e.g., "rust", "python", "database").
    pub capabilities: Vec<String>,
    /// Maximum number of concurrent tasks this agent can handle.
    pub capacity: u32,
    /// Weighted preference scores per skill (higher = more preferred).
    pub preferences: HashMap<String, f64>,
    /// Current number of active tasks.
    pub current_load: u32,
}

impl AgentProfile {
    /// Create a new agent profile.
    pub fn new(id: impl Into<String>, capabilities: Vec<String>, capacity: u32) -> Self {
        Self {
            id: id.into(),
            capabilities,
            capacity,
            preferences: HashMap::new(),
            current_load: 0,
        }
    }

    /// Create a builder-style agent with preferences.
    pub fn with_preferences(mut self, preferences: HashMap<String, f64>) -> Self {
        self.preferences = preferences;
        self
    }

    /// Create a builder-style agent with a current load.
    pub fn with_load(mut self, load: u32) -> Self {
        self.current_load = load;
        self
    }

    /// Check if this agent has a specific capability.
    pub fn has_capability(&self, skill: &str) -> bool {
        self.capabilities.iter().any(|c| c == skill)
    }

    /// Check if this agent has all the required capabilities.
    pub fn has_capabilities(&self, skills: &[String]) -> bool {
        skills.iter().all(|s| self.has_capability(s))
    }

    /// Remaining capacity for new tasks.
    pub fn remaining_capacity(&self) -> u32 {
        self.capacity.saturating_sub(self.current_load)
    }

    /// Check if the agent can accept more tasks.
    pub fn has_capacity(&self) -> bool {
        self.remaining_capacity() > 0
    }

    /// Preference score for a given skill. Returns 0.0 if no preference set.
    pub fn preference_for(&self, skill: &str) -> f64 {
        self.preferences.get(skill).copied().unwrap_or(0.0)
    }

    /// Total preference score for a set of required capabilities.
    pub fn total_preference(&self, skills: &[String]) -> f64 {
        skills.iter().map(|s| self.preference_for(s)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent() -> AgentProfile {
        let mut prefs = HashMap::new();
        prefs.insert("rust".to_string(), 10.0);
        prefs.insert("python".to_string(), 5.0);
        AgentProfile::new("agent-1", vec!["rust".into(), "python".into()], 3)
            .with_preferences(prefs)
            .with_load(1)
    }

    #[test]
    fn test_new_agent() {
        let a = AgentProfile::new("a", vec!["x".into()], 2);
        assert_eq!(a.id, "a");
        assert_eq!(a.capabilities, vec!["x"]);
        assert_eq!(a.capacity, 2);
        assert_eq!(a.current_load, 0);
        assert!(a.preferences.is_empty());
    }

    #[test]
    fn test_has_capability() {
        let a = make_agent();
        assert!(a.has_capability("rust"));
        assert!(a.has_capability("python"));
        assert!(!a.has_capability("java"));
    }

    #[test]
    fn test_has_capabilities() {
        let a = make_agent();
        assert!(a.has_capabilities(&["rust".into()]));
        assert!(a.has_capabilities(&["rust".into(), "python".into()]));
        assert!(!a.has_capabilities(&["rust".into(), "java".into()]));
    }

    #[test]
    fn test_remaining_capacity() {
        let a = make_agent();
        assert_eq!(a.remaining_capacity(), 2);
        assert!(a.has_capacity());
    }

    #[test]
    fn test_zero_capacity() {
        let a = AgentProfile::new("a", vec![], 2).with_load(2);
        assert_eq!(a.remaining_capacity(), 0);
        assert!(!a.has_capacity());
    }

    #[test]
    fn test_overflow_capacity() {
        let a = AgentProfile::new("a", vec![], 1).with_load(5);
        assert_eq!(a.remaining_capacity(), 0);
    }

    #[test]
    fn test_preference_for() {
        let a = make_agent();
        assert_eq!(a.preference_for("rust"), 10.0);
        assert_eq!(a.preference_for("python"), 5.0);
        assert_eq!(a.preference_for("java"), 0.0);
    }

    #[test]
    fn test_total_preference() {
        let a = make_agent();
        let skills = vec!["rust".into(), "python".into()];
        assert_eq!(a.total_preference(&skills), 15.0);
    }

    #[test]
    fn test_serialize_deserialize() {
        let a = make_agent();
        let json = serde_json::to_string(&a).unwrap();
        let a2: AgentProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(a.id, a2.id);
        assert_eq!(a.capabilities, a2.capabilities);
        assert_eq!(a.capacity, a2.capacity);
        assert_eq!(a.current_load, a2.current_load);
        assert_eq!(a.preferences, a2.preferences);
    }

    #[test]
    fn test_empty_agent() {
        let a = AgentProfile::new("empty", vec![], 0);
        assert!(!a.has_capability("anything"));
        assert_eq!(a.remaining_capacity(), 0);
        assert!(!a.has_capacity());
    }
}
