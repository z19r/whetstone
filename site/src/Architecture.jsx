/* global React */

// ============================================================
// ARCHITECTURE — diagram with lanes + arrows
// ============================================================
function ArchitectureDiagram() {
  return (
    <div className="arch">
      <div className="col">
        <div className="node user">
          <div className="tag">// CLIENT</div>
          <div className="name">CLAUDE CODE</div>
          <div className="desc">Cursor · Copilot<br />Aider · Codex · Gemini</div>
        </div>
      </div>

      <div className="right">
        <div className="rail">
          <div className="lane rtk">
            <div>
              <div className="ltag">[RTK HOOK] · PRETOOLUSE</div>
              <div className="desc">rewrites bash → rtk &lt;cmd&gt; before output enters context</div>
            </div>
            <div className="gain">−90%</div>
          </div>
          <div className="lane hr">
            <div>
              <div className="ltag">[HEADROOM PROXY] · :8787</div>
              <div className="desc">cache · route · json · ast · drop-by-score</div>
            </div>
            <div className="gain">−80%</div>
          </div>
          <div className="lane mem">
            <div>
              <div className="ltag">[MEMORY] · ICM / AUTOMEM</div>
              <div className="desc">sessionstart · stop · pre-push · post-commit</div>
            </div>
            <div className="gain">∞ recall</div>
          </div>
        </div>

        <div className="arrow-col">
          <span className="arrow">→</span>
          <span className="arrow">→</span>
          <span className="arrow">→</span>
        </div>

        <div className="rail" style={{ justifyContent: 'center' }}>
          <div className="node api">
            <div className="tag">// LLM</div>
            <div className="name">ANTHROPIC API</div>
            <div className="desc">opus · sonnet · haiku<br />(or bedrock / vertex / azure)</div>
          </div>
        </div>
      </div>
    </div>
  );
}

function Architecture() {
  return (
    <section className="section ws-wrap" id="architecture">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">05 · ARCHITECTURE</div>
        <h2>Where it sits<br />in your stack.</h2>
      </div>

      <ArchitectureDiagram />

      <div className="ws-callout ws-callout--tip" style={{ marginTop: 'var(--s-6)' }}>
        <div className="ws-callout-mark">↯</div>
        <div className="ws-callout-body">
          <h4>// SAVINGS COMPOUND</h4>
          <p>RTK shrinks each tool output <em>before</em> it ever enters the context window. Headroom then compresses the resulting context <em>before</em> it hits the API. Memory recalls relevant context so you don't have to re-explain. The three stack.</p>
        </div>
      </div>
    </section>
  );
}

Object.assign(window, { Architecture, ArchitectureDiagram });
