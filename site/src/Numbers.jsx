/* global React */

// ============================================================
// NUMBERS — animated count-up stat rail + diff hero
// ============================================================
function useCountUp(target, active, duration = 1400) {
  const [val, setVal] = React.useState(0);
  React.useEffect(() => {
    if (!active) return;
    let raf, start;
    const step = (ts) => {
      if (!start) start = ts;
      const p = Math.min(1, (ts - start) / duration);
      const eased = 1 - Math.pow(1 - p, 3);
      setVal(target * eased);
      if (p < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [target, active, duration]);
  return val;
}

function useInView(ref) {
  const [inView, setInView] = React.useState(false);
  React.useEffect(() => {
    if (!ref.current) return;
    const io = new IntersectionObserver((entries) => {
      entries.forEach((e) => { if (e.isIntersecting) setInView(true); });
    }, { threshold: 0.2 });
    io.observe(ref.current);
    return () => io.disconnect();
  }, [ref]);
  return inView;
}

function StatCell({ ix, value, format, unit, label, sub, color }) {
  const ref = React.useRef(null);
  const inView = useInView(ref);
  const v = useCountUp(value, inView);
  return (
    <div className="stat-x" ref={ref}>
      <div className="ix">{ix}</div>
      <div className={'num ' + (color || '')}>
        {format(v)}<span className="u">{unit}</span>
      </div>
      <div className="lbl">{label}</div>
      {sub && <div className="sub">{sub}</div>}
    </div>
  );
}

function Numbers() {
  return (
    <section className="section ws-wrap" id="numbers">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">04 · NUMBERS</div>
        <h2>What the glue<br />is actually doing.</h2>
      </div>

      {/* big diff — example RTK run on this repo */}
      <div className="ws-diff" style={{ marginBottom: 'var(--s-7)' }}>
        <div className="ws-diff-cell is-in">
          <div className="label">IN · CARGO TEST</div>
          <div className="value">4,800</div>
          <div className="unit">TOKENS</div>
        </div>
        <div className="ws-diff-arrow">
          <span>→</span>
          <span className="savings">-99.8%</span>
        </div>
        <div className="ws-diff-cell is-out">
          <div className="label">OUT · RTK REWRITE</div>
          <div className="value">11</div>
          <div className="unit">TOKENS</div>
        </div>
      </div>
      <div style={{
        fontFamily: 'var(--f-mono)', fontSize: 11, color: 'var(--fg-dim)',
        letterSpacing: '0.04em', textAlign: 'center',
        marginTop: 'calc(-1 * var(--s-6))', marginBottom: 'var(--s-7)',
      }}>
        ONE EXAMPLE RUN · WHETSTONE TEST SUITE · SAVINGS CREDITED TO RTK
      </div>

      {/* stat rail — whetstone-specific shipping facts, not upstream compression numbers */}
      <div className="stat-rail-x">
        <StatCell
          ix="// HEADROOM"
          value={90}
          format={(v) => Math.round(v) + '%'}
          unit=" UP TO"
          color="acid"
          label="CONTEXT REDUCTION"
          sub="HEADROOM PROJECT FIGURE · NOT A WHETSTONE NUMBER"
        />
        <StatCell
          ix="// WHETSTONE"
          value={1}
          format={(v) => Math.round(v) + ''}
          unit=" BINARY"
          label="RUST 2021 · IDEMPOTENT SETUP"
          sub="HEADROOM + RTK + ICM, ORCHESTRATED"
        />
        <StatCell
          ix="// MIGRATE"
          value={1}
          format={(v) => Math.round(v) + ''}
          unit=" COMMAND"
          color="mag"
          label="v2 → v3 WITH --rollback"
          sub="ARCHIVE-BACKED, REVERSIBLE"
        />
      </div>
    </section>
  );
}

Object.assign(window, { Numbers });
