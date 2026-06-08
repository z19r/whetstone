/* global React */

// ============================================================
// COMPRESSION DEMO — tabbed: cargo test / git diff / context
// Shows raw input vs. RTK-compressed output
// ============================================================
const DEMOS = [
  {
    id: 'cargo',
    label: 'CARGO TEST',
    cmd: 'rtk cargo test',
    inTokens: 4800,
    outTokens: 11,
    savings: '-99.8%',
    raw:
`   Compiling whetstone-cli v3.0.0 (/Users/dev/whetstone)
    Finished test [unoptimized + debuginfo] target(s) in 4.32s
     Running unittests src/main.rs

running 11 tests
test config::tests::default_config_roundtrips ... ok
test hooks::tests::merges_into_existing_settings ... ok
test memory::tests::icm_provider_creates_db ... ok
test preflight::tests::detects_missing_uv ... ok
test rtk::tests::collision_with_rust_type_kit ... ok
test setup::tests::idempotent_on_rerun ... ok
test shell::tests::injects_anthropic_base_url ... ok
test version::tests::semver_bump_patch ... ok
test version::tests::semver_bump_minor ... ok
test version::tests::semver_bump_major ... ok
test wrapper::tests::exec_replaces_process ... ok

test result: ok. 11 passed; 0 failed; 0 ignored;
 finished in 0.04s`,
    compressed:
`✓ test · 11/11 passed · 0.04s`,
  },
  {
    id: 'gitdiff',
    label: 'GIT DIFF',
    cmd: 'rtk git diff',
    inTokens: 21500,
    outTokens: 1259,
    savings: '-94%',
    raw:
`diff --git a/src/setup.rs b/src/setup.rs
index 7f3a1b2..9c4e8a1 100644
--- a/src/setup.rs
+++ b/src/setup.rs
@@ -120,7 +120,7 @@ pub fn run(opts: SetupOpts) -> Result<()> {
-    let provider = prompt_memory_provider()?;
+    let provider = prompt_memory_provider().context("memory prompt")?;
@@ -148,6 +154,11 @@ pub fn run(opts: SetupOpts) -> Result<()> {
+    if opts.full {
+        update_headroom_extras(&extras)?;
+    }
diff --git a/src/rtk.rs b/src/rtk.rs
index a12b3c4..e56f7d8 100644
--- a/src/rtk.rs
+++ b/src/rtk.rs
@@ -42,6 +42,15 @@ pub fn detect_collision() -> Option<PathBuf> {
+    // also check homebrew prefix on macOS
+    if cfg!(target_os = "macos") {
+        if let Some(brew) = brew_prefix() {
+            let candidate = brew.join("bin/rtk");
+            if candidate.exists() && !is_whetstone_rtk(&candidate) {
+                return Some(candidate);
+            }
+        }
+    }
... 8 more files · 217 + lines · 64 − lines`,
    compressed:
`M  src/setup.rs        +1 −1  L120 memory prompt context
M  src/setup.rs        +5 −0  L154 update_headroom_extras when --full
M  src/rtk.rs          +9 −0  L42  detect collision via brew prefix on macos
M  src/hooks.rs        +3 −1  L88  fix Windows path separator in settings merge
M  src/version.rs      +2 −0  L17  derive Hash on Version
M  src/cli.rs          +12 −0 L201 add 'whetstone update --force' flag
M  src/preflight.rs    +4 −2  L31  warn instead of fail on uv version <0.4
M  src/db.rs           +18 −3 L155 add session.add_insight migration v3
M  CHANGELOG.md        +24 −0 L1   v3.0.0 release notes
+ 8 files · 217 added · 64 removed`,
  },
  {
    id: 'context',
    label: 'HEADROOM · CONTEXT',
    cmd: 'headroom proxy --port 8787',
    inTokens: 184312,
    outTokens: 34951,
    savings: '-81%',
    raw:
`POST /v1/messages
  → model: claude-opus-4-6
  → messages: 87
  → system_prompt: 12,408 tok
  → tools: 23 (incl. mcp__headroom__*)
  → user_messages: 41 (avg 1,140 tok)
  → assistant_replies: 32 (avg 2,890 tok)
  → tool_results: 14 (cargo, grep, read_file, ...)

raw_context_total: 184,312 tokens
≈ $0.83 / call · ≈ 7.2s ttft

[stage: route]
  · 8 messages routed to drop (low score)
  · 12 messages routed to compress (high redundancy)
  · 67 messages routed to pass-through

[stage: json-statistical]
  · 4 large tool_results compressed
  · 11,209 → 1,840 tok (-84%)

[stage: code-ast]
  · 6 code blocks rewrap, deadcode strip
  · 18,440 → 6,201 tok (-66%)

[stage: drop-by-score]
  · 8 messages dropped (relevance < 0.18)

[stage: cache-align]
  · prompt-prefix cache hit: 11,920 tok at 0.1x bill rate

post_context_total: 34,951 tokens
≈ $0.16 / call · ≈ 2.1s ttft`,
    compressed:
`route   8 drop · 12 compress · 67 pass
json   −84% (11,209 → 1,840)
ast    −66% (18,440 → 6,201)
score  8 dropped (relevance < 0.18)
cache  11,920 tok at 0.1× bill rate
out    34,951 tok · $0.16 · 2.1s ttft`,
  },
];

function highlightLine(text) {
  // light hand-roll: + lines acid, - lines magenta, ✓ acid, ✗ magenta
  if (/^\s*\+/.test(text) || /^M\s/.test(text)) return 'ok';
  if (/^\s*\-/.test(text) || /^✗/.test(text))    return 'err';
  if (/^\s*✓/.test(text))                        return 'ok';
  if (/^\s*\[stage/.test(text))                  return 'tag';
  if (/^\s*\.\.\./.test(text))                   return 'dim';
  if (/^\s*\/\//.test(text))                     return 'dim';
  return '';
}

function Pane({ kind, label, count, body }) {
  return (
    <div className={'pane' + (kind === 'out' ? ' out' : '')}>
      <div className="ptag">
        {label}
        <span className="count">{count.toLocaleString()} TOK</span>
      </div>
      <pre>
        {body.split('\n').map((line, i) => {
          const cls = highlightLine(line);
          return (
            <div key={i} className={cls}>{line || '\u00A0'}</div>
          );
        })}
      </pre>
    </div>
  );
}

function CompressionDemo() {
  const [idx, setIdx] = React.useState(0);
  const demo = DEMOS[idx];
  return (
    <section className="section ws-wrap" id="compression">
      <div className="ws-sec-head">
        <div className="ws-sec-tag">03 · COMPRESSION</div>
        <h2>Tool output<br />becomes a line.</h2>
      </div>

      <div className="demo">
        <div className="demo-tabs">
          {DEMOS.map((d, i) => (
            <button
              key={d.id}
              className={'demo-tab' + (i === idx ? ' is-active' : '')}
              onClick={() => setIdx(i)}
            >
              <span className="ix">0{i + 1}</span>
              <span>
                <div>{d.label}</div>
                <div style={{ fontSize: '10px', opacity: 0.7, marginTop: 2 }}>$ {d.cmd}</div>
              </span>
            </button>
          ))}
        </div>

        <div className="demo-body">
          <Pane kind="in"  label="// RAW STREAM"        count={demo.inTokens}  body={demo.raw} />
          <Pane kind="out" label="// AFTER WHETSTONE"   count={demo.outTokens} body={demo.compressed} />
        </div>

        <div className="demo-foot">
          <div className="stat">
            <div className="v">{demo.inTokens.toLocaleString()}</div>
            <div className="l">IN · TOKENS</div>
          </div>
          <div className="stat">
            <div className="v">{demo.outTokens.toLocaleString()}</div>
            <div className="l">OUT · TOKENS</div>
          </div>
          <div className="stat">
            <div className="v">{demo.savings}</div>
            <div className="l">SAVED</div>
          </div>
        </div>
      </div>
    </section>
  );
}

Object.assign(window, { CompressionDemo });
