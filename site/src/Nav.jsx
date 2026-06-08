/* global React */

// ============================================================
// NAV — sticky top with theme toggle
// ============================================================
function WSMark({ size = 32 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 200 200" fill="none" aria-hidden="true">
      <rect x="6" y="6" width="188" height="188" fill="var(--mark-bg, #110628)" stroke="var(--mark-fg, #F5F2E8)" strokeWidth="4" />
      <path d="M40 50 L70 150" stroke="var(--mark-fg, #F5F2E8)" strokeWidth="14" strokeLinecap="square" />
      <path d="M70 150 L100 70" stroke="var(--mark-fg, #F5F2E8)" strokeWidth="14" strokeLinecap="square" />
      <path d="M100 70 L130 150" stroke="#D6FF2E" strokeWidth="14" strokeLinecap="square" />
      <path d="M130 150 L160 50" stroke="var(--mark-fg, #F5F2E8)" strokeWidth="14" strokeLinecap="square" />
      <rect x="40" y="160" width="120" height="4" fill="#FF2E93" />
    </svg>
  );
}

function Wordmark({ size = 22 }) {
  return (
    <span className="ws-wordmark" style={{ fontSize: size }}>
      <span className="s1">/</span><span>/WHET</span><span className="dot">·</span><span>STONE</span>
    </span>
  );
}

function Nav({ theme, onToggleTheme }) {
  const sections = [
    { id: 'modules',  label: '01 · MODULES' },
    { id: 'install',  label: '02 · INSTALL' },
    { id: 'numbers',  label: '03 · NUMBERS' },
    { id: 'editors',  label: '04 · EDITORS' },
    { id: 'releases', label: '05 · RELEASES' },
    { id: 'faq',      label: '06 · FAQ' },
  ];

  const isLight = theme === 'light';

  return (
    <nav className="ws-nav">
      <a href="#top" className="ws-nav-mark" style={{
        '--mark-bg': isLight ? '#F5F2E8' : '#110628',
        '--mark-fg': isLight ? '#07030E' : '#F5F2E8',
      }}>
        <WSMark size={32} />
        <Wordmark size={20} />
      </a>

      <div className="ws-nav-links">
        {sections.map((s) => (
          <a key={s.id} href={'#' + s.id} className="ws-nav-link">{s.label}</a>
        ))}
      </div>

      <div className="ws-nav-right">
        <span className="ws-badge ws-badge--acid"><span className="pulse"></span>v3.1.1</span>
        <button className="theme-toggle" onClick={onToggleTheme} aria-label="Toggle theme">
          <span className="dot"></span>
          {isLight ? 'LIGHT' : 'NIGHT'}
        </button>
        <a className="ws-btn ws-btn--sm ws-btn--ghost" href="https://github.com/z19r/whetstone" target="_blank" rel="noreferrer">GITHUB ↗</a>
      </div>
    </nav>
  );
}

Object.assign(window, { Nav, WSMark, Wordmark });
