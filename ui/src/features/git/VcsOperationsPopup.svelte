<script lang="ts">
  import Popup from '../../components/Popup.svelte';
  import type { Rect } from '../../components/popup';
  import type { NativeMenuMessage } from '../../i18n/native-menu';
  import { nativeMenuText } from '../../i18n/native-menu';
  import { vcsOperationsItems } from './vcs-operations';

  let { anchor, enabled, onclose, onaction }: {
    anchor: Rect;
    enabled: Readonly<Record<string, boolean>>;
    onclose: () => void;
    onaction: (action: string) => void;
  } = $props();
  const items = $derived(vcsOperationsItems(enabled, onaction, (key: NativeMenuMessage) => nativeMenuText(key)));
</script>

<div class="operations-popup">
  <Popup label={nativeMenuText('vcsOperationsTitle')} {anchor} {items} quickSelect onclose={onclose} />
</div>

<style>
  .operations-popup { --popup-width: min(420px, calc(100vw - 16px)); --popup-height: min(480px, calc(100vh - 16px)); }
</style>
