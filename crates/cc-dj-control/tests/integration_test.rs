//! Integration tests exercising the config -> deck -> bridge pipeline in simulation mode.

use cc_dj_control::DeckController;
use cc_dj_types::{DJConfig, DJSoftware};

#[tokio::test]
async fn test_full_pipeline_simulation() {
    let config = DJConfig::default();
    assert_eq!(config.software, DJSoftware::Rekordbox);

    let mut controller = DeckController::new(config);
    assert_eq!(controller.bridge_name(), "Rekordbox");

    // Verify action space is populated
    let space = controller.action_space();
    assert!(!space.is_empty(), "action space should have actions");

    // Beat updates should work without error
    controller.update_beat(1.0);
    controller.update_beat(4.0);
    assert_eq!(controller.current_beat(), 4.0);
}

#[tokio::test]
async fn test_serato_bridge_creation() {
    let config = DJConfig {
        software: DJSoftware::Serato,
        ..DJConfig::default()
    };

    let controller = DeckController::new(config);
    assert_eq!(controller.bridge_name(), "Serato");
}

#[tokio::test]
async fn test_config_yaml_roundtrip() {
    let yaml = r#"
dj:
  software: rekordbox
  quant_window_deg: 20
  tiers_enabled: [0, 1, 2, 3, 4]
  safety:
    lock_playing_deck: true
    forbid_load_on_live: true
  voice:
    enabled: false
"#;

    let config = DJConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.software, DJSoftware::Rekordbox);
    assert_eq!(config.quant_window_deg, 20.0);
    assert_eq!(config.tiers_enabled.len(), 5);
    assert!(config.safety.lock_playing_deck);
}

#[tokio::test]
async fn test_tier_locking() {
    let config = DJConfig {
        tiers_enabled: vec![0, 1],
        ..DJConfig::default()
    };

    let controller = DeckController::new(config);
    let space = controller.action_space();
    assert!(!space.is_empty());
}

#[tokio::test]
async fn test_command_catalog_loading() {
    let yaml = r#"
commands:
  - id: "3006"
    canonical: "Play/Pause"
    synonyms: ["play left deck", "play left"]
    category: transport
    deck: left
    action_type: play_pause
    shortcut: Z
  - id: "3007"
    canonical: "Cue"
    synonyms: ["cue left"]
    category: transport
    deck: left
    action_type: cue
    shortcut:
      type: key_combo
      key: X
      modifiers: [shift]
"#;

    let catalog = cc_dj_types::CommandCatalog::from_yaml(yaml).unwrap();
    assert_eq!(catalog.len(), 2);

    let play = catalog.get("3006").unwrap();
    assert!(play.matches("play left deck"));
    assert!(!play.matches("sync"));

    let cue = catalog.get("3007").unwrap();
    assert!(cue.matches("Cue"));
    assert_eq!(cue.all_triggers().len(), 2);
}

#[tokio::test]
async fn test_dj_software_display() {
    assert_eq!(DJSoftware::Rekordbox.to_string(), "rekordbox");
    assert_eq!(DJSoftware::Serato.to_string(), "serato");
}
