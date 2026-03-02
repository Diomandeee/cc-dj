/* ============================================================
   cc-dj  |  WebSocket Client — auto-reconnect + exponential backoff
   ============================================================ */

const WS = (() => {
  let socket = null;
  let reconnectDelay = 500;
  const MAX_DELAY = 10000;
  const listeners = [];
  let connected = false;

  function getUrl() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${proto}//${location.host}/ws`;
  }

  function connect() {
    if (socket && (socket.readyState === WebSocket.CONNECTING || socket.readyState === WebSocket.OPEN)) {
      return;
    }

    socket = new WebSocket(getUrl());

    socket.onopen = () => {
      connected = true;
      reconnectDelay = 500;
      document.getElementById('connection-dot')?.classList.add('connected');
      dispatch({ type: '_connected' });
    };

    socket.onclose = () => {
      connected = false;
      document.getElementById('connection-dot')?.classList.remove('connected');
      dispatch({ type: '_disconnected' });
      scheduleReconnect();
    };

    socket.onerror = () => {
      socket.close();
    };

    socket.onmessage = (evt) => {
      try {
        const event = JSON.parse(evt.data);
        dispatch(event);
      } catch (e) {
        console.warn('[WS] Bad message:', e);
      }
    };
  }

  function scheduleReconnect() {
    setTimeout(() => {
      reconnectDelay = Math.min(reconnectDelay * 1.5, MAX_DELAY);
      connect();
    }, reconnectDelay);
  }

  function dispatch(event) {
    for (const fn of listeners) {
      try { fn(event); } catch (e) { console.error('[WS] listener error:', e); }
    }
  }

  function on(fn) {
    listeners.push(fn);
  }

  function send(data) {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(typeof data === 'string' ? data : JSON.stringify(data));
    }
  }

  function isConnected() {
    return connected;
  }

  return { connect, on, send, isConnected };
})();
