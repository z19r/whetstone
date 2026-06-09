/* global React */

// ============================================================
// EDITORS — compatibility matrix
// ============================================================
const EDITORS = [
  { id: 'cc',       name: 'CLAUDE CODE',  badge: 'PRIMARY' },
  { id: 'cursor',   name: 'CURSOR' },
  { id: 'copilot',  name: 'GH COPILOT' },
  { id: 'windsurf', name: 'WINDSURF' },
  { id: 'cline',    name: 'CLINE' },
  { id: 'aider',    name: 'AIDER' },
  { id: 'codex',    name: 'OPENAI CODEX' },
  { id: 'gemini',   name: 'GEMINI CLI' },
];

// row × editor matrix — labels/subs from metadata, cells define per-editor support
const META_MODULES = window.WHETSTONE_META.modules;
const CELLS_BY_ID = {
  'headroom':     ['wrap', 'url', 'sdk', 'env', 'set', 'wrap', 'wrap', 'env'],
  'headroom-mcp': ['install', 'manual', '—', '—', '—', '—', '—', '—'],
  'rtk-hooks':    ['hook', 'hook', 'hook', 'rules', 'rules', 'manual', 'instr', 'hook'],
  'rtk-scope':    ['global', 'global', 'global', 'project', 'project', '—', 'global', 'global'],
  'icm-memory':   ['full', '—', '—', '—', '—', '—', '—', '—'],
  'icm-hooks':    ['full', '—', '—', '—', '—', '—', '—', '—'],
  'provider':     ['full', '—', '—', '—', '—', '—', '—', '—'],
};
const ROWS = META_MODULES.map(function (m) {
  return { label: m.label, sub: m.sub, cells: CELLS_BY_ID[m.id] || [] };
});

const TIER = {
  'full':    { cls: 'cell-yes cell-bg-yes',  glyph: '●' },
  'wrap':    { cls: 'cell-yes cell-bg-yes',  glyph: 'wrap' },
  'install': { cls: 'cell-yes cell-bg-yes',  glyph: 'install' },
  'hook':    { cls: 'cell-yes cell-bg-yes',  glyph: '●' },
  'global':  { cls: 'cell-yes cell-bg-yes',  glyph: 'global' },
  'project': { cls: 'cell-part cell-bg-part', glyph: 'project' },
  'rules':   { cls: 'cell-part cell-bg-part', glyph: 'rules' },
  'instr':   { cls: 'cell-part cell-bg-part', glyph: 'instr' },
  'manual':  { cls: 'cell-part cell-bg-part', glyph: 'manual' },
  'sdk':     { cls: 'cell-part cell-bg-part', glyph: 'sdk' },
  'env':     { cls: 'cell-part cell-bg-part', glyph: 'env' },
  'set':     { cls: 'cell-part cell-bg-part', glyph: 'settings' },
  'url':     { cls: 'cell-part cell-bg-part', glyph: 'url' },
  '—':       { cls: 'cell-no',                glyph: '—' },
};

function Editors() {
  return (
    <section className="section ws-wrap" id="editors">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">06 · COMPATIBILITY</div>
        <h2>Works with what<br />you already run.</h2>
      </div>

      <div className="matrix-wrap">
        <table className="matrix">
          <thead>
            <tr>
              <th>FEATURE</th>
              {EDITORS.map((e) => (
                <th key={e.id}>
                  <div>{e.name}</div>
                  {e.badge && (
                    <div style={{
                      fontSize: 9, color: 'var(--c-magenta)', marginTop: 4,
                    }}>· {e.badge} ·</div>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {ROWS.map((row) => (
              <tr key={row.label}>
                <th>
                  <div>{row.label}</div>
                  <div style={{
                    fontFamily: 'var(--f-mono)', fontSize: 11,
                    color: 'var(--fg-dim)', letterSpacing: '0.04em',
                    textTransform: 'none', marginTop: 2, fontWeight: 400,
                  }}>{row.sub}</div>
                </th>
                {row.cells.map((c, i) => {
                  const t = TIER[c] || TIER['—'];
                  return <td key={i} className={t.cls}>{t.glyph}</td>;
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div style={{
        display: 'flex', gap: 'var(--s-4)', flexWrap: 'wrap',
        marginTop: 'var(--s-4)', fontFamily: 'var(--f-mono)', fontSize: 12,
        color: 'var(--fg-dim)', letterSpacing: '0.1em', textTransform: 'uppercase',
      }}>
        <span><span className="cell-yes" style={{ marginRight: 6 }}>●</span> FULL</span>
        <span><span className="cell-part" style={{ marginRight: 6 }}>●</span> PARTIAL · CONFIG REQUIRED</span>
        <span><span className="cell-no" style={{ marginRight: 6 }}>—</span> NOT SUPPORTED</span>
        <span style={{ marginLeft: 'auto', opacity: 0.7 }}>SEE DOCS/EDITORS FOR PER-EDITOR SETUP</span>
      </div>
    </section>
  );
}

Object.assign(window, { Editors });
