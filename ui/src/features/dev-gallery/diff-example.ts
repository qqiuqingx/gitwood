import type { Alignment, ChangeBlock, DiffResult, LineRange } from '../diff/types';
// Built-in, deterministic demonstration only. No repository reads or IPC.
const lines = Array.from({ length: 50 }, (_, i) => `// unchanged line ${i + 1}`);
lines[6] = "const city = '杭州';";
lines[16] = 'const obsolete = true;';
lines[26] = 'return total + fee;';
const next = [...lines]; next[6] = "const city = '上海';"; next[26] = 'return total - fee;';
next.splice(36, 0, 'const added = 1;', 'const another = 2;'); next.splice(16, 1);
export const diffLeft = lines.join('\n');
export const diffRight = next.join('\n') + '\n';
const range = (start: number, end: number): LineRange => ({ start, end });
const block = (kind: ChangeBlock['kind'], left: LineRange, right: LineRange): ChangeBlock => ({ kind, left, right, leftInline: [], rightInline: [] });
const blocks: ChangeBlock[] = [
  { ...block('modify', range(6,7), range(6,7)), leftInline: [{ line: 6, start: 14, end: 16 }], rightInline: [{ line: 6, start: 14, end: 16 }] },
  block('delete', range(16,17), range(16,16)),
  { ...block('modify', range(26,27), range(25,26)), leftInline: [{ line: 26, start: 13, end: 14 }], rightInline: [{ line: 25, start: 13, end: 14 }] },
  block('insert', range(36,36), range(35,37)),
  block('modify', range(49,50), range(50,51)),
];
const align = (ls: number, le: number, rs: number, re: number): Alignment => ({ left: range(ls, le), right: range(rs, re) });
export const diffExample: DiffResult = { blocks, leftLines: 50, rightLines: 51,
  alignment: [align(0,6,0,6), blocks[0], align(7,16,7,16), blocks[1], align(17,26,16,25), blocks[2], align(27,36,26,35), blocks[3], align(36,49,37,50), blocks[4]],
  folds: [align(0,2,0,2), align(40,45,41,46)],
};
