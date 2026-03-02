/* ============================================================
   cc-dj  |  Deck — rendering, waveform, BPM, beat pulse
   ============================================================ */

const Deck = (() => {
  const canvases = [null, null];
  const contexts = [null, null];
  let lastBeatInt = [0, 0];

  const DECK_COLORS = ['#ff6b35', '#3b82f6'];
  const DECK_IDS = ['a', 'b'];

  function init() {
    for (let i = 0; i < 2; i++) {
      const id = DECK_IDS[i];
      const canvas = document.getElementById(`waveform-${id}`);
      canvases[i] = canvas;
      contexts[i] = Canvas.scaleForDPI(canvas);
      // Initial empty draw
      Canvas.drawWaveform(contexts[i], canvas._w, canvas._h, 0, DECK_COLORS[i], 0.5, false);
    }

    // Handle resize
    window.addEventListener('resize', () => {
      for (let i = 0; i < 2; i++) {
        contexts[i] = Canvas.scaleForDPI(canvases[i]);
      }
    });
  }

  function formatTime(secs) {
    if (!secs || secs < 0) return '0:00';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  function update(deckIndex, deckState) {
    if (!deckState) return;

    const id = DECK_IDS[deckIndex];
    const el = document.getElementById(`deck-${id}`);

    // Track info
    const track = deckState.track;
    const titleEl = document.getElementById(`track-title-${id}`);
    const artistEl = document.getElementById(`track-artist-${id}`);

    if (track && track.title) {
      titleEl.textContent = track.title;
      artistEl.textContent = track.artist || '';
    } else {
      titleEl.textContent = '-- No Track --';
      artistEl.textContent = '';
    }

    // BPM
    const bpmEl = el.querySelector('.bpm-value');
    const bpm = deckState.bpm || 0;
    bpmEl.textContent = bpm.toFixed(1);

    // Key
    const keyEl = document.getElementById(`key-${id}`);
    keyEl.textContent = deckState.key || '--';

    // Time
    const elapsed = deckState.position_secs || 0;
    const remaining = (deckState.duration_secs || 0) - elapsed;
    document.getElementById(`time-elapsed-${id}`).textContent = formatTime(elapsed);
    const remEl = document.getElementById(`time-remaining-${id}`);
    remEl.textContent = formatTime(Math.max(0, remaining));
    remEl.classList.toggle('near-end', remaining > 0 && remaining < 30);

    // Master badge
    document.getElementById(`master-${id}`).classList.toggle('hidden', !deckState.is_master);

    // Loop indicator
    const loopEl = document.getElementById(`loop-${id}`);
    if (deckState.loop_active && deckState.loop_beats) {
      loopEl.textContent = `${deckState.loop_beats} BEATS`;
      loopEl.classList.remove('hidden');
    } else {
      loopEl.classList.add('hidden');
    }

    // Play button state
    const playBtn = el.querySelector('.btn-play');
    playBtn.classList.toggle('playing', deckState.is_playing);

    // Beat dots
    const beatPos = deckState.beat_position || 0;
    const activeDot = Math.floor(beatPos % 4);
    const dots = document.getElementById(`beat-dots-${id}`).querySelectorAll('.dot');
    dots.forEach((dot, i) => dot.classList.toggle('active', i === activeDot));

    // Waveform
    const canvas = canvases[deckIndex];
    const ctx = contexts[deckIndex];
    if (canvas && ctx) {
      const progress = deckState.duration_secs > 0
        ? deckState.position_secs / deckState.duration_secs
        : 0;
      const energy = (track && track.energy) ? track.energy / 10 : 0.5;
      Canvas.drawWaveform(ctx, canvas._w, canvas._h, progress, DECK_COLORS[deckIndex], energy, deckState.is_playing);
    }
  }

  function onBeat(beat, bpm) {
    // Trigger beat pulse on both decks
    for (let i = 0; i < 2; i++) {
      const id = DECK_IDS[i];
      const el = document.getElementById(`deck-${id}`);
      const bpmEl = el.querySelector('.bpm-value');

      // Add on-beat class briefly
      el.classList.add('on-beat');
      bpmEl.classList.add('on-beat-bpm');

      setTimeout(() => {
        el.classList.remove('on-beat');
        bpmEl.classList.remove('on-beat-bpm');
      }, 200);
    }
  }

  return { init, update, onBeat };
})();
