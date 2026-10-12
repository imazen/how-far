const $ = id => document.getElementById(id);
const demo = document.querySelector('.demo');
const buttons = [...document.querySelectorAll('[data-scenario]')];
const scenarios = buttons.map(button => button.dataset.scenario);
const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
let traces, scenario = 'sequence', position = 0, animation, origin, looping = false;
let manual = false, inView = false, lastEvent = -1, lastDiagnostic = false;
const nodes = new Map();
const el = (tag, cls, text) => {
  const node = document.createElement(tag);
  node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
};
const percent = fraction => fraction === null ? 'Unknown' : `${(fraction * 100).toFixed(1).replace('.0', '')}%`;
const frames = () => traces[scenario].frames;
const events = () => traces[scenario].events;
function treeNode(node) {
  let view = nodes.get(node.id);
  if (!view) {
    const box = el('div', 'node');
    const heading = el('div', 'node-heading');
    const state = el('span', 'node-state');
    heading.append(el('span', 'status-dot'), el('span', 'node-name', node.name), state);
    const track = el('div', 'track');
    const fill = el('span', '');
    const unknown = el('span', 'unknown-part');
    unknown.setAttribute('aria-hidden', 'true');
    track.append(fill, unknown);
    const details = el('div', 'node-details');
    const children = el('div', 'node-children');
    box.append(heading, track, details);
    if (node.children.length) box.append(children);
    view = { box, state, fill, unknown, details, children };
    nodes.set(node.id, view);
  }
  const status = node.outcome || node.status;
  view.box.dataset.status = status;
  view.state.textContent = status;
  view.fill.style.width = `${Math.max(0, Math.min(1, node.fraction ?? 0)) * 100}%`;
  view.unknown.style.width = `${node.unresolved_fraction * 100}%`;
  view.unknown.title = `${percent(node.unresolved_fraction)} of the budget has an unknown total`;
  const total = node.total?.count;
  view.details.textContent = `${percent(node.fraction)} · ${Number(node.completed).toLocaleString()}${total ? ' / ' + Number(total).toLocaleString() : ''} units${node.unresolved_fraction ? ' · ' + percent(node.unresolved_fraction) + ' unresolved' : ''}${node.completion_inferred ? ' · inferred' : ''}`;
  for (const child of node.children) {
    const box = treeNode(child);
    if (box.parentElement !== view.children) view.children.append(box);
  }
  return view.box;
}
function render() {
  const frame = frames()[position];
  const event = events()[frame.event];
  if (lastEvent !== frame.event) {
    $('event-count').textContent = `STEP ${frame.event + 1} / ${events().length}`;
    $('event-title').textContent = event.title;
    $('event-detail').textContent = event.detail;
    $('event-code').textContent = event.code;
    lastEvent = frame.event;
  }
  const root = treeNode(frame.snapshot.root);
  if (root.parentElement !== $('tree')) $('tree').replaceChildren(root);
  $('overall').textContent = percent(frame.snapshot.root.fraction);
  $('event-position').max = frames().length - 1;
  $('event-position').value = position;
  $('previous').disabled = position === 0;
  $('next').disabled = position === frames().length - 1;
  $('smoothing').hidden = scenario !== 'smooth';
  if (scenario === 'smooth') {
    $('recorded-value').textContent = percent(frame.snapshot.root.fraction);
    $('smoothed-value').textContent = percent(frame.display_fraction);
    $('recorded-bar').style.width = `${(frame.snapshot.root.fraction ?? 0) * 100}%`;
    $('smoothed-bar').style.width = `${(frame.display_fraction ?? 0) * 100}%`;
  }
  const showDiagnostic = scenario === 'diagnostics' && position === frames().length - 1;
  $('diagnostic').hidden = !showDiagnostic;
  if (showDiagnostic && !lastDiagnostic) {
    const finding = traces[scenario].findings[0];
    $('diagnostic-kind').textContent = finding.kind;
    $('diagnostic-evidence').textContent = finding.evidence;
    $('diagnostic-advice').textContent = finding.advice;
  }
  lastDiagnostic = showDiagnostic;
}
function pause(user = false) {
  cancelAnimationFrame(animation);
  animation = undefined;
  looping = false;
  if (user) manual = true;
  $('play').textContent = 'Play';
  $('demo-note').textContent = 'Illustrative pace · actual tracker states';
  // Announce semantic changes when stepping; never stream every counter update.
  $('event-panel').setAttribute('aria-live', 'polite');
}
function choose(key, transition = true) {
  scenario = key;
  position = 0;
  lastEvent = -1;
  lastDiagnostic = false;
  nodes.clear();
  $('tree').replaceChildren();
  demo.dataset.scenario = key;
  for (const button of buttons) button.setAttribute('aria-pressed', String(button.dataset.scenario === key));
  if (transition && !reducedMotion.matches) {
    for (const panel of [ $('event-panel'), $('tree-panel') ]) {
      panel.animate([{ opacity: 0.7 }, { opacity: 1 }], { duration: 160, easing: 'ease-out' });
    }
  }
  render();
}
function tick(now) {
  const elapsed = now - origin;
  while (position + 1 < frames().length && frames()[position + 1].at_ms <= elapsed) position++;
  render();
  const finished = position === frames().length - 1;
  if (finished && !looping) return pause();
  if (finished && elapsed > frames().at(-1).at_ms + 1600) {
    choose(scenarios[(scenarios.indexOf(scenario) + 1) % scenarios.length]);
    origin = now;
  }
  animation = requestAnimationFrame(tick);
}
function play(cycle = false) {
  cancelAnimationFrame(animation);
  if (position === frames().length - 1) choose(scenario, false);
  looping = cycle;
  origin = performance.now() - frames()[position].at_ms;
  $('play').textContent = 'Pause';
  $('demo-note').textContent = cycle ? 'Playing · cycles through all six examples' : 'Playing · illustrative pace';
  $('event-panel').setAttribute('aria-live', 'off');
  animation = requestAnimationFrame(tick);
}
function maybeAutoplay() {
  if (traces && inView && !manual && !reducedMotion.matches && !document.hidden) play(true);
}
for (const button of buttons) button.addEventListener('click', () => {
  pause(true);
  choose(button.dataset.scenario);
  if (!reducedMotion.matches) play();
});
$('event-position').addEventListener('input', event => { pause(true); position = Number(event.target.value); render(); });
$('previous').addEventListener('click', () => {
  pause(true);
  const current = frames()[position].event;
  position = events()[Math.max(0, current - 1)].frame;
  render();
});
$('next').addEventListener('click', () => {
  pause(true);
  const next = frames()[position].event + 1;
  position = next < events().length ? events()[next].frame : frames().length - 1;
  render();
});
$('play').addEventListener('click', () => { if (animation) pause(true); else { manual = true; play(); } });
new IntersectionObserver(entries => {
  inView = entries[0].isIntersecting;
  if (!inView) pause(); else maybeAutoplay();
}, { threshold: 0.15 }).observe(demo);
document.addEventListener('visibilitychange', () => { if (document.hidden) pause(); else maybeAutoplay(); });
reducedMotion.addEventListener('change', () => { if (reducedMotion.matches) pause(); else maybeAutoplay(); });
try {
  const response = await fetch(demo.dataset.traceUrl);
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  traces = await response.json();
  for (const key of scenarios) {
    const trace = traces[key];
    if (!trace?.events?.length || !trace?.frames?.length || trace.frames.some(frame => frame.snapshot.schema_version !== 2)) throw new Error('Unsupported trace');
  }
  $('play').disabled = false;
  $('event-position').disabled = false;
  choose(scenario, false);
  maybeAutoplay();
} catch (error) {
  $('event-title').textContent = 'The trace could not load';
  $('event-detail').textContent = 'Read the integration guide below for the same lifecycle and runnable examples.';
  console.error(error);
}
