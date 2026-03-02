/* ============================================================
   cc-dj  |  Canvas Utilities — waveform rendering, DPI scaling
   ============================================================ */

const Canvas = (() => {
  /**
   * Scale a canvas for high-DPI displays.
   * Call once after the element is in the DOM.
   */
  function scaleForDPI(canvas) {
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    const ctx = canvas.getContext('2d');
    ctx.scale(dpr, dpr);
    // Store logical size for drawing
    canvas._w = rect.width;
    canvas._h = rect.height;
    return ctx;
  }

  /**
   * Draw a procedural waveform with a playhead.
   *
   * @param {CanvasRenderingContext2D} ctx
   * @param {number} w       Logical width
   * @param {number} h       Logical height
   * @param {number} progress 0.0-1.0 playhead position
   * @param {string} color   Waveform color (CSS)
   * @param {number} energy  0.0-1.0 amplitude scale
   * @param {boolean} playing Whether the deck is playing
   */
  function drawWaveform(ctx, w, h, progress, color, energy, playing) {
    ctx.clearRect(0, 0, w, h);

    const mid = h / 2;
    const bars = Math.floor(w / 3); // 3px per bar
    const barW = 2;
    const gap = 1;
    const playheadX = progress * w;

    // Draw bars
    for (let i = 0; i < bars; i++) {
      const x = i * (barW + gap);

      // Procedural amplitude using sin combinations (deterministic per bar)
      const t = i / bars;
      const amp = (
        Math.sin(t * 12.4 + 1.3) * 0.3 +
        Math.sin(t * 27.8 + 0.7) * 0.2 +
        Math.sin(t * 5.1 + 2.1) * 0.35 +
        0.15
      ) * energy;

      const barH = Math.max(2, amp * (h * 0.85));

      // Color: played portion is brighter
      if (x < playheadX) {
        ctx.fillStyle = color;
        ctx.globalAlpha = playing ? 0.8 : 0.4;
      } else {
        ctx.fillStyle = color;
        ctx.globalAlpha = 0.2;
      }

      ctx.fillRect(x, mid - barH / 2, barW, barH);
    }

    ctx.globalAlpha = 1;

    // Playhead line
    if (playing || progress > 0) {
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(playheadX - 1, 0, 2, h);
    }

    // Center line
    ctx.fillStyle = 'rgba(255,255,255,0.06)';
    ctx.fillRect(0, mid, w, 1);
  }

  return { scaleForDPI, drawWaveform };
})();
