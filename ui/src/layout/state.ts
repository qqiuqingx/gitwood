import { constrainSplit } from '../components/splitter';

export interface Ratios {
  workspaceLog: number;
  commitDiff: number;
  diffVersions: number;
  branchesCommits: number;
  commitsInformation: number;
  filesDetails: number;
}
export interface Layout {
  ratios: Ratios;
  commitVisible: boolean;
  logVisible: boolean;
  /** Commit panel list is flat (no directory grouping). */
  commitFlat: boolean;
  /** Commit panel shows the selected file's diff. */
  commitPreview: boolean;
}
export const defaultLayout: Layout = {
  ratios: { workspaceLog: 0.59, commitDiff: 0.22, diffVersions: 0.5,
    branchesCommits: 0.18, commitsInformation: 0.7, filesDetails: 0.6 },
  commitVisible: true,
  logVisible: true,
  commitFlat: true,
  commitPreview: true,
};

/**
 * IPC payload for the layout this window owns: split ratios and Commit panel view toggles.
 * Tool window visibility is excluded; it changes through the native menu.
 */
export function layoutRequest(layout: Pick<Layout, 'ratios' | 'commitFlat' | 'commitPreview'>) {
  return { ratios: layout.ratios, commitFlat: layout.commitFlat, commitPreview: layout.commitPreview };
}

/** Hidden panes occupy no space; the caller retains the user's ratio unchanged. */
export function panelSplit(
  ratio: number, available: number, minFirst: number, minSecond: number,
  firstVisible: boolean, secondVisible: boolean,
) {
  const total = Math.max(0, available);
  if (!firstVisible) return { first: 0, second: secondVisible ? total : 0, min: 0, max: 0 };
  if (!secondVisible) return { first: total, second: 0, min: total, max: total };
  return constrainSplit(total * ratio, total, minFirst, minSecond);
}
