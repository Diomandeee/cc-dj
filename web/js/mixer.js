/* ============================================================
   cc-dj  |  Mixer — EQ bars, crossfader, volume faders, knobs
   ============================================================ */

const Mixer = (() => {
  function init() {
    // Display-only for now — no interactive controls yet
  }

  /**
   * Update mixer display from MixerState.
   * @param {Object} mixer - MixerState from server
   */
  function update(mixer) {
    if (!mixer) return;

    // Crossfader: -1.0 (left) to 1.0 (right) → 0% to 100%
    const cfThumb = document.getElementById('cf-thumb');
    if (cfThumb) {
      const pct = ((mixer.crossfader + 1) / 2) * 100;
      cfThumb.style.left = `${pct}%`;
    }

    // Master volume knob rotation: 0.0 → -135deg, 1.0 → +135deg
    const masterKnob = document.querySelector('#master-vol .knob-indicator');
    if (masterKnob) {
      const deg = (mixer.master_volume - 0.5) * 270;
      masterKnob.style.transform = `rotate(${deg}deg)`;
    }

    // Channel strips
    const channels = mixer.channels || [];
    updateChannel(0, channels[0], 'a');
    updateChannel(1, channels[1], 'b');
  }

  function updateChannel(index, ch, id) {
    if (!ch) return;

    // Volume fader fill: 0.0 → 0%, 1.0 → 100%
    const faderFill = document.querySelector(`#volume-${id} .fader-fill`);
    if (faderFill) {
      faderFill.style.height = `${ch.volume * 100}%`;
    }

    // EQ bars: -1.0 to 1.0
    updateEQ(`eq-hi-${id}`, ch.eq_high);
    updateEQ(`eq-mid-${id}`, ch.eq_mid);
    updateEQ(`eq-lo-${id}`, ch.eq_low);

    // Filter knob: 0.0-1.0 (0.5 = bypass)
    const filterKnob = document.querySelector(`#filter-${id} .knob-indicator`);
    if (filterKnob) {
      const deg = (ch.filter - 0.5) * 270;
      filterKnob.style.transform = `rotate(${deg}deg)`;
    }

    // CUE button
    const cueBtn = document.getElementById(`cue-btn-${id}`);
    if (cueBtn) {
      cueBtn.classList.toggle('active', ch.cue_enabled);
    }
  }

  /**
   * Update an EQ bar. value: -1.0 to 1.0
   */
  function updateEQ(elementId, value) {
    const bar = document.getElementById(elementId);
    if (!bar) return;

    const fill = bar.querySelector('.eq-fill');
    const pct = Math.abs(value) * 50; // 50% max height from center

    if (value >= 0) {
      // Boost: fill upward from center
      bar.classList.remove('cut');
      fill.style.height = `${pct}%`;
      fill.style.bottom = '50%';
      fill.style.top = '';
    } else {
      // Cut: fill downward from center
      bar.classList.add('cut');
      fill.style.height = `${pct}%`;
      fill.style.top = '50%';
      fill.style.bottom = '';
    }
  }

  return { init, update };
})();
