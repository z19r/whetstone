/* global React */

// ============================================================
// FAQ — answers grounded in the actual docs
// ============================================================
const FAQS = [
  {
    q: 'Do I need all three modules?',
    a: (
      <>
        <p>No. <code>whetstone setup</code> prompts for a memory provider (ICM or Skip)
        and <code>--headroom-extras none</code> ships Headroom base-only. RTK and
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
        <code>--llmlingua</code> adds an ML pass at ~2&nbsp;GB model download.</p>
        <p>Compression numbers belong to the Headroom project, not whetstone — see the
        upstream <a href="https://pypi.org/project/headroom-ai/" target="_blank"
        rel="noreferrer">headroom-ai</a> docs and run <code>curl localhost:8787/stats</code>
        to measure your own.</p>
      </>
    ),
  },
  {
    q: 'Does RTK ever cost more tokens than it saves?',
    a: (
      <>
        <p>Yes — compression can strip context the model needed, and RTK&apos;s own
        tracker has logged a ~<strong>18% net cost-increase</strong> case. Two things
        to know:</p>
        <ul>
          <li>The whetstone hook only rewrites <strong>Bash</strong> tool calls. Claude
          Code&apos;s native <code>Read</code>, <code>Grep</code>, <code>Glob</code>, and
          file-edit tools bypass RTK entirely.</li>
          <li>Run <code>rtk gain</code> for cumulative savings, <code>rtk discover</code>
          to spot Bash commands you&apos;re not rewriting yet, and consider RTK&apos;s audit
          mode if you suspect a particular rewrite is hurting more than helping.</li>
        </ul>
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
    q: 'Which memory provider does whetstone install?',
    a: (
      <>
        <p><strong>ICM</strong> (<code>icm init --mode standard</code>) — an embedded
        SQLite store with zero runtime dependencies. Skills, hooks, and the CLI are
        configured by ICM itself; whetstone&apos;s job is to call <code>icm init</code>
        with the right flags, version-pin it in the per-project manifest, and refresh
        it on <code>whetstone update</code>.</p>
        <p>v2 shipped a second graph-backed provider (AutoMem). v3 drops it and uses
        ICM exclusively. If you&apos;re migrating from v2, <code>whetstone migrate</code>
        archives the v2 MemStack store and re-initialises against ICM.</p>
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
