/* ============================================================
   cc-dj  |  Voice — ticker feed, voice toggle, status indicator
   ============================================================ */

const Voice = (() => {
  const MAX_ENTRIES = 50;
  let entries = [];

  function init() {
    const toggle = document.getElementById('voice-toggle');
    toggle.addEventListener('click', async () => {
      const isActive = toggle.classList.contains('active');
      const endpoint = isActive ? '/api/voice/stop' : '/api/voice/start';
      try {
        await fetch(endpoint, { method: 'POST' });
      } catch (e) {
        console.warn('[Voice] Toggle failed:', e);
      }
    });
  }

  function updateStatus(active) {
    const toggle = document.getElementById('voice-toggle');
    const status = document.getElementById('voice-status');

    toggle.classList.toggle('active', active);

    if (active) {
      status.textContent = 'LISTENING';
      status.className = 'voice-status listening';
    } else {
      status.textContent = 'OFF';
      status.className = 'voice-status off';
    }
  }

  /**
   * Add an entry to the voice ticker.
   * @param {string} text     The heard text
   * @param {string|null} command  Command name (null if unrecognized)
   * @param {number} ts       Timestamp in ms
   */
  function addEntry(text, command, ts) {
    const timeStr = formatTs(ts);

    const entry = { text, command, timeStr };
    entries.unshift(entry);
    if (entries.length > MAX_ENTRIES) entries.pop();

    renderFeed();
  }

  function formatTs(ms) {
    const secs = Math.floor(ms / 1000);
    const h = String(Math.floor(secs / 3600)).padStart(2, '0');
    const m = String(Math.floor((secs % 3600) / 60)).padStart(2, '0');
    const s = String(secs % 60).padStart(2, '0');
    return `${h}:${m}:${s}`;
  }

  function renderFeed() {
    const feed = document.getElementById('ticker-feed');
    // Build HTML for latest entries
    feed.innerHTML = entries.map(e => {
      if (e.command) {
        return `<div class="ticker-entry">
          <span class="ts">[${e.timeStr}]</span>
          <span class="heard">"${escapeHtml(e.text)}"</span>
          <span class="command">${escapeHtml(e.command)}</span>
          <span class="check">\u2713</span>
        </div>`;
      } else {
        return `<div class="ticker-entry unrecognized">
          <span class="ts">[${e.timeStr}]</span>
          <span class="heard">"${escapeHtml(e.text)}"</span>
        </div>`;
      }
    }).join('');
  }

  function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  return { init, updateStatus, addEntry };
})();
