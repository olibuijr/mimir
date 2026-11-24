// Tiny line diff (LCS). diffLines(a, b) -> [{t: ' '|'+'|'-', s}] or null when too large.
export const MAX_LINES = 2000;

export function diffLines(oldText, newText) {
  const a = (oldText || '').split('\n'), b = (newText || '').split('\n');
  if (a.length > MAX_LINES || b.length > MAX_LINES) return null;
  let lo = 0;
  while (lo < a.length && lo < b.length && a[lo] === b[lo]) lo++;
  let ea = a.length, eb = b.length;
  while (ea > lo && eb > lo && a[ea - 1] === b[eb - 1]) { ea--; eb--; }
  const x = a.slice(lo, ea), y = b.slice(lo, eb);
  const n = x.length, m = y.length, w = m + 1;
  const L = new Uint16Array((n + 1) * w);
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      L[i * w + j] = x[i] === y[j] ? L[(i + 1) * w + j + 1] + 1 : Math.max(L[(i + 1) * w + j], L[i * w + j + 1]);
  const out = [];
  for (let i = 0; i < lo; i++) out.push({ t: ' ', s: a[i] });
  let i = 0, j = 0;
  while (i < n && j < m) {
    if (x[i] === y[j]) { out.push({ t: ' ', s: x[i] }); i++; j++; }
    else if (L[(i + 1) * w + j] >= L[i * w + j + 1]) out.push({ t: '-', s: x[i++] });
    else out.push({ t: '+', s: y[j++] });
  }
  while (i < n) out.push({ t: '-', s: x[i++] });
  while (j < m) out.push({ t: '+', s: y[j++] });
  for (let k = ea; k < a.length; k++) out.push({ t: ' ', s: a[k] });
  return out;
}
