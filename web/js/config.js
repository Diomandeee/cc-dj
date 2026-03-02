/* ============================================================
   cc-dj  |  Config — settings dialog
   ============================================================ */

const Config = (() => {
  let currentConfig = null;

  function init() {
    // Settings could be opened via a button (add one if desired)
    // For now, dialog can be opened programmatically
  }

  function applyConfig(cfg) {
    currentConfig = cfg;

    // Update software badge
    const badge = document.getElementById('software-badge');
    if (badge && cfg.software) {
      badge.textContent = cfg.software.toUpperCase();
    }

    // Update safety checkboxes in dialog
    const lockPlaying = document.getElementById('lock-playing');
    if (lockPlaying && cfg.safety) {
      lockPlaying.checked = cfg.safety.lock_playing_deck;
    }
    const forbidLoad = document.getElementById('forbid-load');
    if (forbidLoad && cfg.safety) {
      forbidLoad.checked = cfg.safety.forbid_load_on_live;
    }

    // Set software radio
    if (cfg.software) {
      const radio = document.querySelector(`input[name="software"][value="${cfg.software}"]`);
      if (radio) radio.checked = true;
    }
  }

  function openSettings() {
    const dialog = document.getElementById('settings-dialog');
    if (dialog) dialog.showModal();
  }

  return { init, applyConfig, openSettings };
})();
