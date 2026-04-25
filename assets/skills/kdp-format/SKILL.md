---
name: kdp-format
description: "Use when the user says 'kdp', 'format for kdp', 'format book', or 'manuscript' and needs print-ready output."
---

# 📚 KDP Format — Converting manuscript to KDP-ready .docx

*Convert a markdown manuscript into a professionally formatted Word document for Amazon KDP.*

> **⚠️ WARNING — Unit Systems in python-docx**
>
> - Property setters (e.g., `section.left_margin = Inches(0.75)`) handle unit conversion automatically
> - Raw XML attributes use **TWIPS** (1 inch = 1440 twips), NOT EMU (1 inch = 914400 EMU)
> - **NEVER** use `str(Inches(x))` in raw XML — it produces EMU values, which are 635× too large
> - If you must use raw XML: `twips = int(inches * 1440)`

## Activation

When this skill activates, output:

`📚 KDP Format — Converting manuscript to KDP-ready .docx...`

## Context Guard

| Context | Status |
|---------|--------|
| **User asks to format a manuscript for KDP** | ACTIVE — full conversion |
| **User says "format for kdp" with a file path** | ACTIVE — full conversion |
| **User mentions book formatting or manuscript** | ACTIVE — ask for file path |
| **User is writing content, not formatting** | DORMANT — do not activate |
| **Discussing KDP concepts generally** | DORMANT — do not activate |

## Output Specifications

| Property | Value |
|----------|-------|
| **Trim size** | 6" × 9" (standard trade paperback) |
| **Body font** | Georgia 11pt |
| **Heading font** | Arial (24pt chapters, 14pt sub, 13pt sections) |
| **Code font** | Courier New 10pt, gray background |
| **Margins** | Top 0.75", Bottom 0.75", Inside 0.75" (gutter), Outside 0.5" |
| **Line spacing** | 1.3× body, 1.0× code/quotes |
| **Paragraph indent** | 0.3" first-line (except first after heading) |

## Protocol

### Step 1: Get the manuscript path
If the user hasn't provided a markdown file path, ask:
> What is the path to your markdown manuscript file?

### Step 2: Verify dependencies
Ensure `python-docx` is available:
```bash
pip install python-docx
```

### Step 3: Build the document
Use python-docx to convert the markdown manuscript following the output specifications above. Handle:
- YAML frontmatter for title, author, ISBN metadata
- Part headers (H1), chapter headers (H2), section headers (H3)
- Body text with proper indentation
- Blockquotes, code blocks, inline formatting
- Scene breaks (horizontal rules → centered `* * *`)
- Tables, hyperlinks

### Step 4: Report results
```
📚 KDP Format — Complete!

Generated: <output-path>
Trim size: 6" × 9"
Sections: Title page, Copyright, Table of Contents, Body
Chapters: <count>
Parts: <count>

Next steps:
1. Open in Word to verify formatting
2. Check page count for KDP margin requirements
3. Upload to KDP dashboard
```

## Ownership
- "kdp" / "format for kdp" / "format book" / "manuscript" = KDP Format only
