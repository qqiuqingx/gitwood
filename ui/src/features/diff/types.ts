/** Mirrors gitwood_core::diff serde output. Lines are zero-based, half-open;
 * inline columns are UTF-16, excluding line terminators. */
export interface LineRange { start: number; end: number }
export type ChangeKind = 'insert' | 'delete' | 'modify';
export interface InlineRange { line: number; start: number; end: number }
export interface ChangeBlock {
  kind: ChangeKind; left: LineRange; right: LineRange;
  leftInline: readonly InlineRange[]; rightInline: readonly InlineRange[];
}
export interface Alignment { left: LineRange; right: LineRange }
export interface FoldRange { left: LineRange; right: LineRange }
export interface DiffResult {
  blocks: readonly ChangeBlock[]; alignment: readonly Alignment[]; folds: readonly FoldRange[];
  leftLines: number; rightLines: number;
}
export type Content =
  | { kind: 'text'; text: string; byteLen: number; empty: boolean; missingFinalNewline: boolean }
  | { kind: 'binary' | 'undecodable'; byteLen: number }
  | { kind: 'tooLarge'; byteLen: number; limit: number };
export type Side = 'left' | 'right';
