import { describe, expect, it } from 'vitest';
import { defaultLayout, layoutRequest, panelSplit } from '../src/layout/state';

describe('panel visibility and retained split ratios', () => {
  it('restores the user size after hide/show for Commit and Git Log', () => {
    const layout = structuredClone(defaultLayout);
    layout.ratios.commitDiff = 0.35;
    layout.ratios.workspaceLog = 0.65;
    const commit = panelSplit(layout.ratios.commitDiff, 1000, 220, 420, true, true);
    const workspace = panelSplit(layout.ratios.workspaceLog, 800, 240, 240, true, true);
    expect(panelSplit(layout.ratios.commitDiff, 1000, 220, 420, false, true).first).toBe(0);
    expect(panelSplit(layout.ratios.workspaceLog, 800, 240, 240, true, false).first).toBe(800);
    expect(panelSplit(layout.ratios.commitDiff, 1000, 220, 420, true, true)).toEqual(commit);
    expect(panelSplit(layout.ratios.workspaceLog, 800, 240, 240, true, true)).toEqual(workspace);
    expect(layout.ratios.commitDiff).toBe(0.35);
    expect(layout.ratios.workspaceLog).toBe(0.65);
  });
  it('reapplies minimums after hiding and shrinking, then recovers at the original size', () => {
    const ratio = 0.5;
    expect(panelSplit(ratio, 1000, 220, 420, true, true).first).toBe(500);
    expect(panelSplit(ratio, 300, 220, 420, false, true).second).toBe(300);
    const restored = panelSplit(ratio, 700, 220, 420, true, true);
    expect(restored.first).toBe(280);
    expect(restored.second).toBe(420);
    expect(panelSplit(ratio, 1000, 220, 420, true, true).first).toBe(500);
    expect(panelSplit(ratio, 0, 220, 420, true, true).first).toBe(0);
  });
});

describe('persisted Commit view toggles', () => {
  it('defaults to a flat list with the diff preview on, like the panel before it was stored', () => {
    expect(defaultLayout.commitFlat).toBe(true);
    expect(defaultLayout.commitPreview).toBe(true);
    expect(defaultLayout.ratios.diffVersions).toBe(0.5);
  });
  it('saves ratios and both Commit view toggles in one request, without tool windows', () => {
    const layout = { ...structuredClone(defaultLayout), commitFlat: false, commitVisible: false };
    expect(layoutRequest(layout)).toEqual({ ratios: defaultLayout.ratios, commitFlat: false, commitPreview: true });
    expect(layoutRequest({ ...layout, commitPreview: false })).toEqual({ ratios: defaultLayout.ratios, commitFlat: false, commitPreview: false });
  });
});
