import { EditorState, Prec, RangeSet, type Extension } from '@codemirror/state';
import { Decoration, EditorView, WidgetType, lineNumbers, gutter, GutterMarker, type DecorationSet } from '@codemirror/view';
import { collapsedFolds } from './model';
import type { DiffResult, Side } from './types';
import { diffText as t } from '../../i18n/diff';
import { commitText as c } from '../../i18n/commit';

class FoldWidget extends WidgetType {
  constructor(readonly id: number, readonly count: number, readonly expand: (id: number) => void) { super(); }
  eq(other: FoldWidget) { return this.id === other.id && this.count === other.count; }
  toDOM() {
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'diff-fold';
    button.textContent = `${t('expand')} (${this.count})`;
    button.onclick = () => this.expand(this.id);
    return button;
  }
  ignoreEvent() { return true; }
}
export function linePosition(state: EditorState, line: number): number {
  return line >= state.doc.lines ? state.doc.length : state.doc.line(Math.max(0, line) + 1).from;
}
/** Document-local boundary, excluding editor padding even at EOF or a folded tail. */
export function lineBoundary(view: EditorView, line: number): number {
  return line >= view.state.doc.lines ? view.lineBlockAt(view.state.doc.length).bottom
    : view.lineBlockAt(linePosition(view.state, line)).top;
}
export function diffDecorations(state: EditorState, side: Side, result: DiffResult, expanded: ReadonlySet<number>, expand: (id: number) => void): DecorationSet {
  const ranges = [];
  for (const block of result.blocks) {
    const range = block[side];
    for (let line = range.start; line < range.end && line < state.doc.lines; line++) {
      ranges.push(Decoration.line({ class: `diff-${block.kind}` }).range(linePosition(state, line)));
    }
    for (const inline of side === 'left' ? block.leftInline : block.rightInline) {
      if (inline.line >= state.doc.lines) continue;
      const line = state.doc.line(inline.line + 1);
      const from = line.from + Math.min(inline.start, line.length), to = line.from + Math.min(inline.end, line.length);
      if (to > from) ranges.push(Decoration.mark({ class: `diff-inline diff-inline-${side}` }).range(from, to));
    }
  }
  for (const { id, range } of collapsedFolds(result.folds, expanded)) {
    const span = range[side];
    const from = linePosition(state, span.start), to = linePosition(state, span.end);
    if (to > from) ranges.push(Decoration.replace({ block: true, widget: new FoldWidget(id, span.end - span.start, expand) }).range(from, to));
  }
  return Decoration.set(ranges, true);
}
class LineNumber extends GutterMarker {
  constructor(readonly number: number) { super(); }
  eq(other: LineNumber) { return this.number === other.number; }
  toDOM() { return document.createTextNode(String(this.number)); }
}
// @codemirror/view 6.43.15 supports native after-content gutters. Keep CM's
// sticky positioning and block/widget measurements without reversing text direction.
const leftLineNumbers = gutter({
  class: 'cm-lineNumbers', side: 'after',
  lineMarker: (view, line) => new LineNumber(view.state.doc.lineAt(line.from).number),
  initialSpacer: view => new LineNumber(10 ** String(view.state.doc.lines).length - 1),
});
export function editorExtensions(side: Side): Extension { return [
  EditorState.readOnly.of(true), EditorView.editable.of(false), side === 'left' ? leftLineNumbers : lineNumbers(),
  EditorView.contentAttributes.of({ tabindex: '0', 'aria-readonly': 'true' }),
  EditorView.updateListener.of(({ view }) => {
    // CM hides decorative gutters from readers by default; these contain native controls.
    for (const container of view.scrollDOM.querySelectorAll('.cm-gutters')) {
      if (container.querySelector('.commit-inclusion, .diff-rollback')) container.removeAttribute('aria-hidden');
    }
  }),
]; }

class InclusionMarker extends GutterMarker {
  constructor(readonly index: number, readonly included: boolean, readonly disabled: boolean, readonly toggle: (index: number) => void) { super(); }
  eq(other: InclusionMarker) { return this.index === other.index && this.included === other.included && this.disabled === other.disabled; }
  toDOM() {
    const checkbox=document.createElement('input'); checkbox.type='checkbox'; checkbox.checked=this.included; checkbox.disabled=this.disabled;
    checkbox.setAttribute('aria-label',`${c('include')} ${this.index+1}`); checkbox.title=c('include');
    checkbox.onclick=event=>event.stopPropagation(); checkbox.onchange=()=>this.toggle(this.index); return checkbox;
  }
}
export function inclusionGutter(result: DiffResult, included: readonly number[], disabled: boolean, toggle: (index: number) => void) {
  return gutter({class:'commit-inclusion', markers: view => RangeSet.of(result.blocks.map((block,index)=>new InclusionMarker(index,included.includes(index),disabled,toggle).range(markerPosition(view.state,block.right.start))),true)});
}
class RollbackMarker extends GutterMarker {
  constructor(readonly index: number, readonly disabled: boolean, readonly revert: (index: number) => void) { super(); }
  eq(other: RollbackMarker) { return this.index === other.index && this.disabled === other.disabled; }
  toDOM() {
    const button = document.createElement('button');
    button.type = 'button'; button.textContent = '»'; button.disabled = this.disabled;
    button.title = c('revert'); button.setAttribute('aria-label', `${c('revert')} ${this.index + 1}`);
    button.onclick = event => { event.stopPropagation(); this.revert(this.index); };
    return button;
  }
}
function markerPosition(state: EditorState, line: number) {
  // Empty spans at EOF still need a marker on the final visible line.
  return state.doc.line(Math.min(line + 1, state.doc.lines)).from;
}
export function rollbackGutter(result: DiffResult, disabled: boolean, revert: (index: number) => void) {
  // CM appends after-gutters in facet order; precedence keeps IDEA's "» 20 |" order (arrow, then number by the divider).
  return Prec.high(gutter({ class: 'diff-rollback', side: 'after', markers: view => RangeSet.of(result.blocks.map((block, index) =>
    new RollbackMarker(index, disabled, revert).range(markerPosition(view.state, block.left.start))), true) }));
}
