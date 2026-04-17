---
description: Review pending memory digests produced by hellodb-brain and approve or reject them before they land on main. Use when the user says "review memory", "check digests", "what has brain queued", or invokes /hellodb:review.
---

You are helping the user review pending memory digests produced by the hellodb brain (passive memory pipeline) before they land on the facts main branch. Digest branches are named `{facts_namespace}/digest-{timestamp}` and hold proposed facts waiting for human approval.

## Your task

1. **Find pending digest branches.** Call `mcp__hellodb__hellodb_list_branches` with the facts namespace (default: `claude.facts`; if the user configured a different namespace in `~/.hellodb/brain.toml`, ask them or try `claude.facts` first). Filter to branches where:
   - `state == "active"` (not already merged or abandoned)
   - `label` starts with `digest-`

2. **Fetch each digest's proposed facts.** For each pending branch, call `mcp__hellodb__hellodb_query` with `namespace: <facts_ns>`, `branch: <digest_branch_id>`, and `schema: "brain.fact"`. To show ONLY what the digest proposes (not inherited facts already on main), also query `branch: <facts_ns>/main` and subtract matching record_ids.

3. **Present concisely.** For each pending branch, show:
   - branch id (trailing timestamp is enough to distinguish)
   - fact count
   - topics covered (dedup `data.topic`)
   - a one-line preview per fact — truncate `statement` to 80 chars

4. **Ask the user which to merge.** Three actions per branch:
   - **merge** — call `mcp__hellodb__hellodb_merge_branch` with the branch id
   - **keep pending** — do nothing (leave branch active)
   - **forget per-fact** — if some facts are good and others aren't, tombstone the bad ones via `mcp__hellodb__hellodb_forget` before merging

5. **After merges, reinforce accepted facts.** For every fact the user approved, call `mcp__hellodb__hellodb_reinforce` with its `record_id` and `delta: 1.0`. Being approved IS a reinforcement signal that boosts the fact's decay-aware ranking at recall time.

6. **Summarize.** Brief final report: digests reviewed, merged, facts landed on main, facts tombstoned.

## Ground rules

- Never merge without explicit user approval for each branch.
- If there are no pending digests, say so in one line and stop — don't invent work.
- If the facts namespace doesn't exist, the brain has never run; say so and suggest running `hellodb brain --status`.
- Keep output terse — the user wants a decision interface, not a narrative.
