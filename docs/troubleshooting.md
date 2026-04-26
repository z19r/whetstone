# Troubleshooting

## "headroom: command not found"

```bash
uv tool install "headroom-ai[proxy,code,mcp]"
# If installed but not on PATH:
python3 -m headroom proxy --port 8787
```

## "rtk: command not found"

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
# Add to PATH
export PATH="$HOME/.local/bin:$PATH"
# Persist
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc  # or ~/.bashrc
```

## "rtk gain" shows wrong output (Rust Type Kit conflict)

```bash
which rtk                 # Check which binary you have
# If it's the wrong one:
cargo uninstall rtk       # Remove Rust Type Kit
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
```

## RTK hook not rewriting commands

```bash
# Check hook exists
ls -la ~/.claude/hooks/rtk-rewrite.sh

# Check settings.json has the hook
cat ~/.claude/settings.json | python3 -m json.tool

# Test rewrite manually
echo '{"tool_name":"Bash","tool_input":{"command":"git status"}}' | bash ~/.claude/hooks/rtk-rewrite.sh

# Re-initialize
rtk init -g --hook-only --auto-patch
```

## Headroom proxy not compressing

```bash
# Is proxy running?
curl -s localhost:8787/health

# Is env var set?
echo $ANTHROPIC_BASE_URL
# Should be: http://127.0.0.1:8787

# Start manually
headroom proxy --port 8787

# Check stats
curl -s localhost:8787/stats
```

## MemStack skills not loading

```bash
# Is it installed?
ls .claude/skills/MEMSTACK.md

# Are rules present?
ls .claude/rules/

# Is the database initialized?
whetstone db stats

# Re-initialize DB
whetstone db init
```

## Hooks not firing at all

```bash
# Check global settings
cat ~/.claude/settings.json | python3 -m json.tool

# Check hook scripts exist and are accessible
ls -la ~/.claude/hooks/

# Restore from backup if settings.json is broken
ls ~/.claude/settings.json.bak.*
cp ~/.claude/settings.json.bak.NEWEST ~/.claude/settings.json
```

## Uninstall

```bash
whetstone uninstall
```

Interactive prompts let you choose which components to remove (whetstone binary, RTK, Headroom, project MemStack files).

### Manual removal

**Remove MemStack (per-project):**
```bash
rm -rf .claude/skills .claude/rules .claude/commands .claude/memstack
rm -f config.local.json STACK-SETUP.md
```

**Remove RTK (global):**
```bash
rtk init -g --uninstall        # Remove hooks from settings.json
rm ~/.local/bin/rtk            # Remove binary
rm -rf ~/.local/share/rtk      # Remove tracking database
```

**Remove Headroom (global):**
```bash
uv tool uninstall headroom-ai
# Remove systemd service (if created)
systemctl --user disable --now headroom 2>/dev/null
rm -f ~/.config/systemd/user/headroom.service
```

**Restore original settings.json:**
```bash
ls -lt ~/.claude/settings.json.bak.* | head -5
cp ~/.claude/settings.json.bak.TIMESTAMP ~/.claude/settings.json
```

**Full cleanup:**
```bash
whetstone uninstall
rm -f ~/.local/bin/whetstone
rm -rf ~/.whetstone
```
