Summarize the whetstone install for this project.

Run the doctor:

```bash
whetstone doctor
```

Then report:

- hook ordering in `~/.claude/settings.json` (is RTK PreToolUse Bash hook last?)
- ICM hooks present and well-formed
- any drift the doctor flagged

If `whetstone doctor` is not available, respond:

```
whetstone is not installed. Install with:
    curl -fsSL https://raw.githubusercontent.com/z19r/whetstone/main/install.sh | sh

Then run:
    whetstone setup
```
