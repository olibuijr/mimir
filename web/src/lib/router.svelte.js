// Hash router. Paths: #/ (library), #/skills/<name>, #/playground?prompt=..&file=.., #/activity.
// route.query holds the parsed query string of the hash.
export const route = $state({ name: 'library', params: {}, query: {} });

function parse() {
  const raw = (location.hash || '#/').slice(1);
  const qi = raw.indexOf('?');
  const pathPart = qi < 0 ? raw : raw.slice(0, qi);
  const query = {};
  if (qi >= 0) for (const [k, v] of new URLSearchParams(raw.slice(qi + 1))) query[k] = v;
  const parts = decodeURIComponent(pathPart).split('/').filter(Boolean);
  route.query = query;
  if (parts[0] === 'skills' && parts[1]) { route.name = 'skill'; route.params = { name: parts.slice(1).join('/') }; }
  else if (parts[0] === 'playground') { route.name = 'playground'; route.params = {}; }
  else if (parts[0] === 'activity') { route.name = 'activity'; route.params = {}; }
  else { route.name = 'library'; route.params = {}; }
}

function build(path, query) {
  const enc = path.split('/').map((s, i) => (i === 0 ? s : encodeURIComponent(s))).join('/');
  let h = '#' + (enc.startsWith('/') ? enc : '/' + enc);
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(query || {})) if (v !== undefined && v !== null && v !== '') p.set(k, v);
  const s = p.toString();
  return s ? `${h}?${s}` : h;
}

export function go(path, query) {
  const want = build(path, query);
  if (location.hash === want) parse(); else location.hash = want;
}

export function href(path, query) {
  return build(path, query);
}

parse();
addEventListener('hashchange', parse);
