/** The Islandy mark: a pill on a dark rounded square (matches the app icon). */
export function IslandyMark({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden="true" className="shrink-0">
      <rect x="64" y="64" width="896" height="896" rx="224" fill="#080808" />
      <rect
        x="64.5"
        y="64.5"
        width="895"
        height="895"
        rx="223.5"
        fill="none"
        stroke="#ffffff"
        strokeOpacity="0.14"
        strokeWidth="8"
      />
      <rect x="232" y="420" width="560" height="184" rx="92" fill="#f5f5f5" />
    </svg>
  );
}
