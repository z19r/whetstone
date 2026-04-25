---
name: forge
description: "New skill creation — create new MemStack skills following the official Anthropic SKILL.md format. Use when the user says 'forge this', 'new skill', or 'create enchantment'."
---

# 🔨 Forge — New skill creation

## Activation

When this skill activates, output:

`🔨 Forge — Creating new skill...`

## Context Guard

| Context | Status |
|---------|--------|
| **User wants to create a new skill** | ACTIVE |
| **User says "forge this", "new skill", "create enchantment"** | ACTIVE |
| **User is modifying an existing skill** | DORMANT — just edit directly |

## Protocol

1. Ask for: skill name, description, trigger phrases, and intended behavior
2. Create directory: `skills/{name}/SKILL.md`
3. Use the official Anthropic SKILL.md format with YAML frontmatter
4. Include: Activation message, Context Guard table, Protocol steps, Ownership rules
5. If the skill needs a companion rule, create `rules/{name}.md`
6. Update the Skill Index in MEMSTACK.md

## Template

```markdown
---
name: {skill-name}
description: "{one-line description with trigger phrases}"
---

# {emoji} {Name} — {short description}

## Activation
When this skill activates, output:
`{emoji} {Name} — {action}...`

## Context Guard
| Context | Status |
|---------|--------|
| **{active condition}** | ACTIVE |
| **{dormant condition}** | DORMANT |

## Protocol
1. ...

## Ownership
- "{trigger}" = {Name} only
```

## Ownership
- "forge this" / "new skill" / "create enchantment" = Forge only
