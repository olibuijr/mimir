// Shared /api/config. App loads it once and on skill/resync events; features read cfg.data.
import { getConfig } from './api.js';

export const cfg = $state({ data: null });

// The UI build this page was loaded with. After a redeploy the server reports a different one,
// and the page reloads itself, so an open tab never keeps running old code.
let bootUi = null;

export function loadConfig() {
  return getConfig().then((c) => {
    if (c.ui) {
      if (bootUi === null) bootUi = c.ui;
      else if (c.ui !== bootUi) { location.reload(); return c; }
    }
    cfg.data = c;
    return c;
  }).catch(() => cfg.data);
}
