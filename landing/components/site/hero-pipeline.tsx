export function HeroPipeline() {
  const stations: {
    x: number;
    label: string;
    desc: string;
    state: "done" | "active" | "next";
    tip: string;
  }[] = [
    { x: 70, label: "note", desc: "instant", state: "done", tip: "hellodb_note returns in 0ms — agent never blocks" },
    { x: 215, label: "brain", desc: "digests", state: "active", tip: "Brain runs on Stop hook, distills episodes into facts" },
    { x: 360, label: "draft", desc: "branched", state: "next", tip: "Facts land on draft/yyyy-mm-dd-brain (not main)" },
    { x: 505, label: "merged", desc: "approved", state: "next", tip: "/hellodb:review — one approve to merge" },
    { x: 650, label: "recall", desc: "semantic", state: "next", tip: "Cosine similarity + time-decay reinforcement" },
  ];

  return (
    <div className="relative w-full overflow-hidden rounded-[var(--radius-card)] border border-border bg-bg-sunken/60 p-6 ring-amber">
      <div className="mb-4 flex items-center justify-between">
        <div className="font-mono text-[11px] uppercase tracking-[0.18em] text-fg-subtle">
          episode lifecycle
        </div>
        <div className="flex items-center gap-1.5 font-mono text-[11px] text-fg-subtle">
          <span className="h-1.5 w-1.5 animate-[pulse-dot_2s_ease-in-out_infinite] rounded-full bg-success" />
          live
        </div>
      </div>

      <svg
        viewBox="0 0 720 200"
        className="h-auto w-full"
        role="img"
        aria-label="Lifecycle of an episode: note, brain, draft, merged, recall"
      >
        <defs>
          <linearGradient id="hp-line" x1="0" x2="1" y1="0" y2="0">
            <stop offset="0" stopColor="var(--color-accent)" stopOpacity="0.15" />
            <stop offset="0.5" stopColor="var(--color-accent)" stopOpacity="0.85" />
            <stop offset="1" stopColor="var(--color-accent)" stopOpacity="0.15" />
          </linearGradient>
          <radialGradient id="hp-glow">
            <stop offset="0" stopColor="var(--color-accent)" stopOpacity="1" />
            <stop offset="1" stopColor="var(--color-accent)" stopOpacity="0" />
          </radialGradient>
          <marker
            id="hp-arrow"
            viewBox="0 0 10 10"
            refX="9"
            refY="5"
            markerWidth="6"
            markerHeight="6"
            orient="auto"
          >
            <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--color-accent)" opacity="0.7" />
          </marker>
        </defs>

        {/* Base track (faint dotted) */}
        <line
          x1="70"
          x2="650"
          y1="100"
          y2="100"
          stroke="var(--color-border-strong)"
          strokeWidth="1"
          strokeDasharray="2 6"
        />

        {/* Active edges — marching dashes between done→active */}
        {stations.slice(0, -1).map((s, i) => {
          const next = stations[i + 1];
          const isActive = s.state === "done" || (s.state === "active" && next.state !== "next");
          return (
            <line
              key={`edge-${i}`}
              x1={s.x + 14}
              x2={next.x - 14}
              y1={100}
              y2={100}
              stroke={isActive ? "url(#hp-line)" : "var(--color-border-strong)"}
              strokeWidth={isActive ? 2.5 : 1.5}
              strokeDasharray={isActive ? "10 6" : undefined}
              strokeLinecap="round"
              markerEnd={isActive ? "url(#hp-arrow)" : undefined}
              style={
                isActive
                  ? { animation: "dash-march 1.6s linear infinite" }
                  : undefined
              }
            />
          );
        })}

        {/* Travelling glow + dot */}
        <circle r="20" fill="url(#hp-glow)" cy="100">
          <animate
            attributeName="cx"
            values="70;650;70"
            dur="9s"
            repeatCount="indefinite"
          />
        </circle>
        <circle r="5" fill="var(--color-accent)" cy="100">
          <animate
            attributeName="cx"
            values="70;650;70"
            dur="9s"
            repeatCount="indefinite"
          />
        </circle>

        {/* Stations — bigger, with state-based fill, hover tooltips */}
        {stations.map((s) => {
          const isDone = s.state === "done";
          const isActive = s.state === "active";
          // Clamp tooltip rect to viewBox bounds (130px wide)
          const tipW = 138;
          const tipH = 22;
          const tipX = Math.max(4, Math.min(s.x - tipW / 2, 720 - tipW - 4));
          return (
            <g key={s.label} className="group">
              {/* Native a11y tooltip */}
              <title>{s.tip}</title>
              {/* Invisible hit target — bigger than the visible node */}
              <circle
                cx={s.x}
                cy={100}
                r={26}
                fill="transparent"
                style={{ cursor: "help" }}
              />
              {/* Outer ring */}
              <circle
                cx={s.x}
                cy={100}
                r={14}
                fill="var(--color-bg-sunken)"
                stroke={
                  isDone || isActive
                    ? "var(--color-accent)"
                    : "var(--color-border-strong)"
                }
                strokeWidth={isActive ? 2 : 1.5}
                className="transition-all duration-150 group-hover:stroke-[var(--color-accent)] group-hover:[stroke-width:2.5]"
              />
              {/* Inner fill */}
              <circle
                cx={s.x}
                cy={100}
                r={6}
                fill={
                  isDone
                    ? "var(--color-accent)"
                    : isActive
                      ? "var(--color-accent)"
                      : "var(--color-border-strong)"
                }
                opacity={isDone ? 0.95 : isActive ? 0.85 : 0.5}
                className="transition-opacity duration-150 group-hover:opacity-100"
              >
                {isActive && (
                  <animate
                    attributeName="opacity"
                    values="0.4;1;0.4"
                    dur="1.6s"
                    repeatCount="indefinite"
                  />
                )}
              </circle>
              {/* Label above */}
              <text
                x={s.x}
                y={66}
                textAnchor="middle"
                className="fill-fg font-mono"
                style={{ fontSize: 14, fontWeight: 500 }}
              >
                {s.label}
              </text>
              {/* Desc below */}
              <text
                x={s.x}
                y={142}
                textAnchor="middle"
                className={
                  isActive ? "fill-accent font-mono" : "fill-fg-subtle font-mono"
                }
                style={{ fontSize: 11 }}
              >
                {s.desc}
              </text>
              {/* Custom tooltip — shown on group hover */}
              <g
                className="pointer-events-none opacity-0 transition-opacity duration-150 group-hover:opacity-100"
              >
                <rect
                  x={tipX}
                  y={6}
                  width={tipW}
                  height={tipH}
                  rx={4}
                  fill="var(--color-bg-elevated)"
                  stroke="var(--color-accent)"
                  strokeOpacity={0.4}
                />
                <text
                  x={tipX + tipW / 2}
                  y={21}
                  textAnchor="middle"
                  className="fill-fg font-mono"
                  style={{ fontSize: 10 }}
                >
                  {s.tip}
                </text>
              </g>
            </g>
          );
        })}

        {/* Footer rule labels */}
        <text
          x="70"
          y="180"
          className="fill-fg-subtle font-mono"
          style={{ fontSize: 10, letterSpacing: 1.2 }}
        >
          WRITE
        </text>
        <text
          x="360"
          y="180"
          textAnchor="middle"
          className="fill-fg-subtle font-mono"
          style={{ fontSize: 10, letterSpacing: 1.2 }}
        >
          BRAIN (ASYNC)
        </text>
        <text
          x="650"
          y="180"
          textAnchor="end"
          className="fill-fg-subtle font-mono"
          style={{ fontSize: 10, letterSpacing: 1.2 }}
        >
          RECALL
        </text>
      </svg>
    </div>
  );
}
