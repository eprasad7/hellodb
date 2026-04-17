import { Section } from "./section";

const rows: { label: string; saas: string; ours: string }[] = [
  { label: "where data lives", saas: "their servers", ours: "your R2 bucket" },
  { label: "who holds the keys", saas: "they do", ours: "you do" },
  { label: "cost", saas: "$20–200 / mo / seat", ours: "~$0 (CF free tier)" },
  { label: "audit trail", saas: "their dashboard", ours: "git-like branch log" },
  { label: "lock-in", saas: "high", ours: "none — it's your bucket" },
  { label: "embedding model", saas: "what they ship", ours: "Workers AI · OpenAI · local" },
  { label: "offline", saas: "no", ours: "yes (fastembed-rs)" },
  { label: "platforms", saas: "browser only", ours: "macOS · Linux · Windows" },
  { label: "vendor outage", saas: "you stop", ours: "you keep working locally" },
];

export function Comparison() {
  return (
    <Section
      eyebrow="comparison"
      title={
        <>
          Memory you rent vs.{" "}
          <span className="italic text-accent">memory you own.</span>
        </>
      }
      lede="Cloud memory SaaS solves the storage problem and creates a sovereignty problem. hellodb solves both."
    >
      {/* Mobile: stacked cards */}
      <div className="flex flex-col gap-3 md:hidden">
        {rows.map((r) => (
          <div
            key={r.label}
            className="rounded-xl border border-border bg-bg-elevated/30 p-4"
          >
            <div className="mb-3 font-mono text-[11px] uppercase tracking-[0.16em] text-fg-muted">
              {r.label}
            </div>
            <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 text-[13px]">
              <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-fg-subtle">
                saas
              </div>
              <div className="text-fg-muted">{r.saas}</div>
              <div className="font-mono text-[10px] uppercase tracking-[0.14em] text-accent-muted">
                hellodb
              </div>
              <div className="text-fg">{r.ours}</div>
            </div>
          </div>
        ))}
      </div>

      {/* md+: proper table */}
      <div className="hidden overflow-hidden rounded-[var(--radius-card)] border border-border md:block">
        <table className="w-full border-collapse text-left text-[14px]">
          <thead>
            <tr className="border-b border-border bg-bg-elevated/40">
              <th
                scope="col"
                className="px-5 py-4 font-mono text-[11px] uppercase tracking-[0.16em] text-fg-muted"
              >
                <span className="sr-only">property</span>
              </th>
              <th
                scope="col"
                className="px-5 py-4 font-mono text-[11px] uppercase tracking-[0.16em] text-fg-muted"
              >
                cloud memory SaaS
              </th>
              <th
                scope="col"
                className="bg-accent/5 px-5 py-4 font-mono text-[11px] uppercase tracking-[0.16em] text-accent"
              >
                hellodb on your CF
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr
                key={r.label}
                className={i < rows.length - 1 ? "border-b border-border/60" : ""}
              >
                <th
                  scope="row"
                  className="px-5 py-3.5 text-left font-mono text-[12px] font-normal text-fg-muted"
                >
                  {r.label}
                </th>
                <td className="px-5 py-3.5 text-fg-muted">{r.saas}</td>
                <td className="bg-accent/[0.04] px-5 py-3.5 text-fg">
                  {r.ours}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <p className="mt-6 max-w-2xl font-mono text-[12px] leading-relaxed text-fg-muted">
        Sovereignty isn&apos;t a marketing word here. It&apos;s a deployment
        topology: every byte of your memory lives in storage you control,
        encrypted with a key in your OS keychain.
      </p>
    </Section>
  );
}
