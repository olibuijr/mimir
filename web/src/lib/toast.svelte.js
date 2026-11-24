// toast(message, tone='neutral'|'good'|'warn'): shows a transient message in <Toast/>.
export const toasts = $state([]);
let n = 0;
export function toast(message, tone = 'neutral', ms = 3200) {
  const id = ++n;
  toasts.push({ id, message, tone });
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, ms);
}
