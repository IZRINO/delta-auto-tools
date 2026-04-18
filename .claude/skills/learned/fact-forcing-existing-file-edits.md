---
name: fact-forcing-existing-file-edits
description: "Handle Fact-Forcing write hooks by proving facts, then prefer Edit for existing files."
user-invocable: false
origin: auto-extracted
---

# Fact-Forcing Gate for Existing File Updates

**Extracted:** 2026-04-19
**Context:** Applies when a strict PreToolUse/Write hook blocks file updates and asks for factual justification before allowing the operation.

## Problem

Some environments use a strict write gate that treats a `Write` call as file creation or high-risk overwrite.  
When updating an existing file, the hook may block with requirements like:

1. show which files reference the target
2. confirm no duplicate-purpose file exists
3. show data structure if the file reads/writes data
4. quote the user's instruction verbatim

If you blindly retry `Write`, you waste a turn and may keep hitting the same gate.

## Solution

Use this sequence:

### 1. Determine whether the target file already exists
- If it exists, prefer `Edit` over `Write`
- Reserve `Write` for true new files or full rewrites you actually need

### 2. Collect the required facts
Typical tool pattern:

```text
Glob  -> check whether files of the same purpose already exist
Grep  -> find references to the target file in repo docs/config
Read  -> verify current file exists and inspect current content
```

### 3. Present the facts explicitly
Structure the response around the hook's requested items:

```text
1. Referencing file(s) and line(s): <file:line>
2. Existing-file check: <result from Glob>
3. Data-file behavior: not applicable / or show synthetic structure
4. User instruction: "<verbatim user request>"
```

### 4. Retry with the right tool
- **Existing file** -> use `Edit`
- **New file** -> retry `Write` with the same content after presenting facts

## Example

### Bad
- Hook blocks `Write` on `README.md`
- Retry `Write` again
- Get blocked again

### Good
- `Read README.md`
- `Glob **/README.md`
- `Grep` for references like `README.md|AGENTS.md`
- Present the required facts
- Switch to `Edit` because `README.md` already exists

## When to Use

Use this pattern when:
- a PreToolUse/Write hook blocks a doc/config update
- the hook asks for factual justification before retry
- the file already exists and only needs modification
- repeated `Write` attempts would be redundant

Do not use this as a blanket rule for all writes:
- if the file is genuinely new, `Write` is still appropriate
- if the hook requests facts about data shape, provide them before retrying
