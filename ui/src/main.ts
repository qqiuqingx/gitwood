import { mount } from 'svelte';
import App from './App.svelte';
import './theme/light.css';
import './app.css';

const target = document.getElementById('app');
if (!target) throw new Error('Missing application root');

if (import.meta.env.DEV && location.hash === '#gallery') {
  void import('./features/dev-gallery/Gallery.svelte').then(({ default: Gallery }) => mount(Gallery, { target }));
} else {
  mount(App, { target });
}
