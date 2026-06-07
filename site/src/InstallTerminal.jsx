/* global React */

// ============================================================
// INSTALL TERMINAL — animated typing + success/error tabs
// ============================================================
const SUCCESS_LINES = [
  { t: 800,  text: '$ whetstone setup --full',           cls: 'cmd' },
  { t: 600,  text: '  ┌─ PREFLIGHT ─────────────────────────',  cls: 'rule' },
  { t: 120,  text: '   ✓ python 3.11.5',                  cls: 'ok' },
  { t: 120,  text: '   ✓ git 2.45.0',                     cls: 'ok' },
  { t: 120,  text: '   ✓ curl 8.6.0',                     cls: 'ok' },
  { t: 120,  text: '   ✓ uv 0.4.18',                      cls: 'ok' },
  { t: 120,  text: '   ✓ git repo · 47 commits ahead of origin', cls: 'ok' },
  { t: 200,  text: '  └─────────────────────────────────────',   cls: 'rule' },
  { t: 400,  text: '  ┌─ INSTALL ───────────────────────────',  cls: 'rule' },
  { t: 240,  text: '   ✓ headroom-ai[proxy,code,mcp]   → ~/.local/bin/headroom', cls: 'ok' },
  { t: 240,  text: '   ✓ rtk                            → ~/.local/bin/rtk',     cls: 'ok' },
  { t: 240,  text: '   ✓ whetstone                      → ~/.local/bin/whetstone', cls: 'ok' },
  { t: 200,  text: '  └─────────────────────────────────────',   cls: 'rule' },
  { t: 400,  text: '  ┌─ CONFIGURE ─────────────────────────',  cls: 'rule' },
  { t: 200,  text: '   ✓ ANTHROPIC_BASE_URL → ~/.bashrc',  cls: 'ok' },
  { t: 200,  text: '   ✓ rtk PreToolUse hook → ~/.claude/settings.json', cls: 'ok' },
  { t: 200,  text: '   ✓ 20 skills · 5 hooks · 8 rules → .claude/', cls: 'ok' },
  { t: 200,  text: '   ✓ memory provider: icm (sqlite)',  cls: 'ok' },
  { t: 200,  text: '  └─────────────────────────────────────',   cls: 'rule' },
  { t: 600,  text: '',                                    cls: '' },
  { t: 200,  text: '$ whetstone version',                 cls: 'cmd' },
  // Phase 2.5: pull whetstone's version from window.WHETSTONE_VERSION
  // (regenerated from VERSION by `just release`). Component versions for
  // the demo terminal are illustrative.
  { t: 200,  text: `   whetstone ${window.WHETSTONE_VERSION} · headroom 0.9.1 · rtk 1.2.0`, cls: 'dim' },
  { t: 600,  text: '$ ',                                  cls: 'cmd-end' },
];

const ERROR_LINES = [
  { t: 600, text: '$ whetstone setup',                    cls: 'cmd' },
  { t: 120, text: '   ✓ preflight ok',                    cls: 'ok' },
  { t: 220, text: '   ✗ rtk · name collides with Rust Type Kit v0.2.1', cls: 'err' },
  { t: 80,  text: '       found at: ~/.cargo/bin/rtk',    cls: 'dim' },
  { t: 80,  text: '       whetstone rtk wants: ~/.local/bin/rtk', cls: 'dim' },
];

function useTypewriter(lines, active) {
  const [shown, setShown] = React.useState(0);
  const [cursorOn, setCursorOn] = React.useState(true);

  React.useEffect(() => {
    setShown(0);
    if (!active) return;
    let cancelled = false;
    let i = 0;
    function step() {
      if (cancelled || i >= lines.length) return;
      const delay = lines[i].t;
      setTimeout(() => {
        if (cancelled) return;
        i += 1;
        setShown(i);
        step();
      }, delay);
    }
    step();
    return () => { cancelled = true; };
  }, [active, lines]);

  React.useEffect(() => {
    const id = setInterval(() => setCursorOn((c) => !c), 600);
    return () => clearInterval(id);
  }, []);

  const replay = () => setShown(0);
  return { shown, cursorOn, replay };
}

function TerminalView({ lines, active, showCursorAtEnd }) {
  const { shown, cursorOn } = useTypewriter(lines, active);
  const visible = lines.slice(0, shown);
  return (
    <pre style={{
      fontFamily: 'var(--f-mono)', fontSize: '13px', lineHeight: 1.65,
      margin: 0, color: 'var(--c-mist)',
      minHeight: '480px', whiteSpace: 'pre-wrap', wordBreak: 'break-word',
    }}>
      {visible.map((l, i) => (
        <div key={i} style={{
          color:
            l.cls === 'ok'   ? 'var(--c-acid)'    :
            l.cls === 'err'  ? 'var(--c-magenta)' :
            l.cls === 'dim'  ? 'var(--c-lilac)'   :
            l.cls === 'rule' ? 'var(--c-lilac)'   :
            l.cls === 'cmd'  ? 'var(--c-bone)'    :
            'var(--c-mist)',
          opacity: l.cls === 'dim' || l.cls === 'rule' ? 0.75 : 1,
          fontWeight: l.cls === 'cmd' || l.cls === 'cmd-end' ? 700 : 400,
        }}>
          {l.text || '\u00A0'}
          {showCursorAtEnd && i === visible.length - 1 && (
            <span style={{ color: 'var(--c-magenta)', opacity: cursorOn ? 1 : 0 }}>▌</span>
          )}
        </div>
      ))}
    </pre>
  );
}

function InstallTerminal() {
  const [branch, setBranch] = React.useState('success');
  const isSuccess = branch === 'success';
  const lines = isSuccess ? SUCCESS_LINES : ERROR_LINES;

  return (
    <section className="section ws-wrap" id="install">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">02 · INSTALL</div>
        <h2>One command.<br />Two outcomes shown.</h2>
      </div>

      <div style={{
        border: 'var(--b-med) solid var(--fg)', boxShadow: 'var(--sh-stack)',
        background: 'var(--c-void)', position: 'relative', marginBottom: 'var(--s-7)',
      }}>
        <div style={{
          position: 'absolute', top: '-14px', left: 'var(--s-5)',
          background: 'var(--c-magenta)', color: 'var(--c-bone)',
          font: 'var(--t-tag)', padding: '5px 12px', letterSpacing: '0.18em',
        }}>
          INSTALLATION TERMINAL · whetstone setup
        </div>

        {/* terminal chrome */}
        <div style={{
          display: 'flex', justifyContent: 'space-between', alignItems: 'center',
          padding: 'var(--s-3) var(--s-5)',
          borderBottom: 'var(--b-med) solid var(--c-bone)',
          background: 'var(--c-obsidian)',
        }}>
          <div style={{ display: 'flex', gap: '8px' }}>
            <div style={{ width: '12px', height: '12px', background: 'var(--c-magenta)' }}></div>
            <div style={{ width: '12px', height: '12px', background: 'var(--c-acid)' }}></div>
            <div style={{ width: '12px', height: '12px', background: 'var(--c-bone)' }}></div>
          </div>
          <div style={{
            font: 'var(--t-tag)', letterSpacing: '0.14em', textTransform: 'uppercase',
            color: 'var(--c-acid)',
          }}>~/projects/my-app · bash</div>
          <div style={{ font: 'var(--t-mono)', fontSize: '11px', color: 'var(--c-lilac)' }}>
            [RTK · LIVE]
          </div>
        </div>

        {/* branch toggle */}
        <div style={{
          display: 'flex', gap: 0,
          borderBottom: 'var(--b-med) solid var(--c-bone)',
          background: 'var(--c-void)',
        }}>
          <button
            className={'demo-tab' + (isSuccess ? ' is-active' : '')}
            onClick={() => setBranch('success')}
            style={{ flex: 1 }}
          >
            <span className="ix">✓</span>
            <span>SUCCESS BRANCH · v2.6.0</span>
          </button>
          <button
            className={'demo-tab' + (!isSuccess ? ' is-active' : '')}
            onClick={() => setBranch('error')}
            style={{ flex: 1, borderRight: 0 }}
          >
            <span className="ix">✗</span>
            <span>ERROR BRANCH · RTK COLLISION</span>
          </button>
        </div>

        {/* body */}
        <div style={{ padding: 'var(--s-6) var(--s-7)', background: 'var(--c-void)' }}>
          <div style={{
            fontSize: '11px',
            color: isSuccess ? 'var(--c-lilac)' : 'var(--c-magenta)',
            letterSpacing: '0.14em', textTransform: 'uppercase',
            marginBottom: 'var(--s-3)',
            fontFamily: 'var(--f-mono)',
          }}>
            {isSuccess ? '// WHETSTONE SETUP · v2.6.0' : '// ERROR · RTK COLLISION DETECTED'}
          </div>

          <TerminalView lines={lines} active={true} showCursorAtEnd={isSuccess} key={branch} />

          {!isSuccess && (
            <React.Fragment>
              <div className="ws-callout ws-callout--danger" style={{ margin: 'var(--s-5) 0 var(--s-4)' }}>
                <div className="ws-callout-mark">×</div>
                <div className="ws-callout-body">
                  <h4>// CONFLICT</h4>
                  <p>Both binaries can coexist if <code>~/.local/bin</code> precedes <code>~/.cargo/bin</code> in <code>$PATH</code>. Or rename one. Whetstone won't overwrite without explicit consent.</p>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 'var(--s-3)', flexWrap: 'wrap' }}>
                <button className="ws-btn ws-btn--sm ws-btn--primary">REORDER PATH</button>
                <button className="ws-btn ws-btn--sm">RENAME RTK</button>
                <button className="ws-btn ws-btn--sm ws-btn--ghost">SKIP RTK</button>
              </div>
            </React.Fragment>
          )}
        </div>
      </div>

      {/* post-install KV */}
      {isSuccess && (
        <div style={{
          border: 'var(--b-med) solid var(--fg)',
          background: 'var(--bg-card)',
          boxShadow: '4px 4px 0 0 var(--brand)',
        }}>
          <div style={{
            padding: '10px var(--s-5)',
            borderBottom: 'var(--b-med) solid var(--rule)',
            font: 'var(--t-tag)',
            letterSpacing: '0.14em', textTransform: 'uppercase',
            color: 'var(--accent)',
          }}>// POST-INSTALL STATUS</div>
          <div className="ws-kv">
            <div className="ws-kv-row"><div className="k">HEADROOM</div><div className="v">proxy :8787 · indexing</div><div className="s ok">running</div></div>
            <div className="ws-kv-row"><div className="k">RTK</div><div className="v">hook · pretooluse(bash)</div><div className="s ok">active</div></div>
            <div className="ws-kv-row"><div className="k">MEMORY</div><div className="v">icm · .claude/db/whetstone.db</div><div className="s ok">ready</div></div>
            <div className="ws-kv-row"><div className="k">SKILLS</div><div className="v">20 copied to .claude/skills/</div><div className="s ok">ok</div></div>
            <div className="ws-kv-row"><div className="k">HOOKS</div><div className="v">5 hooks merged into ~/.claude/settings.json</div><div className="s ok">ok</div></div>
            <div className="ws-kv-row"><div className="k">SHELL</div><div className="v">ANTHROPIC_BASE_URL → ~/.bashrc</div><div className="s err">action</div></div>
          </div>
        </div>
      )}
    </section>
  );
}

Object.assign(window, { InstallTerminal });
