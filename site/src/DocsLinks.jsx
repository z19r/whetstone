/* global React */

// ============================================================
// DOCS GRID — anchor links to documentation pages
// ============================================================
const DOCS = [
  {
    eyebrow: '// 01 · GETTING STARTED',
    title: 'Installation',
    desc: 'Prereqs (Python 3.10+, git, curl, uv). One-liner and from-source paths. New project vs. existing project.',
    href: 'https://github.com/z19r/whetstone/blob/main/docs/install.md',
  },
  {
    eyebrow: '// 02 · REFERENCE',
    title: 'CLI Reference',
    desc: 'Every command and flag. RTK quick reference for git, test, build, lint and file operations.',
    href: 'https://github.com/z19r/whetstone/blob/main/docs/cli-reference.md',
  },
  {
    eyebrow: '// 03 · INTEGRATION',
    title: 'Editor Setup',
    desc: 'Claude Code, Cursor, Copilot, Windsurf, Cline, Aider, Codex, Gemini CLI, OpenCode + the compatibility matrix.',
    href: 'https://github.com/z19r/whetstone/blob/main/docs/editors.md',
  },
  {
    eyebrow: '// 04 · OPS',
    title: 'Headroom as Service',
    desc: 'systemd unit, launchd plist, and any-platform nohup. Always-on proxy without re-launching per session.',
    href: 'https://github.com/z19r/whetstone/blob/main/docs/headroom-service.md',
  },
  {
    eyebrow: '// 05 · CONFIG',
    title: 'Configuration',
    desc: 'Global vs per-project config files, environment variables, --headroom-extras, --backend options.',
    href: 'https://github.com/z19r/whetstone/blob/main/docs/configuration.md',
  },
  {
    eyebrow: '// 06 · SUPPORT',
    title: 'Troubleshooting',
    desc: 'Common issues, RTK collisions, uninstall path, manual removal. Restore from the timestamped backup.',
    href: 'https://github.com/z19r/whetstone/blob/main/docs/troubleshooting.md',
  },
];

function DocsLinks() {
  return (
    <section className="section ws-wrap" id="docs">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">09 · DOCS</div>
        <h2>Read the fucking manual.</h2>
      </div>

      <div className="docs-grid">
        {DOCS.map((d) => (
          <a className="doc-card" key={d.title} href={d.href} target="_blank" rel="noreferrer">
            <div className="eyebrow">{d.eyebrow}</div>
            <div className="title">{d.title}</div>
            <div className="desc">{d.desc}</div>
            <div className="more">READ <span>→</span></div>
          </a>
        ))}
      </div>
    </section>
  );
}

Object.assign(window, { DocsLinks });
