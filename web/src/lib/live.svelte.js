// Live updates: one EventSource on /api/events for the whole app. Features subscribe with
// onEvent() inside an $effect and refetch or patch their data, so nothing needs a refresh.
// Events: {type:'match', row}, {type:'load', skill, ts}, {type:'skill', action, name?, version?},
// {type:'log_cleared'}, {type:'resync'} (we fell behind: refetch everything).
import { getConfig } from './api.js';

// down: disconnected for more than 5 s (drives the "Reconnecting..." banner).
export const live = $state({ connected: false, down: false });

const subs = new Set();
let es = null;
let downTimer = null;
let retryTimer = null;

function markUp() {
  clearTimeout(downTimer); downTimer = null;
  clearTimeout(retryTimer); retryTimer = null;
  live.down = false;
}

export function connect() {
  if (es) return;
  es = new EventSource('/api/events'); // same-origin, so the login cookie is sent
  es.onopen = () => {
    // After a reconnect we may have missed events: let everyone refetch.
    const wasDown = !live.connected;
    live.connected = true;
    markUp();
    if (wasDown) emit({ type: 'resync' });
  };
  es.onerror = () => {
    live.connected = false;
    downTimer ??= setTimeout(() => { live.down = true; }, 5000);
    // EventSource cannot see the status code: probe a plain endpoint. api() flips auth.needed on 401.
    getConfig().catch(() => {});
    // A failed (non-200) connection is closed for good by the browser: recreate it ourselves.
    if (es && es.readyState === 2 && !retryTimer) {
      retryTimer = setTimeout(() => { retryTimer = null; retry(); }, 3000);
    }
  };
  es.onmessage = (e) => {
    let m;
    try { m = JSON.parse(e.data); } catch { return; }
    emit(m);
  };
}

export function disconnect() {
  es?.close();
  es = null;
  live.connected = false;
  markUp();
}

/** Recreate the EventSource (Retry button). */
export function retry() {
  const wasDown = live.down;
  es?.close();
  es = null;
  clearTimeout(retryTimer); retryTimer = null;
  connect();
  live.down = wasDown && !live.connected;
}

function emit(m) {
  for (const f of subs) {
    try { f(m); } catch (err) { console.error(err); }
  }
}

/** Subscribe to server events. Returns the unsubscribe function, so `$effect(() => onEvent(f))` cleans up. */
export function onEvent(f) {
  subs.add(f);
  return () => subs.delete(f);
}

/** Collapse a burst of events into one call. */
export function debounce(fn, ms = 300) {
  let t;
  return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); };
}
