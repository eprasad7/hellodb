import { ReactNode } from "react";

export function Section({
  id,
  eyebrow,
  title,
  lede,
  children,
  className = "",
}: {
  id?: string;
  eyebrow?: string;
  title?: ReactNode;
  lede?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      id={id}
      className={`relative w-full px-5 py-16 sm:px-6 sm:py-24 md:px-10 md:py-28 ${className}`}
    >
      <div className="mx-auto max-w-6xl">
        {(eyebrow || title || lede) && (
          <div className="mb-14">
            {eyebrow && (
              <div className="mb-3 font-mono text-xs uppercase tracking-[0.18em] text-accent-muted">
                {eyebrow}
              </div>
            )}
            {title && (
              <h2 className="max-w-2xl font-display text-4xl leading-[1.05] text-fg text-balance sm:text-5xl">
                {title}
              </h2>
            )}
            {lede && (
              <p className="mt-5 max-w-4xl text-lg leading-relaxed text-fg-muted text-pretty">
                {lede}
              </p>
            )}
          </div>
        )}
        {children}
      </div>
    </section>
  );
}
