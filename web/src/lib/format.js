// Small display formatters.
export function ago(ts) {
  if (ts == null) return 'never';
  const s = Math.max(0, Math.floor(Date.now() / 1000 - ts));
  if (s < 45) return 'just now';
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  if (s < 86400 * 45) return `${Math.round(s / 86400)}d ago`;
  return new Date(ts * 1000).toISOString().slice(0, 10);
}
export function ms(n) {
  if (n == null) return '-';
  return n < 1000 ? `${Math.round(n)} ms` : `${(n / 1000).toFixed(n < 10000 ? 2 : 1)} s`;
}
export function bytes(n) {
  if (n == null) return '-';
  if (n < 1024) return `${n} B`;
  const u = ['KB', 'MB', 'GB'];
  let i = -1;
  do { n /= 1024; i++; } while (n >= 1024 && i < u.length - 1);
  return `${n < 10 ? n.toFixed(1) : Math.round(n)} ${u[i]}`;
}
export function pct(x) {
  if (x == null || Number.isNaN(x)) return '-';
  return `${Math.round(x * 100)}%`;
}
export function score(x) {
  return x == null ? '-' : x.toFixed(2);
}
