import { mount } from 'svelte';
import App from './App.svelte';
import './theme/light.css';
import './app.css';
import { allowsNativeContextMenu } from './components/context-menu';

// WebView's Reload menu is unrelated to Git actions. Editors retain native copy/edit menus.
document.addEventListener('contextmenu', event => {
  if (!allowsNativeContextMenu(event.target)) event.preventDefault();
});

const target = document.getElementById('app');
if (!target) throw new Error('Missing application root');

if (import.meta.env.DEV && location.hash === '#gallery') {
  void import('./features/dev-gallery/Gallery.svelte').then(({ default: Gallery }) => mount(Gallery, { target }));
} else {
  mount(App, { target });
}
