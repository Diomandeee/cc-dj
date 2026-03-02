/* ============================================================
   cc-dj  |  Actions — grid, tier tabs, cooldown timers
   ============================================================ */

const Actions = (() => {
  let allActions = [];
  let currentTier = 0;
  let enabledTiers = new Set([0, 1, 2, 3]);

  const TIER_NAMES = ['Transport', 'Looping', 'Cues', 'FX', 'Library', 'Blend'];

  function init() {
    // Tier tab clicks
    document.querySelectorAll('.tier-tab').forEach(tab => {
      tab.addEventListener('click', () => {
        const tier = parseInt(tab.dataset.tier);
        if (tab.classList.contains('locked')) return;
        selectTier(tier);
      });
    });
  }

  function selectTier(tier) {
    currentTier = tier;
    document.querySelectorAll('.tier-tab').forEach(t => {
      t.classList.toggle('active', parseInt(t.dataset.tier) === tier);
    });
    renderButtons();
  }

  /**
   * Render the full action list from server data.
   * @param {Array} actions - ActionInfo array from /api/actions
   */
  function render(actions) {
    allActions = actions;

    // Update enabled tiers
    enabledTiers.clear();
    actions.forEach(a => {
      if (a.enabled) enabledTiers.add(a.tier);
    });

    // Update tier tabs and tier bar
    document.querySelectorAll('.tier-tab').forEach(tab => {
      const t = parseInt(tab.dataset.tier);
      tab.classList.toggle('locked', !enabledTiers.has(t));
    });
    document.querySelectorAll('.tier-segment').forEach(seg => {
      const t = parseInt(seg.dataset.tier);
      seg.classList.toggle('locked', !enabledTiers.has(t));
    });

    renderButtons();
  }

  function renderButtons() {
    const container = document.getElementById('action-buttons');
    const tierActions = allActions.filter(a => a.tier === currentTier);

    container.innerHTML = tierActions.map(a => {
      const classes = ['action-btn'];
      if (!a.enabled) classes.push('locked');

      const deckTag = a.deck ? `<span class="deck-tag">${a.deck}</span>` : '';
      const label = formatActionName(a.name);

      return `<button class="${classes.join(' ')}" data-action="${a.name}" title="${a.name}">
        ${label}${deckTag}
      </button>`;
    }).join('');

    // Wire click handlers
    container.querySelectorAll('.action-btn:not(.locked)').forEach(btn => {
      btn.addEventListener('click', () => {
        executeAction(btn.dataset.action);
      });
    });
  }

  /**
   * Format action name for display: PLAY_A → Play A, LOOP_4_A → Loop 4 A
   */
  function formatActionName(name) {
    return name.split('_').map(part =>
      part.charAt(0) + part.slice(1).toLowerCase()
    ).join(' ');
  }

  /**
   * Flash an action button green (success) or red (error).
   */
  function flash(actionName, type) {
    const btn = document.querySelector(`.action-btn[data-action="${actionName}"]`);
    if (!btn) return;

    const cls = type === 'success' ? 'flash-success' : 'flash-error';
    btn.classList.add(cls);
    setTimeout(() => btn.classList.remove(cls), 400);
  }

  /**
   * Animate tier unlock.
   */
  function unlockTier(tierNum, tierName) {
    enabledTiers.add(tierNum);

    // Update tab
    const tab = document.querySelector(`.tier-tab[data-tier="${tierNum}"]`);
    if (tab) tab.classList.remove('locked');

    // Update tier bar segment
    const seg = document.querySelector(`.tier-segment[data-tier="${tierNum}"]`);
    if (seg) {
      seg.classList.remove('locked');
      seg.classList.add('unlocking');
      setTimeout(() => seg.classList.remove('unlocking'), 600);
    }

    // If current tier, re-render buttons with cascade
    if (tierNum === currentTier) {
      renderButtons();
      // Stagger cascade animation
      const btns = document.querySelectorAll('.action-btn');
      btns.forEach((btn, i) => {
        btn.style.animationDelay = `${i * 50}ms`;
        btn.classList.add('cascade');
        setTimeout(() => {
          btn.classList.remove('cascade');
          btn.style.animationDelay = '';
        }, 300 + i * 50);
      });
    }
  }

  return { init, render, flash, unlockTier };
})();
