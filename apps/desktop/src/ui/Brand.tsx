import type { ReactElement } from "react";

/** Símbolo do app: o "P" em pixel art do ícone, desenhado em SVG. */
export function BrandMark({ size = 22 }: { size?: number }) {
  const P = [
    "................",
    "..bbbbbbbbbbbb..",
    ".bbbbbbbbbbbbbb.",
    ".bbbbPPPPPPbbbb.",
    ".bbbbPPPPPPPbbb.",
    ".bbbbPPbbbPPbbb.",
    ".bbbbPPbbbPPbbb.",
    ".bbbbPPPPPPPbbb.",
    ".bbbbPPPPPPbbbb.",
    ".bbbbPPbbbbbbbb.",
    ".bbbbPPbbbccbbb.",
    ".bbbbPPbbbccbbb.",
    ".bbbbPPbbbbbbbb.",
    ".bbbbbbbbbbbbbb.",
    "..bbbbbbbbbbbb..",
    "................",
  ];
  const grad = ["#ff6fa8", "#ff7aa0", "#ff8698", "#ff9290", "#ff9e88", "#ffaa80", "#ffb678", "#ffc270", "#ffce68", "#ffd25f"];
  const cells: ReactElement[] = [];
  P.forEach((row, y) =>
    [...row].forEach((ch, x) => {
      if (ch === ".") return;
      const fill = ch === "b" ? "#252940" : ch === "c" ? "#5fe3ff" : grad[Math.min(9, Math.max(0, y - 3))];
      cells.push(<rect key={`${x}-${y}`} x={x} y={y} width={1.02} height={1.02} fill={fill} />);
    }),
  );
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" shapeRendering="crispEdges" aria-hidden>
      {cells}
    </svg>
  );
}
