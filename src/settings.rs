pub(crate) fn render(dev_reload: &str) -> String {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>RustDL settings</title>
  <style>
    :root { color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }
    * { box-sizing: border-box; }
    body { min-height: 100vh; margin: 0; padding: clamp(1rem,4vw,3rem); color: #f7f7f8; background: radial-gradient(circle at 12% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f; }
    main { width: min(100%, 720px); margin: auto; }
    .top { display: flex; align-items: end; justify-content: space-between; gap: 1rem; margin-bottom: 1.3rem; }
    .eyebrow { color: #8fe3d2; font-size: .7rem; font-weight: 850; letter-spacing: .12em; text-transform: uppercase; }
    h1 { margin: .4rem 0; font-size: clamp(2.4rem,8vw,4.4rem); letter-spacing: -.055em; }
    p { margin: 0; color: #9ca3b3; line-height: 1.55; }
    a, button { min-height: 2.7rem; padding: .72rem .9rem; border: 1px solid #ffffff24; border-radius: 11px; color: #dfe5ef; background: #ffffff0a; text-decoration: none; font: 800 .78rem/1.2 system-ui; }
    button { cursor: pointer; }
    button:disabled { opacity: .45; cursor: default; }
    form { display: grid; gap: .85rem; }
    .card { padding: 1.1rem; border: 1px solid #ffffff18; border-radius: 20px; background: linear-gradient(145deg,#151821e8,#0d0f16ee); }
    .card > label:first-child, .label { display: block; margin-bottom: .45rem; color: #dfe5ef; font-size: .82rem; font-weight: 850; }
    .hint { margin-top: .45rem; color: #7f8798; font-size: .74rem; }
    .folder { display: grid; grid-template-columns: auto minmax(0,1fr); align-items: center; overflow: hidden; border: 1px solid #ffffff28; border-radius: 12px; background: #080a10; }
    .folder span { padding: .85rem 0 .85rem .9rem; color: #70dfc9; font: 800 .86rem ui-monospace, monospace; white-space: nowrap; }
    input[type=text], select { width: 100%; min-height: 3rem; padding: .8rem .9rem; border: 1px solid #ffffff28; border-radius: 11px; color: #fff; background: #080a10; font: inherit; outline: none; }
    .folder input { border: 0; border-radius: 0; }
    input:focus, select:focus, .folder:focus-within { border-color: #70dfc9; }
    .switch-row { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
    .switch-copy { min-width: 0; }
    .switch-copy strong { display: block; font-size: .84rem; }
    .switch-copy small { display: block; margin-top: .3rem; color: #7f8798; line-height: 1.4; }
    .switch { position: relative; flex: 0 0 auto; width: 3.2rem; height: 1.8rem; }
    .switch input { position: absolute; opacity: 0; }
    .switch i { position: absolute; inset: 0; border-radius: 99px; background: #343946; transition: background .15s ease; }
    .switch i::after { content: ""; position: absolute; top: .2rem; left: .2rem; width: 1.4rem; height: 1.4rem; border-radius: 50%; background: #fff; transition: transform .15s ease; }
    .switch input:checked + i { background: #39bda4; }
    .switch input:checked + i::after { transform: translateX(1.4rem); }
    .switch input:focus-visible + i { outline: 2px solid #fff; outline-offset: 3px; }
    .destination { margin-top: .7rem; padding: .75rem; overflow-wrap: anywhere; border: 1px solid #70dfc933; border-radius: 11px; color: #8fe3d2; background: #70dfc90d; font: 750 .78rem ui-monospace, monospace; }
    .actions { display: flex; gap: .65rem; align-items: center; }
    .primary { border-color: transparent; color: #07110f; background: #70dfc9; }
    .status { min-height: 1.3rem; color: #8fe3d2; font-size: .78rem; font-weight: 750; }
    .status.error { color: #ffb6a8; }
    @media(max-width:520px) { .top { align-items: stretch; flex-direction: column; } .actions { display: grid; grid-template-columns: 1fr 1fr; } .actions button { width: 100%; } }
    @media(prefers-reduced-motion:reduce) { * { scroll-behavior: auto !important; transition-duration: .01ms !important; } }
  </style>
</head>
<body>
  <main>
    <div class="top"><div><span class="eyebrow">APK-local preferences</span><h1>Settings.</h1><p>Make RustDL fit this phone. Changes are saved inside the app.</p></div><a href="/">← Gallery</a></div>
    <form id="settings-form">
      <section class="card">
        <label for="download-folder">Completed download folder</label>
        <div class="folder"><span>Downloads /&nbsp;</span><input id="download-folder" type="text" maxlength="48" autocomplete="off" spellcheck="false" required></div>
        <div id="destination" class="destination">Downloads/RustDL</div>
        <p class="hint">Applies to newly exported files. Existing exports stay where they are; RustDL’s streamable working cache remains private to the app.</p>
      </section>
      <section class="card switch-row">
        <div class="switch-copy"><strong>Keep screen awake during playback</strong><small>Prevents the display from sleeping only while media is actively playing.</small></div>
        <label class="switch"><input id="keep-awake" type="checkbox"><i aria-hidden="true"></i><span hidden>Keep screen awake</span></label>
      </section>
      <section class="card">
        <label class="label" for="appearance">Appearance</label>
        <select id="appearance"><option value="system">Follow system</option><option value="dark">Dark</option><option value="light">Light</option></select>
        <p class="hint">The quick theme button on every page switches directly between light and dark.</p>
      </section>
      <section class="card switch-row">
        <div class="switch-copy"><strong>Moving space background</strong><small>Uses layered transform-only star fields and stops automatically with reduced motion.</small></div>
        <label class="switch"><input id="space-effect" type="checkbox"><i aria-hidden="true"></i><span hidden>Moving space background</span></label>
      </section>
      <section class="card">
        <label class="label" for="diagnostics-refresh">Diagnostics refresh rate</label>
        <select id="diagnostics-refresh"><option value="3">Every 3 seconds</option><option value="5">Every 5 seconds</option><option value="10">Every 10 seconds</option><option value="30">Every 30 seconds</option></select>
        <p class="hint">A slower interval uses slightly less battery while the Diagnostics page is open.</p>
      </section>
      <div class="actions"><button class="primary" id="save" type="submit">Save settings</button><button id="reset" type="button">Restore defaults</button></div>
      <div id="status" class="status" role="status" aria-live="polite"></div>
    </form>
  </main>
  <script>
    (() => {
      const bridge = window.RustDLSettings;
      const form = document.querySelector('#settings-form');
      const folder = document.querySelector('#download-folder');
      const destination = document.querySelector('#destination');
      const keepAwake = document.querySelector('#keep-awake');
      const appearance = document.querySelector('#appearance');
      const spaceEffect = document.querySelector('#space-effect');
      const refresh = document.querySelector('#diagnostics-refresh');
      const save = document.querySelector('#save');
      const reset = document.querySelector('#reset');
      const status = document.querySelector('#status');
      const setStatus = (message, error = false) => { status.textContent = message; status.classList.toggle('error', error); };
      const show = result => {
        folder.value = result.downloadFolder || 'RustDL';
        destination.textContent = result.downloadPath || `Downloads/${folder.value}`;
        keepAwake.checked = result.keepScreenAwake !== false;
        refresh.value = String(result.diagnosticsRefreshSeconds || 5);
        appearance.value = result.appearance || 'system';
        spaceEffect.checked = result.spaceEffectEnabled !== false;
        window.RustDLTheme?.apply(appearance.value, spaceEffect.checked, false);
      };
      const call = method => {
        try { const result = JSON.parse(method()); show(result); setStatus(result.detail || '', !result.ok); return result.ok; }
        catch (_error) { setStatus('Could not communicate with Android settings', true); return false; }
      };
      folder.addEventListener('input', () => { destination.textContent = `Downloads/${folder.value.trim() || '…'}`; });
      const previewAppearance = () => window.RustDLTheme?.apply(appearance.value, spaceEffect.checked, false);
      appearance.addEventListener('change', previewAppearance);
      spaceEffect.addEventListener('change', previewAppearance);
      form.addEventListener('submit', event => {
        event.preventDefault();
        if (!bridge) return;
        save.disabled = true;
        call(() => bridge.save(folder.value, keepAwake.checked, Number(refresh.value), appearance.value, spaceEffect.checked));
        save.disabled = false;
      });
      reset.addEventListener('click', () => { if (bridge) call(() => bridge.reset()); });
      if (bridge) call(() => bridge.settings());
      else { save.disabled = reset.disabled = true; folder.value = 'RustDL'; keepAwake.checked = spaceEffect.checked = true; appearance.value = 'system'; setStatus('Browser appearance is controlled by the quick theme button', false); }
    })();
  </script>
  <!--DEV_RELOAD-->
</body>
</html>"#
        .replace("<!--DEV_RELOAD-->", dev_reload)
}
