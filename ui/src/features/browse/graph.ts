import type { HistoryRow } from './types';
export type DrawCommand =
  | { kind: 'edge'; color: number; dotted: boolean; from: [number, number]; to: [number, number]; phase: number }
  | { kind: 'node'; color: number; x: number; y: number; head: boolean };
/** Each edge is two half-row segments. Adjacent pages share exactly one endpoint. */
export function graphCommands(rows: readonly HistoryRow[], scrollTop: number, rowHeight: number, height: number, head: string | null, laneWidth = 16): DrawCommand[] {
  const edges: DrawCommand[] = [], nodes: DrawCommand[] = [];
  for (const row of rows) {
    const y = (row.index + 0.5) * rowHeight - scrollTop;
    if (y + rowHeight / 2 < 0 || y - rowHeight / 2 > height) continue;
    for (const [, from, to, color, direction, dotted] of row.segments) {
      edges.push({ kind: 'edge', color, dotted,
        from: [(from + 1) * laneWidth, y], to: [((from + to) / 2 + 1) * laneWidth, y + direction * rowHeight / 2],
        phase: (row.index + (direction < 0 ? 0 : 0.5)) * rowHeight });
    }
    nodes.push({ kind: 'node', color: row.color, x: (row.column + 1) * laneWidth, y, head: row.commit.hash === head });
  }
  return [...edges, ...nodes]; // Nodes cover lines at junctions.
}
export function drawGraph(context: CanvasRenderingContext2D, commands: readonly DrawCommand[], colors: readonly string[], headColor: string): void {
  context.lineWidth = 1.5;
  for (const command of commands) {
    const color = colors[Math.abs(command.color) % colors.length];
    context.beginPath(); context.strokeStyle = color; context.fillStyle = color;
    if (command.kind === 'edge') {
      context.setLineDash(command.dotted ? [2, 3] : []);
      context.lineDashOffset = -command.phase % 5;
      // Orient every half downwards so dash phase is independent of Up/Down.
      const [from, to] = command.from[1] <= command.to[1] ? [command.from, command.to] : [command.to, command.from];
      context.moveTo(...from); context.lineTo(...to); context.stroke();
    } else {
      context.setLineDash([]); context.arc(command.x, command.y, 3.5, 0, Math.PI * 2); context.fill();
      if (command.head) { context.beginPath(); context.strokeStyle = headColor; context.arc(command.x, command.y, 6, 0, Math.PI * 2); context.stroke(); }
    }
  }
}
