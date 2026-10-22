const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {JSDOM} = require('jsdom');
const script = require('./read-script.cjs');
const tick = () => new Promise(resolve => setImmediate(resolve));
function page(name) {
  const html = fs.readFileSync(path.join(__dirname, '../../assets/html', name + '.html'), 'utf8')
    .replace(/<style>[\s\S]*?<\/style>/, '').replace('{page_script}', '');
  return new JSDOM(html, {url: 'http://localhost/', runScripts: 'outside-only'});
}

test('activity bridge renders synthetic state, filters it and handles refresh events', async t => {
  const dom = page('activity'), w = dom.window, calls = [];
  t.after(() => w.close());
  const state = {downloads: [{filename: 'synthetic.mp4', phase: 'ready'}], counts: {completed: 1}};
  w.fetch = async url => { calls.push(url); return {ok: true, json: async () => state}; };
  w.RustDLActivity = {status: () => JSON.stringify({update: {detail: 'Synthetic update'}})};
  let interval;
  w.setInterval = (_fn, ms) => { interval = ms; };
  w.eval(script('activity.js'));
  await tick();
  assert.deepEqual(calls, ['/__app/activity.json']);
  assert.equal(w.document.querySelector('#feed strong').textContent, 'synthetic.mp4');
  assert.equal(w.document.querySelector('#feed a').getAttribute('href'), '/watch/synthetic.mp4');
  assert.equal(w.document.querySelector('#updates').textContent, 'Synthetic update');
  assert.equal(interval, 15000);
  w.document.querySelector('[data-filter="issue"]').click();
  assert.equal(w.document.querySelector('#feed .item').classList.contains('hidden'), true);
  w.dispatchEvent(new w.CustomEvent('rustdl:state', {detail: {type: 'activity'}}));
  await tick();
  assert.equal(calls.length, 2);
});

test('diagnostics bridge renders and copies only the supplied synthetic sample', async t => {
  const dom = page('diagnostics'), w = dom.window;
  t.after(() => w.close());
  const sample = {batteryLevel: 70, batteryTemperatureC: 25, memoryAvailableBytes: 1024, timestamp: 12345};
  let copied;
  w.RustDLSettings = {diagnosticsRefreshSeconds: () => 5};
  w.RustDLDiagnostics = {
    diagnostics: () => JSON.stringify({ok: true, data: sample, detail: 'Synthetic sample'}),
    copySnapshot: value => { copied = JSON.parse(value); return true; }
  };
  w.fetch = async () => ({ok: true, json: async () => ({})});
  w.setInterval = () => {};
  w.eval(script('diagnostics.js'));
  await tick();
  assert.equal(w.document.querySelector('#battery').textContent, '70%');
  assert.equal(w.document.querySelector('#diagnostics-grid').getAttribute('aria-busy'), 'false');
  w.document.querySelector('#copy').click();
  assert.deepEqual(copied, sample);
  assert.equal(w.document.querySelector('#copy').textContent, 'Copied');
});

test('watchlist removal posts to the form endpoint and updates the saved count', async t => {
  const dom = new JSDOM('<span id="watch-count">2 saved</span><article class="watch-card"><form class="remove-form" action="/__app/watchlist"><input name="action" value="remove"><button>Remove</button></form></article><article class="watch-card"></article>', {url: 'http://localhost/', runScripts: 'outside-only'});
  const w = dom.window, calls = [];
  t.after(() => w.close());
  w.fetch = async (url, options) => { calls.push({url, method: options.method, action: options.body.get('action')}); return {ok: true}; };
  w.eval(script('watchlist.js'));
  const event = new w.Event('submit', {bubbles: true, cancelable: true});
  w.document.querySelector('form').dispatchEvent(event);
  await tick();
  assert.equal(event.defaultPrevented, true);
  assert.deepEqual(calls, [{url: '/__app/watchlist', method: 'POST', action: 'remove'}]);
  assert.equal(w.document.querySelectorAll('.watch-card').length, 1);
  assert.equal(w.document.querySelector('#watch-count').textContent, '1 saved');
});

test('catalog save retains the action endpoint and changes the next action to remove', async t => {
  const dom = new JSDOM('<form class="stream-save-form" action="/__app/watchlist"><input name="action" value="add"><input name="title" value="Synthetic series"><button>Save</button></form>', {url: 'http://localhost/', runScripts: 'outside-only'});
  const w = dom.window, calls = [];
  t.after(() => w.close());
  w.fetch = async (url, options) => { calls.push({url, action: options.body.get('action'), response: options.body.get('response')}); return {ok: true, json: async () => ({watchlisted: true})}; };
  w.eval(script('streaming-catalog.js'));
  const form = w.document.querySelector('form');
  form.dispatchEvent(new w.Event('submit', {bubbles: true, cancelable: true}));
  await tick();
  assert.deepEqual(calls, [{url: '/__app/watchlist', action: 'add', response: 'json'}]);
  assert.equal(form.elements.action.value, 'remove');
  assert.equal(form.querySelector('button').getAttribute('aria-label'), 'Remove Synthetic series');
  assert.equal(form.querySelector('button').disabled, false);
});
