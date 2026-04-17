export function SectionDivider() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none mx-auto h-px w-full max-w-6xl"
      style={{
        background:
          "linear-gradient(90deg, transparent, var(--color-border-strong) 20%, var(--color-accent-glow) 50%, var(--color-border-strong) 80%, transparent)",
        opacity: 0.55,
      }}
    />
  );
}
