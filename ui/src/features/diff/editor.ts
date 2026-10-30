import { EditorState, type Extension } from '@codemirror/state';
import { Decoration, EditorView, WidgetType, lineNumbers, type DecorationSet } from '@codemirror/view';
import { collapsedFolds } from './model';
import type { DiffResult, Side } from './types';
import { diffText as t } from '../../i18n/diff';

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
export const editorExtensions: Extension = [
  EditorState.readOnly.of(true), EditorView.editable.of(false), lineNumbers(),
  EditorView.contentAttributes.of({ tabindex: '0', 'aria-readonly': 'true' }),
];

import { gutter, GutterMarker } from '@codemirror/view';
import { RangeSet } from '@codemirror/state';
import { commitText as c } from '../../i18n/commit';
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
  return gutter({class:'commit-inclusion', markers: view => RangeSet.of(result.blocks.map((block,index)=>new InclusionMarker(index,included.includes(index),disabled,toggle).range(linePosition(view.state,block.left.start))),true)});
}
