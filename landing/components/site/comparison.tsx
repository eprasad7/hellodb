import { Section } from "./section";

export function Comparison() {
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
      <div className="overflow-x-auto rounded-[var(--radius-card)] border border-border">
        <table className="w-full min-w-[640px] border-collapse text-left text-[14px]">
          <thead>
            <tr className="border-b border-border bg-bg-elevated/40">
              <th className="px-5 py-4 font-mono text-[11px] uppercase tracking-[0.16em] text-fg-subtle"></th>
              <th className="px-5 py-4 font-mono text-[11px] uppercase tracking-[0.16em] text-fg-subtle">
                cloud memory SaaS
              </th>
              <th className="bg-accent/5 px-5 py-4 font-mono text-[11px] uppercase tracking-[0.16em] text-accent">
                hellodb on your CF
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr
                key={r.label}
                className={
                  i < rows.length - 1
                    ? "border-b border-border/60"
                    : ""
                }
              >
                <td className="px-5 py-3.5 font-mono text-[12px] text-fg-subtle">
                  {r.label}
                </td>
                <td className="px-5 py-3.5 text-fg-muted">{r.saas}</td>
                <td className="bg-accent/[0.04] px-5 py-3.5 text-fg">
                  {r.ours}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <p className="mt-6 max-w-2xl font-mono text-[12px] leading-relaxed text-fg-subtle">
        Sovereignty isn't a marketing word here. It's a deployment topology:
        every byte of your memory lives in storage you control, encrypted with
        a key in your OS keychain.
      </p>
    </Section>
  );
}
