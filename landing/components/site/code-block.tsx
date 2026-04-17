import { ReactNode } from "react";

export function Terminal({
  children,
  label,
  className = "",
}: {
  children: ReactNode;
  label?: string;
  className?: string;
}) {
  return (
    <div
      className={`relative overflow-hidden rounded-[var(--radius-card)] border border-border bg-bg-sunken/80 ${className}`}
    >
      <div className="flex items-center gap-2 border-b border-border bg-bg-elevated/60 px-4 py-2.5">
        <div className="flex gap-1.5">
          <span className="h-2.5 w-2.5 rounded-full bg-fg-subtle/40" />
          <span className="h-2.5 w-2.5 rounded-full bg-fg-subtle/40" />
          <span className="h-2.5 w-2.5 rounded-full bg-fg-subtle/40" />
        </div>
        {label && (
          <div className="ml-2 font-mono text-[11px] tracking-tight text-fg-subtle">
            {label}
          </div>
        )}
      </div>
      <div className="px-5 py-4 font-mono text-[13px] leading-relaxed text-fg [overflow-wrap:anywhere]">
        {children}
      </div>
    </div>
  );
}

export function Prompt({
  children,
  comment,
}: {
  children: ReactNode;
  comment?: string;
}) {
  return (
    <div className="flex flex-col">
      <div className="flex items-baseline gap-3">
        <span className="select-none text-accent">$</span>
        <span className="text-fg">{children}</span>
      </div>
      {comment && (
        <div className="ml-5 mt-1 text-fg-subtle">
          <span className="select-none">{"# "}</span>
          {comment}
        </div>
      )}
    </div>
  );
}
