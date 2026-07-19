

export function Sparkline({
  values,
  width = 260,
  height = 44,
  className = "",
}: {
  values: number[];
  width?: number;
  height?: number;
  className?: string;
}) {
  const pad = 3;
  const max = Math.max(...values, 1);
  const n = values.length;
  const line = values
    .map((v, i) => {
      const x = n === 1 ? width - pad : (i / (n - 1)) * (width - pad * 2) + pad;
      const y = height - pad - (v / max) * (height - pad * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const area = `${pad},${height - pad} ${line} ${width - pad},${height - pad}`;
  return (
    <svg
      className={`spark ${className}`}
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      aria-hidden
    >
      <polygon className="spark__fill" points={area} />
      <polyline className="spark__line" points={line} />
    </svg>
  );
}
