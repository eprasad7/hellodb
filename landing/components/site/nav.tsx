import Link from "next/link";

export function Nav() {
  return (
    <header className="sticky top-0 z-50 w-full border-b border-border/60 bg-bg/70 backdrop-blur-xl">
      <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-6 md:px-10">
        <Link
          href="/"
          className="font-mono text-[15px] tracking-tight text-fg"
          aria-label="hellodb home"
        >
          <span className="text-accent">›</span> hellodb
        </Link>
        <nav className="flex items-center gap-1 sm:gap-2">
          <NavLink href="#diagram">how it works</NavLink>
          <NavLink href="#install">install</NavLink>
          <NavLink
            href="https://github.com/eprasad7/hellodb"
            external
          >
            github
          </NavLink>
          <Link
            href="#install"
            className="ml-2 inline-flex h-9 items-center gap-2 rounded-full border border-accent/40 bg-accent/10 px-4 font-mono text-[13px] text-accent transition-colors hover:border-accent hover:bg-accent/15"
          >
            install
          </Link>
        </nav>
      </div>
    </header>
  );
}

function NavLink({
  href,
  children,
  external,
}: {
  href: string;
  children: React.ReactNode;
  external?: boolean;
}) {
  const className =
    "hidden h-9 items-center rounded-full px-3 font-mono text-[13px] text-fg-muted transition-colors hover:text-fg sm:inline-flex";
  if (external) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className={className}
      >
        {children}
      </a>
    );
  }
  return (
    <Link href={href} className={className}>
      {children}
    </Link>
  );
}
