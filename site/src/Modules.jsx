/* global React */

// ============================================================
// MODULES — three deep cards: Headroom / RTK / Memory
// ============================================================
function Modules() {
  return (
    <section className="section ws-wrap" id="modules">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">01 · MODULES</div>
        <h2>Whetstone installs,<br />upstream compresses.</h2>
      </div>

      <div className="modules-grid">
        {/* HEADROOM */}
        <div className="module">
          <div className="head">
            <div className="eyebrow">// UPSTREAM · 01</div>
            <div className="glyph">↯</div>
          </div>
          <div className="title">HEADROOM</div>
          <div className="desc">
            HTTP proxy between your AI tool and the LLM provider. Whetstone installs it
            via <code className="ws-icode">uv</code>, version-pins it, and points
            {' '}<code className="ws-icode">ANTHROPIC_BASE_URL</code> at it.
          </div>
          <ul>
            <li>Cache alignment + content routing</li>
            <li>Statistical JSON / AST code compression</li>
            <li>Score-based message dropping</li>
            <li>Optional LLMLingua (ML-based) mode</li>
            <li>Stats at <code className="ws-icode">localhost:8787/stats</code></li>
          </ul>
          <div className="stat">
            up to 90<span className="unit">% context (Headroom)</span>
          </div>
        </div>

        {/* RTK */}
        <div className="module mag">
          <div className="head">
            <div className="eyebrow">// UPSTREAM · 02</div>
            <div className="glyph">→</div>
          </div>
          <div className="title">RTK</div>
          <div className="desc">
            Bash-output compression hook. Rewrites <strong>Bash tool calls only</strong>
            through a PreToolUse hook — Claude Code&apos;s native Read/Grep/Glob bypass it.
            Run <code className="ws-icode">rtk gain</code> for real savings,
            {' '}<code className="ws-icode">rtk discover</code> for missed wins.
          </div>
          <ul>
            <li>Git: <code className="ws-icode">status</code>, <code className="ws-icode">log</code>, <code className="ws-icode">diff</code></li>
            <li>Test runners: cargo, pytest, vitest, go test</li>
            <li>Build / lint: tsc, eslint, cargo build</li>
            <li>File ops: <code className="ws-icode">ls</code>, <code className="ws-icode">grep</code>, <code className="ws-icode">find</code></li>
          </ul>
          <div className="stat">
            net cost can rise<span className="unit">audit with rtk gain</span>
          </div>
        </div>

        {/* WHETSTONE */}
        <div className="module stack">
          <div className="head">
            <div className="eyebrow">// THE GLUE · 03</div>
            <div className="glyph">∞</div>
          </div>
          <div className="title">WHETSTONE</div>
          <div className="desc">
            Single Rust binary that installs the two above plus ICM, runs an idempotent
            setup, ships <code className="ws-icode">migrate</code>,
            {' '}<code className="ws-icode">doctor</code>, and
            {' '}<code className="ws-icode">update</code>, and tracks tool versions in a
            per-project manifest.
          </div>
          <ul>
            <li>ICM via <code className="ws-icode">icm init --mode standard</code></li>
            <li>v2 → v3 migration with <code className="ws-icode">--rollback</code></li>
            <li>Version dashboard: <code className="ws-icode">whetstone doctor</code></li>
            <li>Release automation: <code className="ws-icode">whetstone release</code></li>
          </ul>
          <div className="stat">
            1<span className="unit">binary · 0 runtime deps</span>
          </div>
        </div>
      </div>
    </section>
  );
}

Object.assign(window, { Modules });
