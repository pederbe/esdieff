// Brand the sidebar title as a route back to the homepage, and add the project footer.
(function () {
  var title = document.querySelector(".menu-title");
  if (title) {
    title.innerHTML =
      '<a class="docs-brand" href="/" aria-label="Esdieff homepage">' +
      '<svg width="22" height="22" viewBox="0 0 32 32" aria-hidden="true" focusable="false">' +
      '<g fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round">' +
      '<path d="M21 16a8 8 0 1 0-2.2 5.5"/><path d="M5 16h19.5"/></g>' +
      '<circle cx="26.5" cy="16" r="2.6" fill="currentColor"/></svg>Esdieff</a>' +
      '<a class="docs-home" href="/docs/">Docs</a>';
  }
  var main = document.querySelector("main");
  if (!main) return;
  var footer = document.createElement("footer");
  footer.className = "esdieff-footer";
  footer.innerHTML =
    'Esdieff is free software under MIT OR Apache-2.0 · ' +
    '<a href="https://github.com/pederbe/esdieff">Source</a> · ' +
    'Built by <a href="https://pederbe.dev" rel="author me">Peder Bergan</a>';
  main.appendChild(footer);
})();
