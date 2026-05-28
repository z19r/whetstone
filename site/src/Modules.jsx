/* global React */

// ============================================================
// MODULES — three deep cards: Headroom / RTK / Memory
// ============================================================
function Modules() {
  return (
    <section className="section ws-wrap" id="modules">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">01 · MODULES</div>
        <h2>Three layers,<br />one binary.</h2>
      </div>

      <div className="modules-grid">
        {/* HEADROOM */}
        <div className="module">
          <div className="head">
            <div className="eyebrow">// MODULE · 01</div>
            <div className="glyph">↯</div>
          </div>
          <div className="title">HEADROOM</div>
          <div className="desc">
            HTTP proxy between your AI tool and the LLM provider. Multi-stage compression
            pipeline runs before every API call.
          </div>
          <ul>
            <li>Cache alignment + content routing</li>
            <li>Statistical JSON compression</li>
            <li>AST-aware code compression</li>
            <li>Score-based message dropping</li>
            <li>Optional LLMLingua (ML-based) mode</li>
          </ul>
          <div className="stat">
            50–90<span className="unit">% context saved</span>
          </div>
        </div>

        {/* RTK */}
        <div className="module mag">
          <div className="head">
            <div className="eyebrow">// MODULE · 02</div>
            <div className="glyph">→</div>
          </div>
          <div className="title">RTK</div>
          <div className="desc">
            Tool-output compression hook. Rewrites every Bash call through a PreToolUse hook
            so output is squeezed <em>before</em> it enters the context window.
          </div>
          <ul>
            <li>Git: <code className="ws-icode">status</code>, <code className="ws-icode">log</code>, <code className="ws-icode">diff</code></li>
            <li>Test runners: cargo, pytest, vitest, go test</li>
            <li>Build / lint: tsc, eslint, cargo build</li>
            <li>File ops: <code className="ws-icode">ls</code>, <code className="ws-icode">grep</code>, <code className="ws-icode">read</code>, <code className="ws-icode">find</code></li>
          </ul>
          <div className="stat">
            60–90<span className="unit">% per call</span>
          </div>
        </div>

        {/* MEMORY */}
        <div className="module stack">
          <div className="head">
            <div className="eyebrow">// MODULE · 03</div>
            <div className="glyph">∞</div>
          </div>
          <div className="title">MEMORY</div>
          <div className="desc">
            Persistent project context across sessions, with bundled skills and hooks for
            pre-push checks and post-commit cleanup.
          </div>
          <ul>
            <li>ICM — embedded SQLite, zero deps</li>
            <li>AutoMem — graph via FalkorDB + Qdrant</li>
            <li>20 skills · 5 hooks · 8 rules</li>
            <li>SessionStart / Stop lifecycle</li>
          </ul>
          <div className="stat">
            2<span className="unit">providers · pick one</span>
          </div>
        </div>
      </div>
    </section>
  );
}

Object.assign(window, { Modules });
