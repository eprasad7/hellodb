/**
 * Context vs Memory — thesis diagram for the launch blog post.
 *
 * Two panels, side-by-side on desktop, stacked on mobile:
 *   LEFT  (triage):       bounded window → /compact lossy event → shrunken summary
 *   RIGHT (engineering):  unbounded signed stack → top-k recall feeds session
 *
 * The visual argument is about *boundedness*: the left panel has a hard outer
 * box that facts drop out of; the right panel has no outer box, facts stay,
 * and only a selective top-k gets pulled in. Same input, different retention.
 */
export function ContextVsMemory() {
  return (
    <figure className="not-prose my-10 rounded-[var(--radius-card)] border border-border bg-bg-sunken/60 p-5 md:p-7">
      <div className="grid grid-cols-1 gap-x-8 gap-y-10 md:grid-cols-[1fr_auto_1fr]">
        <ContextPanel />
        <Divider />
        <MemoryPanel />
      </div>
      <figcaption className="mt-6 border-t border-border pt-4 text-center font-mono text-[11px] uppercase tracking-[0.14em] text-fg-subtle">
        same input · different retention
      </figcaption>
    </figure>
  );
}

/* ───────────────────────────── left panel ─────────────────────────── */

function ContextPanel() {
  const facts = [
    "pnpm over npm",
    "oauth, not sessions",
    "tabs over spaces",
    "use OKLCH",
    "dark by default",
    "Rust workspaces",
    "brain.toml @ 0.75",
    "no force-push",
  ];
  // Which facts survive /compact. The others are lossy-dropped.
  const survivorIdx = new Set([0, 3, 6]);

  return (
    <div className="flex flex-col">
      <PanelHead
        eyebrow="context"
        accent="triage"
        muted
      />

      {/* Bounded window: this is the whole point of the left panel. */}
      <div className="relative mt-4 rounded-[8px] border border-border-strong bg-bg/40 p-3">
        <div className="mb-2 flex items-center justify-between font-mono text-[10px] uppercase tracking-[0.14em] text-fg-subtle">
          <span>1M window</span>
          <span className="text-fg-muted">986K / 1M</span>
        </div>
        <ul className="grid grid-cols-2 gap-1.5">
          {facts.map((f, i) => (
            <li
              key={f}
              className="truncate rounded-[4px] border border-border bg-bg-elevated px-2 py-1 font-mono text-[10.5px] text-fg-muted"
              title={f}
            >
              {f}
            </li>
          ))}
        </ul>
      </div>

      {/* Compact event — shown as a lossy gate */}
      <div className="relative mt-3 flex items-center gap-3 py-1">
        <div className="h-px flex-1 bg-gradient-to-r from-transparent via-fg-subtle/40 to-fg-subtle/40" />
        <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-fg-subtle">
          /compact
        </span>
        <ArrowDown />
        <div className="h-px flex-1 bg-gradient-to-l from-transparent via-fg-subtle/40 to-fg-subtle/40" />
      </div>

      {/* After-compact window — smaller, with dropped facts visibly lost */}
      <div className="relative mt-1 rounded-[8px] border border-border bg-bg/20 p-3">
        <div className="mb-2 font-mono text-[10px] uppercase tracking-[0.14em] text-fg-subtle">
          summary
        </div>
        <ul className="grid grid-cols-2 gap-1.5">
          {facts.map((f, i) => {
            const kept = survivorIdx.has(i);
            return (
              <li
                key={f}
                className={
                  kept
                    ? "truncate rounded-[4px] border border-border bg-bg-elevated px-2 py-1 font-mono text-[10.5px] text-fg-muted"
                    : "truncate rounded-[4px] border border-dashed border-border/50 px-2 py-1 font-mono text-[10.5px] text-fg-subtle/50 line-through decoration-fg-subtle/60"
                }
                title={f}
              >
                {f}
              </li>
            );
          })}
        </ul>
      </div>

      <p className="mt-4 font-mono text-[11px] leading-relaxed text-fg-subtle">
        5 facts dropped. the model can&rsquo;t tell you which.
        <br />
        next prompt referencing them misses.
      </p>
    </div>
  );
}

/* ───────────────────────────── right panel ────────────────────────── */

function MemoryPanel() {
  const facts = [
    { h: "b3:a7f2", t: "pnpm over npm" },
    { h: "b3:e4c8", t: "oauth, not sessions" },
    { h: "b3:9d11", t: "tabs over spaces" },
    { h: "b3:f03a", t: "use OKLCH" },
    { h: "b3:2b6e", t: "dark by default" },
    { h: "b3:5c44", t: "Rust workspaces" },
    { h: "b3:1ab9", t: "brain.toml @ 0.75" },
    { h: "b3:c3d7", t: "no force-push" },
  ];
  // Which facts the top-k pulls back for the next session (semantic + decay).
  const pulledIdx = new Set([0, 3, 6]);

  return (
    <div className="flex flex-col">
      <PanelHead eyebrow="memory" accent="engineering" />

      {/* Unbounded stack: intentionally NO outer bounded box — visually
          contrasts with the left panel's hard window. */}
      <ul className="mt-4 flex flex-col divide-y divide-border/60">
        {facts.map((f, i) => {
          const pulled = pulledIdx.has(i);
          return (
            <li
              key={f.h}
              className={
                "flex items-center gap-3 py-1.5 font-mono text-[11px] " +
                (pulled ? "text-fg" : "text-fg-muted")
              }
            >
              <span
                aria-hidden="true"
                className={
                  "w-[3px] self-stretch rounded-full " +
                  (pulled ? "bg-accent" : "bg-border-strong")
                }
              />
              <span className="shrink-0 text-fg-subtle tracking-tight">
                {f.h}
              </span>
              <span className="truncate">{f.t}</span>
            </li>
          );
        })}
        <li className="py-1 pl-[11px] font-mono text-[11px] text-fg-subtle">
          &hellip; immutable, signed, content-addressed
        </li>
      </ul>

      {/* Selective recall gate */}
      <div className="relative mt-3 flex items-center gap-3 py-1">
        <div className="h-px flex-1 bg-gradient-to-r from-transparent via-accent/40 to-accent/40" />
        <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-accent">
          top-k
        </span>
        <ArrowDown accent />
        <div className="h-px flex-1 bg-gradient-to-l from-transparent via-accent/40 to-accent/40" />
      </div>

      {/* What flows into the next session */}
      <div className="mt-1 rounded-[8px] border border-accent/40 bg-accent/5 p-3">
        <div className="mb-2 font-mono text-[10px] uppercase tracking-[0.14em] text-accent">
          next session
        </div>
        <ul className="flex flex-col gap-1.5">
          {facts
            .filter((_, i) => pulledIdx.has(i))
            .map((f) => (
              <li
                key={f.h}
                className="flex items-center gap-2 font-mono text-[10.5px] text-fg"
              >
                <span className="text-accent">›</span>
                <span className="text-fg-subtle">{f.h}</span>
                <span className="truncate">{f.t}</span>
              </li>
            ))}
        </ul>
      </div>

      <p className="mt-4 font-mono text-[11px] leading-relaxed text-fg-subtle">
        window stays lean.
        <br />
        store is the source of truth.
      </p>
    </div>
  );
}

/* ───────────────────────────── shared bits ─────────────────────────── */

function PanelHead({
  eyebrow,
  accent,
  muted,
}: {
  eyebrow: string;
  accent: string;
  muted?: boolean;
}) {
  return (
    <div className="flex items-baseline gap-2">
      <span
        className={
          "font-mono text-[11px] uppercase tracking-[0.18em] " +
          (muted ? "text-fg-subtle" : "text-accent")
        }
      >
        {eyebrow}
      </span>
      <span className="text-fg-subtle">·</span>
      <span className="font-display text-[18px] italic leading-none text-fg">
        {accent}
      </span>
    </div>
  );
}

function Divider() {
  return (
    <div
      aria-hidden="true"
      className="hidden self-stretch md:block"
    >
      <div className="h-full w-px bg-gradient-to-b from-transparent via-border-strong to-transparent" />
    </div>
  );
}

function ArrowDown({ accent }: { accent?: boolean }) {
  return (
    <svg
      width="10"
      height="10"
      viewBox="0 0 10 10"
      fill="none"
      aria-hidden="true"
      className="shrink-0"
    >
      <path
        d="M5 0 L5 9 M2 6 L5 9 L8 6"
        stroke={accent ? "var(--color-accent)" : "var(--color-fg-subtle)"}
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
