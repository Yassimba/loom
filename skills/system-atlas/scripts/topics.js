// Required topic behavior follows local nav overrides, preserving their customization.
(() => {
  for (const figure of document.querySelectorAll('figure[data-source-links]')) {
    const links = JSON.parse(figure.dataset.sourceLinks);
    for (const element of figure.querySelectorAll('[data-code]')) {
      const binding = element.dataset.code.split(',')[0].trim();
      const repo = element.dataset.repo || figure.dataset.repo;
      const href = links[`${repo}:${binding}`];
      if (!href || element.closest('a')) continue;
      const link = document.createElementNS('http://www.w3.org/2000/svg', 'a');
      link.setAttribute('href', href);
      link.setAttribute('aria-label', `Open source: ${binding}`);
      element.before(link);
      link.append(element);
    }
  }
  if (typeof locations !== 'undefined') {
    locations.push(...document.querySelectorAll('main .atlas-topic[id]'));
    locations.sort((a, b) => a.compareDocumentPosition(b) & 2 ? 1 : -1);
  }
  const input = document.querySelector('.toc-search');
  if (!input) return;
  input.placeholder = 'Search topics, symbols, and figures';
  input.setAttribute('aria-label', 'Search atlas');
  const empty = document.querySelector('.toc-empty');
  if (empty) empty.textContent = 'No atlas entries match that search.';
  input.addEventListener('input', () => {
    const terms = input.value.toLowerCase().trim().split(/\s+/).filter(Boolean);
    const items = [...document.querySelectorAll('.toc-root li[data-search]')];
    for (const item of items) {
      item.classList.toggle('is-hidden', terms.length > 0 && !terms.some(term => item.dataset.search.includes(term)));
    }
    for (const group of document.querySelectorAll('.toc-root > li')) {
      const matches = [...group.querySelectorAll('li[data-search]')].some(item => !item.classList.contains('is-hidden'));
      group.classList.toggle('is-hidden', terms.length > 0 && !matches);
    }
    empty?.classList.toggle('is-shown', terms.length > 0 && !items.some(item => !item.classList.contains('is-hidden')));
  });
})();
