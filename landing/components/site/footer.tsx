export function Footer() {
  return (
    <footer className="relative w-full border-t border-border px-6 py-12 md:px-10 md:py-14">
      <div className="mx-auto max-w-6xl">
        <div className="grid grid-cols-1 gap-8 sm:grid-cols-2 sm:gap-10 lg:grid-cols-[2fr_1fr_1fr_1fr]">
          <div>
            <div className="font-mono text-[15px] tracking-tight text-fg">
              <span className="text-accent">›</span> hellodb
            </div>
            <p className="mt-3 max-w-sm text-[14px] leading-relaxed text-fg-muted text-pretty">
              Sovereign memory for Claude Code. Built in the open in Rust.
              MIT-licensed. v0.1.0.
            </p>
          </div>

          <FooterCol title="product">
            <FooterLink href="#diagram">how it works</FooterLink>
            <FooterLink href="#install">install</FooterLink>
          </FooterCol>

          <FooterCol title="code">
            <FooterLink href="https://github.com/eprasad7/hellodb" external>
              github
            </FooterLink>
            <FooterLink
              href="https://github.com/eprasad7/hellodb/tree/main/crates"
              external
            >
              crates
            </FooterLink>
            <FooterLink
              href="https://github.com/eprasad7/hellodb/tree/main/gateway"
              external
            >
              gateway worker
            </FooterLink>
          </FooterCol>

          <FooterCol title="philosophy">
            <FooterLink href="#diagram">no shared infra</FooterLink>
            <FooterLink href="#install">your keys</FooterLink>
            <FooterLink href="#diagram">open in the wild</FooterLink>
          </FooterCol>
        </div>

        <div className="mt-10 flex flex-col items-start justify-between gap-4 border-t border-border pt-5 text-[12px] text-fg-subtle sm:flex-row sm:items-center">
          <div className="font-mono">
            built fast, kept thin — design and copy iterated in-Claude. inspired
            by the same playbook that produced supermemory.ai.
          </div>
          <div className="font-mono">MIT · 2026</div>
        </div>
      </div>
    </footer>
  );
}

function FooterCol({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div className="mb-3 font-mono text-[11px] uppercase tracking-[0.16em] text-fg-subtle">
        {title}
      </div>
      <ul className="flex flex-col gap-2">{children}</ul>
    </div>
  );
}

function FooterLink({
  href,
  children,
  external,
}: {
  href: string;
  children: React.ReactNode;
  external?: boolean;
}) {
  if (external) {
    return (
      <li>
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer"
          className="font-mono text-[13px] text-fg-muted transition-colors hover:text-fg"
        >
          {children}
        </a>
      </li>
    );
  }
  return (
    <li>
      <a
        href={href}
        className="font-mono text-[13px] text-fg-muted transition-colors hover:text-fg"
      >
        {children}
      </a>
    </li>
  );
}
