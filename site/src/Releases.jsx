/* global React */

// ============================================================
// RELEASES — feed of past versions
// (Grounded in actual source-tree features — version numbers
//  reflect 2.2.2 as the source-of-truth current.)
// ============================================================
const RELEASES = [
  {
    ver: '2.2.2',
    date: '2026-05-24',
    title: 'Idempotent setup, hardened RTK collision detection',
    bullets: [
      'rtk collision: also check Homebrew prefix on macOS',
      'setup.rs: idempotent on rerun · skips installed components',
      'fix: Windows path separator in ~/.claude/settings.json merge',
      'db: session.add_insight migration v3',
      'cli: whetstone update --force flag',
    ],
    stats: [
      { v: '+217', l: 'LINES ADDED' },
      { v: '−64',  l: 'LINES REMOVED' },
      { v: '11/11', l: 'TESTS PASSING' },
    ],
  },
  {
    ver: '2.2.0',
    date: '2026-04-18',
    title: '--headroom-extras flag, MCP install path',
    bullets: [
      '--headroom-extras: all | none | proxy,code (custom)',
      'headroom mcp install reachable via `whetstone proxy mcp install`',
      'preflight: warn instead of fail on uv version < 0.4',
      'release.rs: `just release patch|minor|major` opens release PR',
    ],
    stats: [
      { v: '0.9.1', l: 'HEADROOM' },
      { v: '1.2.0', l: 'RTK' },
      { v: '20', l: 'BUNDLED SKILLS' },
    ],
  },
  {
    ver: '2.0.0',
    date: '2026-02-03',
    title: 'Rust rewrite. ~1200 Bash + ~460 Python → one binary',
    bullets: [
      'Single Rust binary replaces shell + python installer',
      'rusqlite bundled · no system SQLite dep',
      'Absolute paths in hooks (no PATH/shell-state issues)',
      'Backup-before-modify on ~/.claude/settings.json',
      'New: MemoryProvider enum (ICM · AutoMem · Skip)',
    ],
    stats: [
      { v: '−1,660', l: 'LINES SCRIPT' },
      { v: '1', l: 'STATIC BIN' },
      { v: 'MIT', l: 'LICENSE' },
    ],
  },
  {
    ver: '1.4.0',
    date: '2026-01-12',
    title: 'AutoMem provider · graph memory via FalkorDB + Qdrant',
    bullets: [
      'AutoMem: optional graph-backed memory provider',
      'ICM remains the zero-dep default (embedded SQLite)',
      'Memory hooks: pre-push secrets scan, post-commit cleanup',
      '8 bundled rules · 2 commands · 5 hooks',
    ],
    stats: [
      { v: '2', l: 'PROVIDERS' },
      { v: '+graph', l: 'MEMORY' },
      { v: '0', l: 'BREAKING' },
    ],
  },
];

function Releases() {
  return (
    <section className="section ws-wrap" id="releases">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">07 · RELEASES</div>
        <h2>Recent shipments.</h2>
      </div>

      <div className="releases">
        {RELEASES.map((r) => (
          <div className="release" key={r.ver}>
            <div className="when">
              <span className="ver-pill">v{r.ver}</span>
              <span>{r.date}</span>
              <span style={{ opacity: 0.6 }}>git tag · v{r.ver}</span>
            </div>
            <div className="body">
              <div className="title">{r.title}</div>
              <ul>
                {r.bullets.map((b, i) => <li key={i}>{b}</li>)}
              </ul>
            </div>
            <div className="stats">
              {r.stats.map((s, i) => (
                <div key={i}>
                  <b>{s.v}</b> · {s.l}
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>

      <div style={{
        marginTop: 'var(--s-5)',
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        flexWrap: 'wrap', gap: 'var(--s-3)',
      }}>
        <div style={{
          fontFamily: 'var(--f-mono)', fontSize: 12,
          color: 'var(--fg-dim)', letterSpacing: '0.14em', textTransform: 'uppercase',
        }}>// SHOWING 4 OF 12 · git log v1.0.0..HEAD</div>
        <a className="ws-btn ws-btn--sm ws-btn--ghost" href="https://github.com/z19r/whetstone/releases" target="_blank" rel="noreferrer">FULL CHANGELOG ↗</a>
      </div>
    </section>
  );
}

Object.assign(window, { Releases });
