<script lang="ts">
  import { initials, projectColorIndex } from './avatar';
  let { path, name, failed = false }: { path: string; name: string; failed?: boolean } = $props();
  const index = $derived(projectColorIndex(path));
</script>
<span class="avatar" class:failed aria-hidden="true"
  style:--avatar-start={`var(--project-avatar-${index}-start)`}
  style:--avatar-end={`var(--project-avatar-${index}-end)`}>{initials(name)}</span>
<style>
  /* RoundRectangle2D arcRatio=.4 is an arc diameter: CSS radius is 20%. */
  .avatar { display: inline-flex; flex: none; align-items: center; justify-content: center;
    width: var(--project-avatar-size); height: var(--project-avatar-size); border-radius: 20%;
    background: linear-gradient(to top right, var(--avatar-end), var(--avatar-start));
    color: var(--project-avatar-text); font-family: var(--font-editor-family);
    font-weight: var(--font-weight-heading); font-size: calc(var(--project-avatar-size) * .65); line-height: 1; }
  .failed { filter: saturate(0); }
</style>
