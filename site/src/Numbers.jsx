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
        <h2>Measured against<br />a clean baseline.</h2>
      </div>

      {/* big diff */}
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
          <div className="label">OUT · COMPRESSED</div>
          <div className="value">11</div>
          <div className="unit">TOKENS</div>
        </div>
      </div>

      {/* stat rail */}
      <div className="stat-rail-x">
        <StatCell
          ix="// HEADROOM"
          value={90}
          format={(v) => Math.round(v) + '%'}
          unit=""
          color="acid"
          label="CONTEXT COMPRESSED"
          sub="UP TO · MULTI-STAGE PIPELINE"
        />
        <StatCell
          ix="// SQUAD V2"
          value={97}
          format={(v) => Math.round(v) + ''}
          unit="% ACC"
          label="ACCURACY @ 19% TOK"
          sub="EXTRACTIVE QA BENCHMARK"
        />
        <StatCell
          ix="// RTK"
          value={99.8}
          format={(v) => v.toFixed(1)}
          unit="% SAVED"
          color="mag"
          label="CARGO TEST · 4,800 → 11"
          sub="REAL RUN · WHETSTONE TESTS"
        />
      </div>
    </section>
  );
}

Object.assign(window, { Numbers });
