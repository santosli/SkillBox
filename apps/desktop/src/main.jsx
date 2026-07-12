import React from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.jsx';
import './colors.css';
import './styles.css';

const rootElement = document.getElementById('root');

function applyTextInputDefaults(node) {
  if (!(node instanceof Element)) return;
  if (node.matches('input, textarea')) {
    node.setAttribute('autocapitalize', 'none');
    node.setAttribute('autocomplete', 'off');
    node.setAttribute('autocorrect', 'off');
    node.setAttribute('spellcheck', 'false');
  }
  node.querySelectorAll('input, textarea').forEach((field) => {
    field.setAttribute('autocapitalize', 'none');
    field.setAttribute('autocomplete', 'off');
    field.setAttribute('autocorrect', 'off');
    field.setAttribute('spellcheck', 'false');
  });
}

new MutationObserver((mutations) => {
  mutations.forEach((mutation) => {
    mutation.addedNodes.forEach(applyTextInputDefaults);
  });
}).observe(rootElement, { childList: true, subtree: true });

applyTextInputDefaults(rootElement);

createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
