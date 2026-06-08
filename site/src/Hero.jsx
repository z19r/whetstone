/* global React */

// ============================================================
// HERO — three variants: display headline / big-number / arch
// ============================================================
const CopyChip = function CopyChip({ cmd }) {
  const [copied, setCopied] = React.useState(false);
  const onCopy = () => {
    navigator.clipboard?.writeText(cmd).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    });
  };
  return (
    <div className="copy-chip">
      <span className="prompt">$</span>
      <span className="cmd">{cmd}</span>
      <button className={'copy-btn' + (copied ? ' is-copied' : '')} onClick={onCopy}>
        {copied ? 'COPIED ✓' : 'COPY'}
      </button>
    </div>
  );
};

function HeroBadges() {
  return (
    <div className="hero-badges">
      <span className="ws-badge ws-badge--acid"></span>
      <span className="ws-badge ws-badge--acid">&nbsp;&nbsp;</span>
      <span className="ws-badge ws-badge--acid">&nbsp;&nbsp;&nbsp;</span>
      <span className="ws-badge ws-badge--royal">CRATES.IO · v3.1.1</span>
      <span className="ws-badge ws-badge--mag">MIT</span>
      <span className="ws-badge ws-badge--acid">RUST 2021</span>
    </div>
  );
}

function HeroEyebrow() {
  return (
    <div className="eyebrow-row">
      <span className="bar"></span>
      <span>// WHETSTONE · v3.1.1 · MAY 24 2026</span>
    </div>
  );
}

const INSTALL_CMD = 'curl -fsSL https://whetstone.sh | bash';

// ──────────────────────────────────────────────────────────
// Variant A — display headline (default)
// ──────────────────────────────────────────────────────────
function HeroDisplay() {
  return (
    <section className="hero ws-wrap" id="top">
      <HeroEyebrow />

      <h1 className="display">
        Token optimization<br />
        for <span className="acc">AI coding</span><br />
        assistants.
      </h1>

      <p className="sub">
        One Rust binary that installs and orchestrates <b>Headroom</b> (context proxy),
        {' '}<b>RTK</b> (Bash-output hook), and <b>ICM</b> (project memory) — idempotent
        setup, version dashboard, release automation. Compression is upstream's job;
        the glue is whetstone's.
      </p>

      <div className="hero-cta">
        <CopyChip cmd={INSTALL_CMD} />
        <a className="ws-btn ws-btn--primary" href="#install">INSTALL <span className="glyph">→</span></a>
        <a className="ws-btn ws-btn--ghost" href="https://github.com/z19r/whetstone" target="_blank" rel="noreferrer">VIEW ON GITHUB</a>
      </div>

      <HeroBadges />
    </section>
  );
}

// ──────────────────────────────────────────────────────────
// Variant B — big-number compression
// ──────────────────────────────────────────────────────────
function HeroBigNumber() {
  return (
    <section className="hero ws-wrap" id="top">
      <HeroEyebrow />

      <div className="hero-bignum">
        <div className="topline">// CARGO TEST · COMPRESSED</div>
        <div className="number">
          <span>4,800</span>
          <span className="arrow">→</span>
          <span className="now">11</span>
          <span className="pct">tok</span>
        </div>
        <div className="caption">-99.8% · RTK PRETOOLUSE HOOK · ONE BINARY</div>
      </div>

      <h1 className="display" style={{ fontSize: 'clamp(36px, 5vw, 64px)', margin: '0 0 24px' }}>
        Stop burning context<br /> on <span className="acc">tool noise</span>.
      </h1>

      <p className="sub">
        Whetstone installs and orchestrates three upstream tools for Claude Code,
        Cursor and friends: <b>RTK</b> rewrites Bash output, <b>Headroom</b> proxies the
        rest of the context, <b>ICM</b> remembers what mattered. One <code>setup</code>,
        one <code>update</code>, one <code>migrate</code>.
      </p>

      <div className="hero-cta">
        <CopyChip cmd={INSTALL_CMD} />
        <a className="ws-btn ws-btn--primary" href="#install">INSTALL <span className="glyph">→</span></a>
        <a className="ws-btn ws-btn--ghost" href="https://github.com/z19r/whetstone" target="_blank" rel="noreferrer">VIEW ON GITHUB</a>
      </div>

      <HeroBadges />
    </section>
  );
}

// ──────────────────────────────────────────────────────────
// Variant C — architecture-first
// ──────────────────────────────────────────────────────────
function HeroArchitecture() {
  return (
    <section className="hero ws-wrap" id="top">
      <HeroEyebrow />

      <h1 className="display" style={{ fontSize: 'clamp(40px, 6vw, 80px)' }}>
        Three layers,<br />
        <span className="acc">one</span> binary.
      </h1>

      <p className="sub">
        A single Rust install sits between your AI coding tool and the LLM API. RTK
        compresses Bash output, Headroom proxies the context, ICM keeps project memory
        across sessions.
      </p>

      <div className="hero-cta">
        <CopyChip cmd={INSTALL_CMD} />
        <a className="ws-btn ws-btn--primary" href="#install">INSTALL <span className="glyph">→</span></a>
        <a className="ws-btn ws-btn--ghost" href="#architecture">VIEW ARCHITECTURE</a>
      </div>

      <HeroBadges />

      <div className="hero-arch">
        <ArchitectureDiagram />
      </div>
    </section>
  );
}

function Hero({ variant }) {
  if (variant === 'bignum') return <HeroBigNumber />;
  if (variant === 'arch')   return <HeroArchitecture />;
  return <HeroDisplay />;
}

Object.assign(window, { Hero, CopyChip, INSTALL_CMD });
