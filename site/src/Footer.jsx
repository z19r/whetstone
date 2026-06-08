/* global React */

// ============================================================
// FOOTER — release banner + nav + meta + watermark
// ============================================================
function Footer() {
  return (
    <footer className="footer">
      <div className="watermark">/\/\</div>

      <div className="ws-wrap" style={{ position: 'relative', zIndex: 1 }}>

        <div className="ws-release-banner" style={{ marginBottom: 'var(--s-7)' }}>
          <span className="ver">v3.0.0</span>
          <div>
            <div className="title">
              <span className="acid">SHIPPED</span> · idempotent setup, hardened RTK collision detection
            </div>
            <div className="sub">2026-05-24 · whetstone-cli · MIT · sha 9c4e8a1</div>
          </div>
          <a className="ws-btn ws-btn--sm ws-btn--primary" href="https://github.com/z19r/whetstone/releases" target="_blank" rel="noreferrer">
            RELEASE NOTES <span className="glyph">→</span>
          </a>
        </div>

        <div className="row">
          <div className="col footer-mark">
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <Wordmark size={28} />
            </div>
            <p>
              Token optimization and project memory for AI coding assistants. A single
              Rust binary installs and configures Headroom, RTK, and persistent memory.
            </p>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <span className="ws-badge ws-badge--acid"><span className="pulse"></span>v3.0.0 · STABLE</span>
              <span className="ws-badge ws-badge--mag">MIT</span>
              <span className="ws-badge ws-badge--royal">RUST 2021</span>
            </div>
          </div>

          <div className="col">
            <h4>// PROJECT</h4>
            <ul>
              <li><a href="https://github.com/z19r/whetstone" target="_blank" rel="noreferrer">github.com/z19r/whetstone ↗</a></li>
              <li><a href="https://github.com/z19r/whetstone/issues" target="_blank" rel="noreferrer">Issues ↗</a></li>
              <li><a href="https://github.com/z19r/whetstone/releases" target="_blank" rel="noreferrer">Releases ↗</a></li>
              <li><a href="https://github.com/z19r/whetstone/blob/main/LICENSE" target="_blank" rel="noreferrer">MIT License ↗</a></li>
            </ul>
          </div>

          <div className="col">
            <h4>// DOCS</h4>
            <ul>
              <li><a href="#install">Install</a></li>
              <li><a href="#modules">Modules</a></li>
              <li><a href="#editors">Editor matrix</a></li>
              <li><a href="#architecture">Architecture</a></li>
              <li><a href="#faq">FAQ</a></li>
            </ul>
          </div>
        </div>

        <div className="meta">
          <span>// WHETSTONE · v3.0.0 · {new Date().getFullYear()}</span>
          <span>MADE IN CHICAGO, WITH 🫀 ©2026 z19r. All rights reserved.</span>
          <span>BUILT IN A SHED · NO TELEMETRY</span>
        </div>
      </div>
    </footer>
  );
}

Object.assign(window, { Footer });
