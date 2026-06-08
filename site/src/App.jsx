/* global React, Nav, Hero, Modules, InstallTerminal, CompressionDemo, Numbers, Architecture, Editors, Releases, FAQ, DocsLinks, Footer */

// ============================================================
// TWEAK DEFAULTS — wrapped in EDITMODE markers for persistence
// ============================================================
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "auto",
  "heroVariant": "display",
  "showMarquee": false,
  "showArchitecture": true,
  "headlineAccent": "acid"
}/*EDITMODE-END*/;

// ============================================================
// APP — top-level composition
// ============================================================
function App() {
  const t = TWEAK_DEFAULTS;

  // theme persistence — sync to <html data-theme="…">
  // Priority: explicit user choice (localStorage) > OS preference.
  const getSystemTheme = () =>
    (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches)
      ? 'dark' : 'light';

  const [theme, setTheme] = React.useState(() => {
    try {
      const saved = localStorage.getItem('ws-theme');
      if (saved === 'light' || saved === 'dark') return saved;
    } catch (_) {}
    return getSystemTheme();
  });
  const [hasExplicitChoice, setHasExplicitChoice] = React.useState(() => {
    try { return !!localStorage.getItem('ws-theme'); } catch (_) { return false; }
  });

  // Apply theme to <html>
  React.useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  // Track OS preference changes — follow them unless the user has chosen explicitly
  React.useEffect(() => {
    if (hasExplicitChoice) return;
    const mq = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)');
    if (!mq) return;
    const onChange = (e) => setTheme(e.matches ? 'dark' : 'light');
    mq.addEventListener ? mq.addEventListener('change', onChange) : mq.addListener(onChange);
    return () => {
      mq.removeEventListener ? mq.removeEventListener('change', onChange) : mq.removeListener(onChange);
    };
  }, [hasExplicitChoice]);

  const choose = (next) => {
    setTheme(next);
    setHasExplicitChoice(true);
    try { localStorage.setItem('ws-theme', next); } catch (_) {}
  };

  const onToggleTheme = () => {
    const next = theme === 'light' ? 'dark' : 'light';
    choose(next);
  };

  // Headline accent CSS var override (for "headlineAccent" tweak)
  React.useEffect(() => {
    const root = document.documentElement;
    const map = {
      acid:     'var(--c-acid)',
      magenta:  'var(--c-magenta)',
      royal:    'var(--c-royal)',
    };
    root.style.setProperty('--headline-accent', map[t.headlineAccent] || map.acid);
  }, [t.headlineAccent]);

  return (
    <React.Fragment>
      <div className="page-grid"></div>

      <Nav theme={theme} onToggleTheme={onToggleTheme} />

      <main>
        <Hero variant={t.heroVariant} />

        {t.showMarquee && (
          <div className="marquee" aria-hidden="true">
            <div className="marquee-track">
              <span>RUST 2021 <span className="sep">·</span></span>
              <span>MIT <span className="sep">·</span></span>
              <span>HEADROOM · RTK · MEMORY <span className="sep">·</span></span>
              <span>ONE BINARY <span className="sep">·</span></span>
              <span>IDEMPOTENT SETUP <span className="sep">·</span></span>
              <span>RTK CARGO TEST 4,800 → 11 <span className="sep">·</span></span>
              <span>v3.1.0 <span className="sep">·</span></span>
              {/* duplicate for seamless loop */}
              <span>RUST 2021 <span className="sep">·</span></span>
              <span>MIT <span className="sep">·</span></span>
              <span>HEADROOM · RTK · MEMORY <span className="sep">·</span></span>
              <span>ONE BINARY <span className="sep">·</span></span>
              <span>IDEMPOTENT SETUP <span className="sep">·</span></span>
              <span>RTK CARGO TEST 4,800 → 11 <span className="sep">·</span></span>
              <span>v3.1.0 <span className="sep">·</span></span>
            </div>
          </div>
        )}

        <Modules />
        <InstallTerminal />
        <CompressionDemo />
        <Numbers />
        {t.showArchitecture && t.heroVariant !== 'arch' && <Architecture />}
        <Editors />
        <Releases />
        <FAQ />
        <DocsLinks />
      </main>

      <Footer />
    </React.Fragment>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
