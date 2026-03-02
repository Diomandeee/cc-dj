//! Command definitions for DJ software control.
//!
//! Commands represent actions that can be performed in DJ software
//! like Rekordbox or Serato, mapped to keyboard shortcuts or MIDI.

use serde::{Deserialize, Serialize};

/// A DJ software command.
///
/// Commands are loaded from `commands.yaml` and define the mapping
/// between voice/gesture inputs and DJ software actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    /// Unique command identifier (e.g., "3006").
    pub id: String,

    /// Canonical name (e.g., "Play/Pause").
    pub canonical: String,

    /// Alternative names/phrases that trigger this command.
    #[serde(default)]
    pub synonyms: Vec<String>,

    /// Command category for grouping.
    pub category: CommandCategory,

    /// Target deck (if deck-specific).
    #[serde(default)]
    pub deck: Option<Deck>,

    /// Type of action performed.
    pub action_type: ActionType,

    /// Keyboard shortcut or MIDI mapping.
    pub shortcut: Shortcut,

    /// Safety configuration for this command.
    #[serde(default)]
    pub safety: CommandSafety,
}

impl Command {
    /// Returns true if this command matches the given text.
    pub fn matches(&self, text: &str) -> bool {
        let text_lower = text.to_lowercase();
        let canonical_lower = self.canonical.to_lowercase();

        if canonical_lower == text_lower {
            return true;
        }

        self.synonyms.iter().any(|s| s.to_lowercase() == text_lower)
    }

    /// Returns all trigger phrases (canonical + synonyms).
    pub fn all_triggers(&self) -> Vec<&str> {
        let mut triggers = vec![self.canonical.as_str()];
        triggers.extend(self.synonyms.iter().map(|s| s.as_str()));
        triggers
    }
}

/// Command category for grouping related commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCategory {
    /// Transport controls (play, pause, cue).
    Transport,
    /// Layout controls (zoom, view switching).
    Layout,
    /// Looping controls.
    Looping,
    /// Cue point controls.
    Cue,
    /// Effects controls.
    Effects,
    /// Library/browser controls.
    Library,
    /// Browse/navigation controls.
    Browse,
    /// Mixer controls (EQ, fader).
    Mixer,
    /// Grid/beatgrid controls.
    Grid,
    /// Hot cue controls.
    Hotcue,
    /// Sampler controls.
    Sampler,
    /// Recording controls.
    Recording,
    /// System/utility commands.
    System,
    /// Unknown category.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Target deck for deck-specific commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deck {
    /// Left deck (Deck 1/A).
    Left,
    /// Right deck (Deck 2/B).
    Right,
    /// Both decks.
    Both,
    /// Master/global.
    Master,
}

impl Deck {
    /// Returns the deck number (1-based).
    pub fn number(&self) -> Option<u8> {
        match self {
            Deck::Left => Some(1),
            Deck::Right => Some(2),
            Deck::Both | Deck::Master => None,
        }
    }

    /// Returns the deck letter.
    pub fn letter(&self) -> &'static str {
        match self {
            Deck::Left => "A",
            Deck::Right => "B",
            Deck::Both => "AB",
            Deck::Master => "M",
        }
    }
}

/// Type of action performed by a command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Play/pause toggle.
    PlayPause,
    /// Cue point operation.
    Cue,
    /// Sync operation.
    Sync,
    /// Loop operation.
    Loop,
    /// Effect toggle.
    Effect,
    /// EQ adjustment.
    Eq,
    /// Fader/crossfader.
    Fader,
    /// Library navigation.
    Navigate,
    /// Track loading.
    Load,
    /// Layout/view change.
    Layout,
    /// Grid adjustment.
    Grid,
    /// Hot cue operation.
    Hotcue,
    /// Sampler operation.
    Sampler,
    /// Recording operation.
    Record,
    /// System operation.
    System,
    /// Unknown action type.
    #[default]
    #[serde(other)]
    Unknown,
}

/// Keyboard shortcut or MIDI mapping.
///
/// Supports both bare strings (backward compat: `shortcut: Z`) and
/// tagged objects (`shortcut: { type: key, key: Z }`).
#[derive(Debug, Clone, Serialize)]
pub enum Shortcut {
    /// Simple keyboard key.
    Key {
        /// The key to press.
        key: String,
    },
    /// Key with modifiers.
    KeyCombo {
        /// The main key.
        key: String,
        /// Modifier keys (shift, ctrl, alt, cmd).
        modifiers: Vec<Modifier>,
    },
    /// Sequence of keys.
    Sequence {
        /// Steps in the sequence.
        steps: Vec<ShortcutStep>,
    },
    /// MIDI message.
    Midi {
        /// MIDI channel (0-15).
        channel: u8,
        /// Note or CC number.
        note: u8,
        /// Velocity or value.
        velocity: u8,
    },
}

/// Tagged shortcut format for structured YAML/JSON.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TaggedShortcut {
    Key {
        key: String,
    },
    #[serde(alias = "key_combo")]
    KeyCombo {
        key: String,
        #[serde(default)]
        modifiers: Vec<Modifier>,
    },
    Sequence {
        steps: Vec<ShortcutStep>,
    },
    Midi {
        channel: u8,
        note: u8,
        #[serde(default = "default_velocity")]
        velocity: u8,
    },
}

impl<'de> serde::Deserialize<'de> for Shortcut {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        // Accept either a bare string or a tagged object.
        struct ShortcutVisitor;

        impl<'de> de::Visitor<'de> for ShortcutVisitor {
            type Value = Shortcut;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string shortcut or a tagged shortcut object")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Shortcut, E> {
                Ok(Shortcut::Key { key: v.to_string() })
            }

            fn visit_map<M: de::MapAccess<'de>>(
                self,
                map: M,
            ) -> std::result::Result<Shortcut, M::Error> {
                let tagged: TaggedShortcut =
                    de::Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(match tagged {
                    TaggedShortcut::Key { key } => Shortcut::Key { key },
                    TaggedShortcut::KeyCombo { key, modifiers } => {
                        Shortcut::KeyCombo { key, modifiers }
                    }
                    TaggedShortcut::Sequence { steps } => Shortcut::Sequence { steps },
                    TaggedShortcut::Midi {
                        channel,
                        note,
                        velocity,
                    } => Shortcut::Midi {
                        channel,
                        note,
                        velocity,
                    },
                })
            }
        }

        deserializer.deserialize_any(ShortcutVisitor)
    }
}

fn default_velocity() -> u8 {
    127
}

/// A step in a shortcut sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutStep {
    /// The key to press.
    pub key: String,
    /// Optional modifiers.
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    /// Optional delay after this step (ms).
    #[serde(default)]
    pub delay_ms: u32,
}

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    /// Shift key.
    Shift,
    /// Control key.
    Ctrl,
    /// Alt/Option key.
    Alt,
    /// Command key (macOS).
    Cmd,
    /// Windows key.
    Win,
}

/// Safety configuration for a command.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandSafety {
    /// Whether this command is destructive (can't undo).
    #[serde(default)]
    pub destructive: bool,

    /// Whether this command requires the deck to be idle.
    #[serde(default)]
    pub requires_idle: bool,

    /// Whether to confirm before executing if deck is playing.
    #[serde(default)]
    pub confirm_if_playing: bool,

    /// Cooldown period in milliseconds.
    #[serde(default = "default_cooldown")]
    pub cooldown_ms: u32,
}

fn default_cooldown() -> u32 {
    500
}

/// A catalog of commands loaded from YAML.
#[derive(Debug, Clone, Default)]
pub struct CommandCatalog {
    /// All commands indexed by ID.
    commands: std::collections::HashMap<String, Command>,
    /// Commands indexed by category.
    by_category: std::collections::HashMap<CommandCategory, Vec<String>>,
}

impl CommandCatalog {
    /// Creates a new empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads commands from YAML.
    pub fn from_yaml(yaml: &str) -> crate::Result<Self> {
        #[derive(Deserialize)]
        struct CommandsFile {
            commands: Vec<Command>,
        }

        let file: CommandsFile = serde_yml::from_str(yaml)?;
        let mut catalog = Self::new();

        for cmd in file.commands {
            catalog.add(cmd);
        }

        Ok(catalog)
    }

    /// Adds a command to the catalog.
    pub fn add(&mut self, command: Command) {
        let id = command.id.clone();
        let category = command.category;

        self.commands.insert(id.clone(), command);
        self.by_category.entry(category).or_default().push(id);
    }

    /// Gets a command by ID.
    pub fn get(&self, id: &str) -> Option<&Command> {
        self.commands.get(id)
    }

    /// Finds commands matching the given text.
    pub fn find_matching(&self, text: &str) -> Vec<&Command> {
        self.commands
            .values()
            .filter(|cmd| cmd.matches(text))
            .collect()
    }

    /// Gets all commands in a category.
    pub fn by_category(&self, category: CommandCategory) -> Vec<&Command> {
        self.by_category
            .get(&category)
            .map(|ids| ids.iter().filter_map(|id| self.get(id)).collect())
            .unwrap_or_default()
    }

    /// Returns the total number of commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns true if the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Iterates over all commands.
    pub fn iter(&self) -> impl Iterator<Item = &Command> {
        self.commands.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_matches() {
        let cmd = Command {
            id: "3006".to_string(),
            canonical: "Play/Pause".to_string(),
            synonyms: vec!["play left deck".to_string(), "play left".to_string()],
            category: CommandCategory::Transport,
            deck: Some(Deck::Left),
            action_type: ActionType::PlayPause,
            shortcut: Shortcut::Key {
                key: "Z".to_string(),
            },
            safety: CommandSafety::default(),
        };

        assert!(cmd.matches("Play/Pause"));
        assert!(cmd.matches("play/pause"));
        assert!(cmd.matches("play left deck"));
        assert!(!cmd.matches("stop"));
    }

    #[test]
    fn test_deck_properties() {
        assert_eq!(Deck::Left.number(), Some(1));
        assert_eq!(Deck::Right.number(), Some(2));
        assert_eq!(Deck::Left.letter(), "A");
        assert_eq!(Deck::Right.letter(), "B");
    }

    #[test]
    fn test_catalog_from_yaml() {
        let yaml = r#"
commands:
  - id: "1"
    canonical: Test Command
    synonyms: []
    category: transport
    action_type: play_pause
    shortcut:
      type: key
      key: Z
"#;

        let catalog = CommandCatalog::from_yaml(yaml).unwrap();
        assert_eq!(catalog.len(), 1);
        assert!(catalog.get("1").is_some());
    }
}
