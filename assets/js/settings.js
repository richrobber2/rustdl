
    (() => {
      const bridge = window.RustDLSettings;
      const form = document.querySelector('#settings-form');
      const folder = document.querySelector('#download-folder');
      const destination = document.querySelector('#destination');
      const screenshots = document.querySelector('#allow-screenshots');
      const privacy = document.querySelector('#inspection-privacy');
      const keepAwake = document.querySelector('#keep-awake');
      const appearance = document.querySelector('#appearance');
      const background = document.querySelector('#background-theme');
      const reduceMotion = document.querySelector('#reduce-motion');
      const spaceEffect = document.querySelector('#space-effect');
      const mobileDownloads = document.querySelector('#mobile-downloads');
      const refresh = document.querySelector('#diagnostics-refresh');
      const save = document.querySelector('#save');
      const reset = document.querySelector('#reset');
      const status = document.querySelector('#status');
      const setStatus = (message, error = false) => { status.textContent = message; status.classList.toggle('error', error); };
      const show = result => {
        folder.value = result.downloadFolder || 'RustDL';
        destination.textContent = result.downloadPath || `Downloads/${folder.value}`;
        keepAwake.checked = result.keepScreenAwake !== false;
        screenshots.checked = result.allowScreenshots === true;
        privacy.checked = result.inspectionPrivacy !== false;
        refresh.value = String(result.diagnosticsRefreshSeconds || 5);
        appearance.value = result.appearance || 'system';
        background.value = result.backgroundTheme || 'space';
        reduceMotion.checked = result.reduceMotion === true;
        spaceEffect.checked = result.spaceEffectEnabled !== false;
        mobileDownloads.value = result.mobileDownloadPolicy || 'ask';
        spaceEffect.disabled = background.value === 'rainy-city';
        window.RustDLTheme?.apply(appearance.value, spaceEffect.checked, false, true, background.value, reduceMotion.checked);
      };
      const call = method => {
        try { const result = JSON.parse(method()); show(result); setStatus(result.detail || '', !result.ok); return result.ok; }
        catch (_error) { setStatus('Could not communicate with Android settings', true); return false; }
      };
      folder.addEventListener('input', () => { destination.textContent = `Downloads/${folder.value.trim() || '…'}`; });
      const previewAppearance = () => window.RustDLTheme?.apply(appearance.value, spaceEffect.checked, false, true, background.value, reduceMotion.checked);
      background.addEventListener('change', () => { spaceEffect.disabled = background.value === 'rainy-city'; previewAppearance(); });
      appearance.addEventListener('change', previewAppearance);
      spaceEffect.addEventListener('change', previewAppearance);
      reduceMotion.addEventListener('change', previewAppearance);
      form.addEventListener('submit', event => {
        event.preventDefault();
        if (!bridge) return;
        save.disabled = true;
        call(() => bridge.save(folder.value, keepAwake.checked, Number(refresh.value), appearance.value, spaceEffect.checked, background.value, screenshots.checked, reduceMotion.checked, mobileDownloads.value, privacy.checked));
        save.disabled = false;
      });
      reset.addEventListener('click', () => { if (bridge) call(() => bridge.reset()); });
      if (bridge) call(() => bridge.settings());
      else { save.disabled = reset.disabled = true; folder.value = 'RustDL'; keepAwake.checked = spaceEffect.checked = true; appearance.value = 'system'; setStatus('Browser appearance is controlled by the quick theme button', false); }
    })();
