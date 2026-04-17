import type { Metadata } from "next";
import Link from "next/link";
import { Nav } from "@/components/site/nav";
import { Footer } from "@/components/site/footer";

export const metadata: Metadata = {
  title: "Blog",
  description:
    "Notes on sovereign memory, Claude Code, agent workflows, and the quiet engineering under hellodb.",
  alternates: { canonical: "https://hellodb.dev/blog" },
};

type Post = {
  slug: string;
  title: string;
  date: string;
  description: string;
  readingTime: string;
};

// Posts are hand-curated for now. When this grows past ~10 entries, swap to
// content-collections or MDX-on-disk. Keeping it flat while the list is small.
const POSTS: Post[] = [
  {
    slug: "context-is-triage-memory-is-engineering",
    title: "Context is triage. Memory is engineering.",
    date: "2026-04-17",
    description:
      "Anthropic just published a clear-eyed post on Claude Code session management. It names three problems — context rot, lossy /compact, hand-written /clear briefs. hellodb was built to make each of those problems go away.",
    readingTime: "8 min",
  },
];

export default function BlogIndexPage() {
  return (
    <>
      <Nav />
      <main className="flex flex-col">
        <section className="relative mx-auto w-full max-w-6xl px-6 pt-20 pb-16 md:px-10 md:pt-28 md:pb-24">
          <div className="max-w-3xl">
            <div className="font-mono text-[11px] uppercase tracking-[0.18em] text-accent">
              blog
            </div>
            <h1 className="mt-4 font-display text-[40px] leading-[1.05] tracking-tight text-balance text-fg md:text-[56px]">
              Notes on sovereign memory,{" "}
              <span className="italic text-accent">one post at a time</span>.
            </h1>
            <p className="mt-6 max-w-2xl text-[16px] leading-relaxed text-fg-muted text-pretty md:text-[17px]">
              Short, technical posts on the problems hellodb was built to solve —
              context rot, session continuity, agent trust, the difference
              between a summary and a fact.
            </p>
          </div>
        </section>

        <section className="relative mx-auto w-full max-w-6xl px-6 pb-24 md:px-10 md:pb-32">
          <ul className="flex flex-col divide-y divide-border border-y border-border">
            {POSTS.map((post) => (
              <li key={post.slug}>
                <Link
                  href={`/blog/${post.slug}`}
                  className="group flex flex-col gap-3 py-8 transition-colors hover:bg-bg-elevated/40 md:flex-row md:items-start md:gap-10 md:px-4"
                >
                  <div className="flex shrink-0 flex-col gap-1 font-mono text-[12px] text-fg-subtle md:w-36 md:pt-1">
                    <time dateTime={post.date}>{formatDate(post.date)}</time>
                    <span>{post.readingTime}</span>
                  </div>
                  <div className="flex flex-1 flex-col gap-2">
                    <h2 className="font-display text-[24px] leading-tight tracking-tight text-fg transition-colors group-hover:text-accent md:text-[28px]">
                      {post.title}
                    </h2>
                    <p className="text-[15px] leading-relaxed text-fg-muted text-pretty">
                      {post.description}
                    </p>
                    <span className="mt-1 inline-flex items-center gap-1.5 font-mono text-[12px] text-accent">
                      read
                      <span aria-hidden="true" className="transition-transform group-hover:translate-x-0.5">
                        →
                      </span>
                    </span>
                  </div>
                </Link>
              </li>
            ))}
          </ul>
        </section>
      </main>
      <Footer />
    </>
  );
}

function formatDate(iso: string): string {
  const d = new Date(iso + "T00:00:00Z");
  return d.toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  });
}
