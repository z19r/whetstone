/* global React */

// ============================================================
// FAQ — answers grounded in the actual docs
// ============================================================
const FAQS = [
  {
    q: 'Do I need all three modules?',
    a: (
      <>
        <p>No. <code>whetstone setup</code> prompts for a memory provider (ICM, AutoMem, or
        Skip) and <code>--headroom-extras none</code> ships Headroom base-only. RTK and
        Headroom run independently; you can disable either without touching the other.</p>
      </>
    ),
  },
  {
    q: 'Will this overwrite my existing ~/.claude/settings.json?',
    a: (
      <>
        <p>Setup merges into an existing <code>settings.json</code>, not overwrites. The
        file is backed up with a timestamp before any change. Hooks are registered with
        absolute paths so they survive shell state changes.</p>
        <p>If your project already has a <code>.claude/</code> directory, your existing
        skills, rules and <code>CLAUDE.md</code> are preserved &mdash; only
        <code>.claude/skills/</code> is added.</p>
      </>
    ),
  },
  {
    q: 'What happens if I already have a binary called rtk?',
    a: (
      <>
        <p>Whetstone detects the collision with <em>Rust Type Kit</em> v0.2.x at install
        and stops. It offers three resolutions: reorder <code>$PATH</code> so
        <code>~/.local/bin</code> wins, rename one of the binaries, or skip RTK.</p>
        <p>Nothing is overwritten without explicit consent.</p>
      </>
    ),
  },
  {
    q: 'How does Headroom compress without losing fidelity?',
    a: (
      <>
        <p>Five stages: cache alignment, content routing, statistical JSON compression,
        AST-aware code compression, and score-based message dropping. Optional
        <code>--llmlingua</code> adds   an ML pass at ~2 GB model download.</p>
        <p>Benchmarks: <strong>97% accuracy at 19% tokens</strong> on SQuAD v2.</p>
      </>
    ),
  },
  {
    q: 'Which editors are supported?',
    a: (
      <>
        <p>Claude Code (CLI + VS Code + JetBrains) is the primary target &mdash; full
        stack. Cursor, Copilot, Windsurf, Cline, Aider, Codex, Gemini CLI and OpenCode get
        partial support: RTK and Headroom work in some configuration; Memory hooks rely on
        Claude Code's lifecycle and don't port.</p>
        <p>See the matrix above for per-feature support.</p>
      </>
    ),
  },
  {
    q: 'ICM or AutoMem?',
    a: (
      <>
        <p><strong>ICM</strong> is an embedded SQLite database with zero runtime
        dependencies &mdash; it's the default for new projects. <strong>AutoMem</strong>
        is a graph-backed provider using FalkorDB + Qdrant; richer retrieval, but you run
        two extra services.</p>
      </>
    ),
  },
  {
    q: 'Does the proxy phone home?',
    a: (
      <>
        <p>No. Headroom listens on <code>127.0.0.1:8787</code> by default and forwards to
        the LLM provider you configure (<code>anthropic</code>, <code>bedrock</code>,
        <code>vertex_ai</code>, <code>azure</code>, or <code>openrouter</code>). Logs are
        opt-in via <code>--log-file</code> and stay on disk.</p>
      </>
    ),
  },
  {
    q: 'Uninstall path?',
    a: (
      <>
        <p><code>whetstone uninstall</code> walks every component interactively. It
        restores <code>~/.claude/settings.json</code> from the timestamped backup, removes
        binaries from <code>~/.local/bin</code>, and leaves your project memory database
        untouched unless you say so.</p>
      </>
    ),
  },
];

function FAQItem({ q, a, idx }) {
  const [open, setOpen] = React.useState(idx === 0);
  return (
    <div className={'faq-item' + (open ? ' is-open' : '')}>
      <button className="faq-q" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <span className="num">// {String(idx + 1).padStart(2, '0')}</span>
        <span>{q}</span>
        <span className="glyph">{open ? '×' : '+'}</span>
      </button>
      <div className="faq-a">{a}</div>
    </div>
  );
}

function FAQ() {
  return (
    <section className="section ws-wrap" id="faq">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">08 · FAQ</div>
        <h2>What people<br />usually ask.</h2>
      </div>

      <div className="faq-list">
        {FAQS.map((f, i) => <FAQItem key={i} idx={i} q={f.q} a={f.a} />)}
      </div>
    </section>
  );
}

Object.assign(window, { FAQ });
