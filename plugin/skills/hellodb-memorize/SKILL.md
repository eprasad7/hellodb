---
description: Actively capture durable facts the user states (preferences, workflow rules, codebase conventions, decisions) into hellodb memory. Use proactively — not only when explicitly asked. Trigger when the user states something like "I prefer X", "always do Y", "this project uses Z", "remember that...", or settles a recurring decision. Do NOT trigger on ephemeral debugging state, transient commands, or things already obvious from the codebase.
---

You just observed the user state something durable — a preference, a rule, a convention, or a decision. Capture it in hellodb so the next session (yours or a future one) inherits the knowledge instead of asking again.

## What counts as durable (capture these)

- User preferences: "I use pnpm not npm", "tabs over spaces", "prefer OKLCH colors"
- Workflow rules: "run tests before commit", "never force-push to main"
- Codebase conventions: "components live in `src/components/`", "all API routes go through `/api/v1/`"
- Decisions that settle debate: "we're using Biome, not ESLint+Prettier", "auth is OAuth via Google only"
- Environmental facts: "dev DB runs on port 5433", "the staging URL is ..."

## What does NOT count (skip)

- Transient state: "the build is failing right now", "I'm debugging X"
- Things obvious from the codebase: "this is a Next.js project"
- One-time task commands: "run this test for me"
- Your own observations or inferences — only capture what the user explicitly stated
- Questions, explorations, or speculation

## How to capture

Use `mcp__hellodb__hellodb_note` (the zero-setup write path — auto-creates namespace and schema on first use). Default namespace is `claude.episodes` so brain can digest these into consolidated facts on the next pass.

```
mcp__hellodb__hellodb_note({
  namespace: "claude.episodes",
  data: {
    topic: "<short topic tag: preferences | workflow | codebase | environment | decision>",
    text: "<verbatim statement in the user's words, 1 sentence>",
    source: "<brief context — what task the user was doing when they said this>"
  }
})
```

## Operating discipline

- **Capture silently.** Do not announce that you saved something unless the user asks. A one-word confirmation at most.
- **Don't over-capture.** Better to miss a borderline fact than pollute memory with low-signal notes. The digest pipeline aggregates and dedupes, but it only works on genuinely distinct signals.
- **Don't editorialize.** Store the user's exact words in `text`. Interpretation happens later at digest time.
- **One note per distinct fact.** If the user states three preferences in one turn, that's three notes, not one merged blob.
- **Trust the pipeline.** You don't need to decide what's "important" — just whether it's durable. The brain's digest+reinforce+decay cycle handles salience.

## Anti-patterns

- DON'T call `mcp__hellodb__hellodb_remember` here — that requires a registered schema. `hellodb_note` is the looser write path designed exactly for this use case.
- DON'T write to `claude.facts` — that's the brain's output namespace, for consolidated facts.
- DON'T ask permission before capturing. The point is for this to be frictionless. The user can review/reject at digest time via `/hellodb:hellodb-review`.
