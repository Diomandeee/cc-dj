//! Intent processor - maps recognized speech to DJ commands.

use cc_dj_types::{Command, CommandCatalog, DJConfig};
use std::collections::HashMap;
use std::sync::Arc;

/// Intent processor for voice commands.
///
/// Handles the mapping from natural language to structured commands,
/// including synonym resolution and context-aware disambiguation.
pub struct IntentProcessor {
    /// DJ configuration.
    #[allow(dead_code)]
    config: Arc<DJConfig>,
    /// Command catalog.
    catalog: CommandCatalog,
    /// Custom command mappings from config.
    custom_map: HashMap<String, String>,
}

impl IntentProcessor {
    /// Creates a new intent processor.
    pub fn new(config: Arc<DJConfig>) -> Self {
        let custom_map = config.voice.command_map.clone();
        Self {
            config,
            catalog: CommandCatalog::new(),
            custom_map,
        }
    }

    /// Loads commands from YAML.
    pub fn load_commands(&mut self, yaml: &str) -> cc_dj_types::Result<()> {
        self.catalog = CommandCatalog::from_yaml(yaml)?;
        Ok(())
    }

    /// Processes text and returns matching commands.
    pub fn process(&self, text: &str) -> Vec<Command> {
        let text_lower = text.to_lowercase().trim().to_string();

        // Check custom mappings first
        if let Some(action_name) = self.custom_map.get(&text_lower) {
            tracing::debug!("Custom mapping: {} -> {}", text_lower, action_name);

            // Look up the command by action name in the catalog
            if let Some(cmd) = self
                .catalog
                .iter()
                .find(|c| c.id == *action_name || c.canonical.eq_ignore_ascii_case(action_name))
            {
                return vec![cmd.clone()];
            }

            // If not found in catalog, create an action-based command
            return vec![Command {
                id: action_name.clone(),
                canonical: action_name.clone(),
                synonyms: vec![text_lower.clone()],
                category: cc_dj_types::CommandCategory::Unknown,
                deck: None,
                action_type: cc_dj_types::ActionType::Unknown,
                shortcut: cc_dj_types::Shortcut::Key { key: String::new() },
                safety: Default::default(),
            }];
        }

        // Find matching commands from catalog
        self.catalog
            .find_matching(&text_lower)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Adds a custom command mapping.
    pub fn add_mapping(&mut self, phrase: impl Into<String>, action: impl Into<String>) {
        self.custom_map
            .insert(phrase.into().to_lowercase(), action.into());
    }

    /// Returns the number of custom mappings.
    pub fn custom_mapping_count(&self) -> usize {
        self.custom_map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_processor() {
        let config = Arc::new(DJConfig::default());
        let processor = IntentProcessor::new(config);
        assert_eq!(processor.custom_mapping_count(), 0);
    }

    #[test]
    fn test_add_mapping() {
        let config = Arc::new(DJConfig::default());
        let mut processor = IntentProcessor::new(config);
        processor.add_mapping("play deck a", "PLAY_A");
        assert_eq!(processor.custom_mapping_count(), 1);
    }
}
