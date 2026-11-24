// Fetch wrapper + endpoint helpers. Errors throw Error(server message); 401 flips auth.needed.
import { auth } from './auth.svelte.js';

export async function api(path, opts = {}) {
  const { body, headers, ...rest } = opts;
  const init = { credentials: 'same-origin', ...rest, headers: { ...(headers || {}) } };
  if (body !== undefined) {
    init.body = JSON.stringify(body);
    init.headers['Content-Type'] = 'application/json';
  }
  let r;
  try { r = await fetch(path, init); } catch { throw new Error('Cannot reach the server'); }
  if (r.status === 401) auth.needed = true;
  if (!r.ok) {
    let msg = r.statusText || `HTTP ${r.status}`;
    try { const j = await r.json(); if (j && j.error) msg = j.error; } catch {}
    throw new Error(msg);
  }
  if (r.status === 204) return null;
  const ct = r.headers.get('content-type') || '';
  return ct.includes('json') ? r.json() : r.text();
}

const qs = (o) => {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(o || {})) if (v !== undefined && v !== null && v !== '' && v !== false) p.set(k, v === true ? '1' : v);
  const s = p.toString();
  return s ? `?${s}` : '';
};
const enc = (p) => p.split('/').map(encodeURIComponent).join('/');

export const getConfig = () => api('/api/config');
export const listSkills = () => api('/api/skills');
export const getSkill = (name) => api(`/api/skills/${encodeURIComponent(name)}`);
export const fileUrl = (name, path) => `/api/skills/${encodeURIComponent(name)}/files/${enc(path)}`;
export const getStats = (since) => api(`/api/stats${qs({ since })}`);
export const getLog = (params) => api(`/api/log${qs(params)}`);
export const clearLog = (before) => api(`/api/log${qs({ before })}`, { method: 'DELETE', headers: { 'X-Mimir': '1' } });
export const match = ({ prompt, k, min, rerank, lexical }) =>
  api('/match', { method: 'POST', body: { prompt, k, min, rerank, lexical, log: false } });
export const login = (token) => api('/api/login', { method: 'POST', body: { token } });
export const logout = () => api('/api/logout', { method: 'POST' });
