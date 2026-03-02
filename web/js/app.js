/* ============================================================
   cc-dj  |  App — init, global state, render loop, event routing
   ============================================================ */

const STATE = {
  session: null,
  elapsed_secs: 0,
  voice_active: false,
  actions: [],
  connected: false,
};

// ── Fetch initial data ──
async function fetchActions() {
  try {
    const res = await fetch('/api/actions');
    STATE.actions = await res.json();
    Actions.render(STATE.actions);
  } catch (e) {
    console.warn('[App] Failed to fetch actions:', e);
  }
}

async function fetchConfig() {
  try {
    const res = await fetch('/api/config');
    const cfg = await res.json();
    document.getElementById('software-badge').textContent = cfg.software.toUpperCase();
    Config.applyConfig(cfg);
  } catch (e) {
    console.warn('[App] Failed to fetch config:', e);
  }
}

// ── Session clock ──
function updateClock() {
  const secs = Math.floor(STATE.elapsed_secs);
  const h = String(Math.floor(secs / 3600)).padStart(2, '0');
  const m = String(Math.floor((secs % 3600) / 60)).padStart(2, '0');
  const s = String(secs % 60).padStart(2, '0');
  document.getElementById('session-clock').textContent = `${h}:${m}:${s}`;
}

// ── Main event router ──
function handleWsEvent(event) {
  switch (event.type) {
    case 'state':
      STATE.session = event.session;
      STATE.elapsed_secs = event.elapsed_secs;
      STATE.voice_active = event.voice_active;
      updateClock();
      Deck.update(0, event.session.decks[0]);
      Deck.update(1, event.session.decks[1]);
      Mixer.update(event.session.mixer);
      Voice.updateStatus(event.voice_active);
      // Energy
      const energy = event.session.energy_level || 5;
      document.getElementById('energy-fill').style.width = `${energy * 10}%`;
      document.getElementById('energy-value').textContent = energy;
      break;

    case 'beat':
      Deck.onBeat(event.beat, event.bpm);
      break;

    case 'voice_heard':
      Voice.addEntry(event.text, null, event.ts);
      break;

    case 'voice_command':
      Voice.addEntry(event.text, event.command, event.ts);
      break;

    case 'action_executed':
      Actions.flash(event.action, 'success');
      break;

    case 'action_failed':
      Actions.flash(event.action, 'error');
      break;

    case 'tier_unlocked':
      Actions.unlockTier(event.tier, event.name);
      break;

    case '_connected':
      STATE.connected = true;
      fetchActions();
      fetchConfig();
      break;

    case '_disconnected':
      STATE.connected = false;
      break;
  }
}

// ── Execute action via REST ──
async function executeAction(actionName) {
  try {
    const res = await fetch('/api/execute', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: actionName }),
    });
    const data = await res.json();
    if (!data.ok) {
      console.warn('[App] Execute failed:', data.error);
    }
  } catch (e) {
    console.error('[App] Execute error:', e);
  }
}

// ── Wire transport buttons ──
function wireTransportButtons() {
  document.querySelectorAll('.transport-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const action = btn.dataset.action;
      if (action) executeAction(action);
    });
  });
}

// ── Init ──
document.addEventListener('DOMContentLoaded', () => {
  Deck.init();
  Mixer.init();
  Voice.init();
  Actions.init();
  Config.init();
  wireTransportButtons();

  WS.on(handleWsEvent);
  WS.connect();
});
