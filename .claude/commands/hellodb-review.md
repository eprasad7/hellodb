---
description: Review pending memory digests from hellodb-brain and approve or reject them
---

You are helping the user review pending memory digests produced by `hellodb-brain` (the passive memory pipeline) before they land on the facts `main` branch. Digest branches are named `{facts_namespace}/digest-{timestamp}` and hold proposed facts waiting for human approval.

## Your task

1. **Find pending digest branches.** Call `mcp__hellodb__hellodb_list_branches` with the facts namespace (default: `claude.facts` — if the user has configured a different one in `~/.hellodb/brain.toml`, ask them or try `claude.facts` first). Filter to branches where:
   - `state == "active"` (not already merged or abandoned)
   - `label` starts with `digest-`

2. **Fetch each digest's facts.** For each pending branch:
   - Call `mcp__hellodb__hellodb_query` with `namespace: <facts_ns>`, `branch: <digest_branch_id>`, and `schema: "brain.fact"` (the default schema the brain writes).
   - Note: `query` on the draft branch returns inherited records too. To show ONLY what this digest proposes, diff against the parent: call it again with `branch: <facts_ns>/main` and subtract records that appear in both.

3. **Present concisely.** For each digest branch, show a short table:
   - branch id (last 16 chars of timestamp is enough)
   - fact count
   - topics covered (deduped list of `data.topic` values)
   - a one-line preview of each fact (truncate `statement` to 80 chars)

4. **Ask the user which to merge.** Offer three actions per branch:
   - **merge** — call `mcp__hellodb__hellodb_merge_branch` with the branch id
   - **keep pending** — do nothing (user will decide later)
   - **forget per-fact** — if some facts in a branch are good and others aren't, tombstone the bad ones via `mcp__hellodb__hellodb_forget` before merging, so only the good ones land on main

5. **After merges, reinforce accepted facts.** For every fact you merged, call `mcp__hellodb__hellodb_reinforce` with its `record_id` (delta=1.0). Being approved by the user IS a reinforcement signal and should boost the fact's decay-aware ranking at recall time.

6. **Summarize the session.** End with a short report: how many digests were reviewed, how many merged, how many facts landed on main, how many tombstoned.

## Ground rules

- Never merge without explicit user approval.
- If there are no pending digests, say so in one line and stop — don't invent work.
- If the facts namespace doesn't exist yet, the brain has never run; say so and suggest the user run `hellodb-brain --status` to confirm.
- Keep your output terse. The user wants a decision interface, not a narrative.
