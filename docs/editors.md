# Editor & Tool Configuration

## Claude Code (CLI)

**Full stack support.** This is the primary target — all three tools work natively.

```bash
# Option A: One command (recommended)
whetstone
# same idea: headroom wrap claude

# Option B: Manual
headroom proxy --port 8787 &
claude

# Option C: Without Headroom (RTK + Memory only)
claude
```

**What happens automatically:**
- RTK hook rewrites every Bash tool call (`git status` -> `rtk git status`)
- Headroom compresses context before API calls (if proxy is running)
- Memory hooks fire on session start/end, commits, pushes
- Memory skills activate on keyword triggers ("recall", "todo", "verify", etc.)

**Hook configuration** lives in `~/.claude/settings.json` (global). All hooks — RTK and Memory — are installed to `~/.claude/hooks/` with absolute paths. Works in every project, every directory.

**MCP tools** (optional, adds `headroom_compress`, `headroom_retrieve`, `headroom_stats`):
```bash
headroom mcp install
headroom mcp status
```

---

## Claude Code (VS Code Extension)

**Full stack support.** The VS Code extension uses the same CLI and configuration.

**Setup:**
1. Run `whetstone setup` from your project root (terminal)
2. The extension reads the same `~/.claude/settings.json` hooks
3. RTK and Memory hooks work identically to CLI

**Headroom proxy connection** — two options:

Option A — Set in VS Code settings (`settings.json`):
```json
{
  "claude-code.environmentVariables": [
    {
      "name": "ANTHROPIC_BASE_URL",
      "value": "http://localhost:8787"
    }
  ]
}
```

Option B — The setup command already added `ANTHROPIC_BASE_URL` to your shell profile, which VS Code's integrated terminal inherits.

**Start the proxy** before opening VS Code:
```bash
headroom proxy --port 8787 &
```
Or use the [systemd service](headroom-service.md) to have it always running.

---

## Claude Code (JetBrains Extension)

**Full stack support.** Same as VS Code — the JetBrains extension uses the same CLI backend.

1. Run `whetstone setup` from your project terminal
2. Hooks are shared via `~/.claude/settings.json`
3. Start Headroom proxy before your session, or use the systemd service

**Environment variable:** Set `ANTHROPIC_BASE_URL=http://localhost:8787` in your JetBrains run configuration or shell profile.

---

## Cursor

**Partial support.** RTK and Headroom work. Memory does not (Cursor has a different hook system).

**RTK setup:**
```bash
rtk init -g --agent cursor
```
This creates `~/.cursor/hooks/rtk-rewrite.sh` and patches Cursor's `hooks.json`.

**Headroom setup:**
1. Start the proxy: `headroom proxy --port 8787 &`
2. In Cursor: Settings > Models > Override OpenAI Base URL
3. Set to: `http://localhost:8787/v1`
4. Enter your API key

Alternatively, set the environment variable before launching:
```bash
ANTHROPIC_BASE_URL=http://localhost:8787 cursor
```

**Memory:** Not supported. Cursor does not implement Claude Code's `PreToolUse`/`PostToolUse`/`SessionStart`/`Stop` lifecycle hooks.

---

## VS Code + GitHub Copilot

**Partial support.** RTK works. Headroom requires SDK integration. Memory does not work.

**RTK setup:**
```bash
rtk init -g --copilot
```
This creates `.github/hooks/rtk-rewrite.json` and `.github/copilot-instructions.md`. Copilot Chat in VS Code gets transparent command rewriting. Copilot CLI uses deny-with-suggestion (CLI limitation — it cannot silently rewrite).

**Headroom:** No native base URL override in Copilot. For programmatic usage, use the SDK:
```typescript
import { withHeadroom } from 'headroom-ai/openai';
const client = withHeadroom(new OpenAI());
```

**Memory:** Not supported (different hook architecture).

---

## Windsurf

**Partial support.** RTK works (project-scoped). Headroom via env var. Memory does not work.

**RTK setup:**
```bash
cd your-project
rtk init --agent windsurf
```
This creates `.windsurfrules` in your project root. Windsurf's Cascade reads this file and prefixes commands with `rtk`. Note: project-scoped only (no global `-g` flag).

**Headroom:** Set the environment variable before launching:
```bash
ANTHROPIC_BASE_URL=http://localhost:8787 windsurf
```

**Memory:** Not supported.

---

## Cline / Roo Code (VS Code)

**Partial support.** RTK works (project-scoped). Headroom via settings. Memory does not work.

**RTK setup:**
```bash
cd your-project
rtk init --agent cline
```
This creates `.clinerules` in your project root. Project-scoped only.

**Headroom:** Cline has API base URL fields in its settings panel. Set the Anthropic base URL to `http://localhost:8787`.

**Memory:** Not supported.

---

## Aider

**Partial support.** Headroom works natively. RTK is instruction-based. Memory does not work.

**Headroom (one command):**
```bash
headroom wrap aider
```
This starts the proxy and launches Aider with the correct base URL.

**RTK:** No hook system in Aider. You can add instructions to `.aider.conf.yml` or use `rtk` commands directly in your prompts.

**Memory:** Not supported.

---

## OpenAI Codex CLI

**Partial support.** RTK works (instruction-based). Headroom works. Memory does not work.

**Headroom:**
```bash
headroom wrap codex
```

**RTK setup:**
```bash
rtk init -g --codex
```
This creates `~/.codex/RTK.md` and `~/.codex/AGENTS.md`. Codex reads these as global instructions and prefixes commands with `rtk`. This is instruction-based (no hook API), so compliance depends on the model.

**Memory:** Not supported.

---

## Gemini CLI

**Partial support.** RTK works (hook-based). Headroom via env var. Memory does not work.

**RTK setup:**
```bash
rtk init -g --gemini
```
This creates `~/.gemini/hooks/rtk-hook-gemini.sh` and patches `~/.gemini/settings.json` with a `BeforeTool` hook.

**Headroom:**
```bash
OPENAI_BASE_URL=http://localhost:8787/v1 gemini
```

**Memory:** Not supported.

---

## OpenCode

**Partial support.** RTK works (plugin-based). Headroom via env var. Memory does not work.

**RTK setup:**
```bash
rtk init -g --opencode
```
This creates `~/.config/opencode/plugins/rtk.ts` using the `tool.execute.before` plugin hook.

---

## Compatibility Matrix

| Feature | Claude Code | Cursor | Copilot | Windsurf | Cline | Aider | Codex | Gemini CLI |
|---------|:-----------:|:------:|:-------:|:--------:|:-----:|:-----:|:-----:|:----------:|
| **Headroom proxy** | `wrap` | Manual URL | SDK only | env var | settings | `wrap` | `wrap` | env var |
| **Headroom MCP** | `mcp install` | manual | -- | -- | -- | -- | -- | -- |
| **RTK hooks** | PreToolUse | preToolUse | PreToolUse | `.windsurfrules` | `.clinerules` | manual | instructions | BeforeTool |
| **RTK scope** | global | global | global | project | project | -- | global | global |
| **Memory skills** | full | -- | -- | -- | -- | -- | -- | -- |
| **Memory hooks** | full | -- | -- | -- | -- | -- | -- | -- |
| **Memory memory** | full | -- | -- | -- | -- | -- | -- | -- |

Legend: full = fully supported, manual = requires manual configuration, -- = not supported
