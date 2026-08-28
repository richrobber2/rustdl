mod activity;
mod activity_state;
#[cfg(test)]
mod activity_tests;
mod live_events;
#[cfg(test)]
mod live_events_tests;
mod settings;
#[cfg(test)]
mod settings_tests;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use qrcode::{Color as QrColor, QrCode};
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_RANGE, RANGE};
use rustypipe::{
    client::{ClientType as YouTubeClientType, RustyPipe},
    model::{
        AudioCodec as YouTubeAudioCodec, AudioFormat as YouTubeAudioFormat,
        VideoCodec as YouTubeVideoCodec, VideoFormat as YouTubeVideoFormat,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::env;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{
    Condvar, Mutex, Once, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const USER_AGENT: &str = "rustdl/0.1";
const DEFAULT_BIND: &str = "127.0.0.1:8080";
const DOWNLOAD_FOLDER_NAME: &str = "RustDL";
const DEV_TOKEN_ENV: &str = "RUSTDL_DEV_TOKEN";
const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>rustdl — video downloader</title>
  <style>
    @view-transition { navigation: auto; }
    @keyframes page-out { to { opacity: 0; transform: scale(.985); } }
    @keyframes page-in { from { opacity: 0; transform: scale(1.015); } }
    ::view-transition-old(root) { animation: 160ms ease-out both page-out; }
    ::view-transition-new(root) { animation: 280ms cubic-bezier(.2,.8,.2,1) both page-in; }
    ::view-transition-group(*) { animation-duration: 380ms; animation-timing-function: cubic-bezier(.2,.8,.2,1); }
    :root { color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }
    * { box-sizing: border-box; }
    body {
      min-height: 100vh; margin: 0; display: grid; place-items: center;
      color: #f7f7f8; background:
        radial-gradient(circle at 20% 10%, #273166 0, transparent 34rem),
        radial-gradient(circle at 90% 85%, #173e3a 0, transparent 30rem), #090a0f;
    }
    main {
      width: min(92vw, 720px); padding: clamp(1.5rem, 5vw, 3.5rem);
      border: 1px solid #ffffff24; border-radius: 28px;
      background: #12141dcc; box-shadow: 0 30px 80px #0008;
      backdrop-filter: blur(18px);
    }
    .badge { color: #8fe3d2; font-size: .78rem; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
    h1 { margin: .7rem 0 .8rem; font-size: clamp(2rem, 7vw, 4rem); line-height: .98; letter-spacing: -.055em; }
    p { margin: 0; color: #aeb3c2; line-height: 1.65; }
    form { display: grid; gap: .8rem; margin-top: 2rem; }
    label { font-size: .85rem; font-weight: 700; color: #d9dce5; }
    .controls { display: flex; gap: .65rem; }
    textarea {
      min-width: 0; flex: 1; padding: 1rem 1.05rem; color: #fff;
      border: 1px solid #ffffff2b; border-radius: 14px; outline: none;
      resize: vertical; background: #090a10; font: inherit; line-height: 1.45;
    }
    textarea:focus { border-color: #70dfc9; box-shadow: 0 0 0 4px #70dfc91f; }
    button {
      padding: 1rem 1.25rem; border: 0; border-radius: 14px; cursor: pointer;
      color: #07110f; background: #70dfc9; font: inherit; font-weight: 850;
    }
    button:hover { background: #94ead9; }
    .fine-print { margin-top: 1.1rem; font-size: .8rem; color: #747a8a; }
    .tool-links { display: flex; flex-wrap: wrap; gap: 1rem; margin-top: 1rem; }
    .queue-link { color: #8fe3d2; font-size: .82rem; font-weight: 800; text-decoration: none; }
    .activity-count { display: inline-grid; place-items: center; min-width: 1.25rem; height: 1.25rem; margin-left: .25rem; padding: 0 .3rem; border-radius: 999px; color: #07110f; background: #70dfc9; font-size: .64rem; }
    .library { margin-top: 2rem; padding-top: 1.5rem; border-top: 1px solid #ffffff18; }
    .library-head { display: flex; align-items: end; justify-content: space-between; gap: 1rem; margin-bottom: 1rem; }
    .library h2 { margin: 0; font-size: 1.08rem; }
    .library-count { color: #747a8a; font-size: .72rem; font-weight: 750; text-transform: uppercase; letter-spacing: .1em; }
    .collection-nav { display: flex; align-items: center; gap: .7rem; margin-bottom: 1rem; }
    .collection-nav a { padding: .55rem .7rem; border: 1px solid #ffffff20; border-radius: 10px; color: #8fe3d2; font-size: .74rem; font-weight: 800; text-decoration: none; }
    .gallery-tools { position: sticky; z-index: 6; top: .5rem; display: grid; gap: .65rem; margin-bottom: .9rem; padding: .75rem; border: 1px solid #ffffff20; border-radius: 16px; background: #10131bf2; box-shadow: 0 12px 30px #0007; backdrop-filter: blur(16px); }
    .gallery-search { width: 100%; padding: .8rem .9rem; border: 1px solid #ffffff28; border-radius: 11px; color: #fff; background: #080a10; font: inherit; outline: none; }
    .gallery-search:focus { border-color: #70dfc9; box-shadow: 0 0 0 3px #70dfc91a; }
    .gallery-filters { display: flex; flex-wrap: wrap; gap: .45rem; }
    .gallery-filters button { width: auto; padding: .5rem .68rem; border: 1px solid #ffffff20; border-radius: 999px; color: #b9c0cd; background: #181b24; font-size: .68rem; }
    .gallery-filters button[aria-pressed="true"] { color: #07110f; border-color: #70dfc9; background: #70dfc9; }
    .gallery-filter-status { color: #777f90; font-size: .7rem; }
    .gallery-empty { grid-column: 1/-1; padding: 1.2rem; border: 1px dashed #ffffff24; border-radius: 14px; color: #8f97a8; text-align: center; }
    .gallery { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .85rem; }
    .media-card { min-width: 0; overflow: hidden; border: 1px solid #ffffff16; border-radius: 17px; color: #fff; background: #090a10; text-decoration: none; transition: transform .18s ease, border-color .18s ease; }
    .media-card:hover { transform: translateY(-2px); border-color: #70dfc959; }
    .media-thumb { position: relative; display: grid; place-items: center; aspect-ratio: 16 / 10; overflow: hidden; background: linear-gradient(135deg,#19203b,#07100f); }
    .media-art { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
    .media-thumb::before { content: ""; position: absolute; z-index: 1; inset: 0; opacity: .55; background: radial-gradient(circle at 78% 18%,#70dfc940,transparent 38%), linear-gradient(115deg,transparent 40%,#ffffff08 41%,transparent 42%); }
    .media-card:nth-child(3n+2) .media-thumb { background: linear-gradient(135deg,#271b3c,#0a1013); }
    .media-card:nth-child(3n) .media-thumb { background: linear-gradient(135deg,#123731,#111321); }
    .media-thumb::after { content: "▶"; position: absolute; z-index: 2; display: grid; place-items: center; width: 2.7rem; height: 2.7rem; border-radius: 50%; color: #07110f; background: #70dfc9e8; box-shadow: 0 10px 28px #0008; }
    .media-card.audio .media-thumb::after { content: "♫"; }
    .media-card.downloading .media-thumb::after { content: "↓"; animation: pulse 1.3s ease-in-out infinite; }
    .media-card.collection-folder .media-thumb::after { content: "▦"; font-size: 1.25rem; }
    .collection-folder .media-thumb { box-shadow: inset 0 -8px #70dfc91c, inset 0 -15px #ffffff0a; }
    .media-info { display: grid; gap: .3rem; padding: .8rem .85rem .9rem; }
    .media-title { font-size: .83rem; font-weight: 800; }
    .media-file { overflow: hidden; color: #777e90; font: .65rem/1.4 ui-monospace, monospace; text-overflow: ellipsis; white-space: nowrap; }
    .media-state { color: #8fe3d2; font-size: .64rem; font-weight: 750; text-transform: uppercase; letter-spacing: .08em; }
    .watch-progress { position: absolute; z-index: 3; inset: auto .7rem .6rem; height: 4px; overflow: hidden; border-radius: 999px; background: #ffffff42; }
    .watch-progress i { display: block; height: 100%; border-radius: inherit; background: #70dfc9; }
    .media-card-shell { position: relative; min-width: 0; }
    .media-card-shell > .media-card { display: block; height: 100%; }
    .card-menu-button {
      position: absolute; z-index: 5; top: .55rem; right: .55rem; width: 2.35rem; height: 2.35rem;
      padding: 0; border: 1px solid #ffffff32; border-radius: 50%; color: #fff;
      background: #07090dcc; box-shadow: 0 8px 24px #0007; backdrop-filter: blur(10px);
    }
    .card-menu-button:hover { color: #07110f; background: #70dfc9; }
    .card-popover, .control-popover {
      width: min(19rem, calc(100vw - 1.5rem)); margin: 0; padding: .55rem;
      border: 1px solid #ffffff24; border-radius: 16px; color: #f6f7fa;
      background: #11141df5; box-shadow: 0 24px 70px #000b; backdrop-filter: blur(20px);
    }
    .card-popover::backdrop, .control-popover::backdrop { background: #0004; }
    .card-popover nav { display: grid; gap: .25rem; }
    .card-popover a, .card-popover button {
      width: 100%; padding: .72rem .78rem; border: 0; border-radius: 10px; color: #e8ebf2;
      background: transparent; font: 750 .78rem/1.2 system-ui; text-align: left; text-decoration: none;
    }
    .card-popover a:hover, .card-popover button:hover { color: #07110f; background: #70dfc9; }
    .card-popover .danger { color: #ffaaa5; }
    .queue-mini {
      position: fixed; z-index: 20; right: max(1rem, env(safe-area-inset-right));
      bottom: max(1rem, env(safe-area-inset-bottom)); display: grid; grid-template-columns: auto minmax(8rem, 1fr) auto;
      align-items: center; gap: .7rem; width: min(29rem, calc(100vw - 2rem)); padding: .65rem;
      border: 1px solid #ffffff26; border-radius: 17px; color: #f5f7fa; background: #10131bea;
      box-shadow: 0 22px 60px #000b; backdrop-filter: blur(22px); view-transition-name: queue-mini;
    }
    .queue-mini > a { display: grid; place-items: center; width: 2.55rem; height: 2.55rem; border-radius: 12px; color: #07110f; background: #70dfc9; font-weight: 900; text-decoration: none; }
    .queue-mini-info { min-width: 0; display: grid; gap: .3rem; }
    .queue-mini-info strong { overflow: hidden; font-size: .76rem; text-overflow: ellipsis; white-space: nowrap; }
    .queue-mini-info span { color: #8c94a5; font-size: .67rem; }
    .queue-mini-progress { height: 3px; overflow: hidden; border-radius: 99px; background: #ffffff1b; }
    .queue-mini-progress i { display: block; height: 100%; background: #70dfc9; }
    .queue-mini button { width: auto; padding: .62rem .7rem; border-radius: 10px; font-size: .7rem; }
    @supports (anchor-name: --card-actions) {
      .card-popover { position: fixed; position-area: block-end span-inline-start; position-try-fallbacks: flip-block, flip-inline; }
    }
    @supports (animation-timeline: view()) {
      @keyframes gallery-reveal { from { opacity: .35; transform: translateY(18px) scale(.985); } to { opacity: 1; transform: none; } }
      .media-card-shell, .gallery > .media-card { animation: gallery-reveal both linear; animation-timeline: view(); animation-range: entry 0% entry 45%; }
    }
    @keyframes pulse { 50% { transform: scale(.9); box-shadow: 0 0 0 10px #70dfc914; } }
    .inspection { margin-bottom: 1.2rem; padding: .75rem 1rem; border: 1px solid #70dfc955; border-radius: 12px; color: #8fe3d2; background: #70dfc912; font-size: .82rem; font-weight: 750; }
    .mode-switch { display: inline-flex; align-items: center; gap: .45rem; margin-bottom: 1rem; padding: .58rem .8rem; border: 1px solid #ffffff24; border-radius: 999px; color: #dfe5ef; background: #ffffff0a; font-size: .76rem; font-weight: 750; text-decoration: none; }
    @media (max-width: 560px) { .controls { flex-direction: column; } button { width: 100%; } .gallery { gap: .6rem; } .media-info { padding: .65rem; } }
    @media (prefers-reduced-motion: reduce) { ::view-transition-group(*), ::view-transition-old(root), ::view-transition-new(root) { animation-duration: .01ms; } .media-card-shell, .gallery > .media-card { animation: none; } }
  </style>
</head>
<body>
  <main>
    <!--MODE_SWITCH-->
    <!--INSPECTION_BANNER-->
    <div class="badge">Built with Rust</div>
    <h1>Save a video.</h1>
    <p>Paste X posts, threads, profiles, YouTube videos or playlists, or Snapchat Spotlight links. RustDL finds every video first, then lets you choose what enters the queue.</p>
    <form id="downloader" action="/discover" method="get">
      <label for="source">X, YouTube, or Snapchat links</label>
      <div class="controls">
        <textarea id="source" name="source" rows="3" inputmode="url"
          placeholder="Post, profile, playlist, Short, or Spotlight" required autofocus></textarea>
        <button type="submit">Find videos</button>
      </div>
    </form>
    <div class="tool-links"><a class="queue-link" id="activity-link" href="/activity">Activity <span class="activity-count" id="activity-count" hidden></span> →</a><a class="queue-link" href="/queue">Download queue →</a><a class="queue-link" href="/storage">Storage manager →</a><a class="queue-link" href="/peers">Device transfer →</a><a class="queue-link" href="/diagnostics">Diagnostics →</a><a class="queue-link" href="/settings">Settings →</a><a class="queue-link" href="/changelog">What’s new →</a></div>
    <p class="fine-print">Public posts only. Download media you have permission to save.</p>
    <!--SAVED_VIDEOS-->
  </main>
  <!--PLAYBACK_SCRIPT-->
  <!--VIEW_TRANSITIONS-->
  <!--DEV_RELOAD-->
</body>
</html>"#;

const PLAYER_CSS: &str = r#"
    @view-transition { navigation: auto; }
    @keyframes page-out { to { opacity: 0; transform: scale(.985); } }
    @keyframes page-in { from { opacity: 0; transform: scale(1.015); } }
    ::view-transition-old(root) { animation: 160ms ease-out both page-out; }
    ::view-transition-new(root) { animation: 280ms cubic-bezier(.2,.8,.2,1) both page-in; }
    ::view-transition-group(*) { animation-duration: 380ms; animation-timing-function: cubic-bezier(.2,.8,.2,1); }
    :root { color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }
    * { box-sizing: border-box; }
    html { background: #07080c; }
    body {
      min-height: 100vh; margin: 0; padding: clamp(1rem, 4vw, 3rem);
      display: grid; place-items: center; overflow-x: hidden; color: #f7f7fb;
      background:
        radial-gradient(circle at 12% 0%, #313d7a 0, transparent 34rem),
        radial-gradient(circle at 100% 100%, #0d4f48 0, transparent 30rem), #07080c;
    }
    body::before {
      content: ""; position: fixed; inset: 0; pointer-events: none; opacity: .18;
      background-image: linear-gradient(#ffffff08 1px, transparent 1px),
        linear-gradient(90deg, #ffffff08 1px, transparent 1px);
      background-size: 48px 48px; mask-image: linear-gradient(to bottom, #000, transparent 72%);
    }
    .player-shell {
      position: relative; width: min(94vw, 940px); padding: clamp(1.15rem, 4vw, 2.5rem);
      border: 1px solid #ffffff20; border-radius: 32px; overflow: hidden;
      background: linear-gradient(145deg, #171a24ed, #0e1018f5);
      box-shadow: 0 32px 100px #000a, inset 0 1px #ffffff0d;
      backdrop-filter: blur(22px);
    }
    .player-shell::before {
      content: ""; position: absolute; width: 22rem; height: 22rem; right: -12rem; top: -15rem;
      border-radius: 50%; background: #70dfc91c; filter: blur(25px); pointer-events: none;
    }
    .topline, .player-toolbar, .meta-card { display: flex; align-items: center; }
    .topline { position: relative; justify-content: space-between; gap: 1rem; margin-bottom: 2rem; }
    .brand { display: inline-flex; align-items: center; gap: .65rem; color: #fff; font-weight: 850; text-decoration: none; }
    .brand-mark {
      display: grid; place-items: center; width: 2rem; height: 2rem; border-radius: 10px;
      color: #07110f; background: #70dfc9; box-shadow: 0 8px 25px #70dfc936;
    }
    .context-pill {
      display: inline-flex; align-items: center; gap: .5rem; padding: .5rem .7rem;
      border: 1px solid #ffffff1c; border-radius: 999px; color: #bdc2d0;
      background: #ffffff08; font-size: .72rem; font-weight: 750;
    }
    .context-pill i { width: .45rem; height: .45rem; border-radius: 50%; background: #70dfc9; box-shadow: 0 0 0 4px #70dfc918; }
    header { position: relative; margin-bottom: 1.4rem; }
    .eyebrow { color: #89ead7; font-size: .72rem; font-weight: 850; letter-spacing: .16em; text-transform: uppercase; }
    h1 { margin: .55rem 0 .65rem; font-size: clamp(2.25rem, 7vw, 4.4rem); line-height: .98; letter-spacing: -.052em; }
    .copy { max-width: 43rem; margin: 0; color: #adb3c3; font-size: clamp(.95rem, 2vw, 1.08rem); line-height: 1.6; }
    .player-frame {
      position: relative; padding: .5rem; border: 1px solid #ffffff1f; border-radius: 24px;
      background: linear-gradient(135deg, #272c3a, #11131b 48%, #17312d);
      box-shadow: 0 24px 60px #0008, inset 0 1px #ffffff18;
    }
    .player-toolbar { justify-content: space-between; gap: .75rem; padding: .35rem .45rem .8rem; color: #c9ceda; font-size: .72rem; font-weight: 750; letter-spacing: .04em; }
    .player-actions { display: flex; align-items: center; justify-content: flex-end; gap: .4rem; }
    .player-control { min-width: 3.1rem; padding: .38rem .55rem; border: 1px solid #ffffff1d; border-radius: 9px; color: #dfe4ec; background: #ffffff08; font: 750 .67rem/1 system-ui, sans-serif; cursor: pointer; }
    .player-control[aria-pressed="true"] { color: #07110f; border-color: #70dfc9; background: #70dfc9; }
    .player-control:disabled { opacity: .38; cursor: default; }
    .codec { padding: .24rem .48rem; border-radius: 7px; color: #8fe3d2; background: #70dfc914; font-size: .62rem; letter-spacing: .1em; }
    video {
      display: block; width: 100%; max-height: 68vh; aspect-ratio: 16 / 9; object-fit: contain;
      border: 1px solid #ffffff12; border-radius: 17px; background: #020305;
      accent-color: #70dfc9;
    }
    audio.audio-player {
      display: block; width: 100%; height: 15rem; padding: 5rem clamp(1rem,8vw,5rem) 1rem;
      border: 1px solid #ffffff12; border-radius: 17px; background:
        radial-gradient(circle at 50% 35%,#70dfc94a,transparent 24%),
        repeating-radial-gradient(circle at 50% 35%,#ffffff12 0 2px,transparent 3px 18px),#05070a;
      accent-color: #70dfc9;
    }
    .seek-toast { position: absolute; z-index: 4; left: 50%; top: 55%; translate: -50% -50%; padding: .55rem .75rem; border-radius: 999px; color: #fff; background: #000c; font-size: .8rem; font-weight: 800; opacity: 0; pointer-events: none; transition: opacity .16s ease; }
    .seek-toast.visible { opacity: 1; }
    .synthetic-video {
      position: relative; width: 100%; aspect-ratio: 16 / 9; overflow: hidden;
      border: 1px solid #ffffff12; border-radius: 17px; background: #020305 url('/__inspect/poster.svg') center / cover no-repeat;
    }
    .play-button {
      position: absolute; left: 50%; top: 45%; width: 4.8rem; height: 4.8rem;
      transform: translate(-50%, -50%); border: 1px solid #ffffff38; border-radius: 50%;
      background: #70dfc9e8; box-shadow: 0 14px 40px #0009, 0 0 0 10px #ffffff0c;
    }
    .play-button::after {
      content: ""; position: absolute; left: 50%; top: 50%; transform: translate(-40%, -50%);
      border-top: .72rem solid transparent; border-bottom: .72rem solid transparent;
      border-left: 1.05rem solid #07110f;
    }
    .fake-controls {
      position: absolute; inset: auto 0 0; display: grid; gap: .65rem; padding: 3rem 1.15rem .9rem;
      color: #f8f9fb; background: linear-gradient(transparent, #000e 60%);
    }
    .control-row { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
    .control-actions { display: flex; align-items: center; gap: 1.1rem; color: #d5d8df; font-size: 1.25rem; }
    .time { font-size: .9rem; font-variant-numeric: tabular-nums; }
    .timeline { height: 4px; overflow: hidden; border-radius: 999px; background: #ffffff42; }
    .timeline::before { content: ""; display: block; width: 0; height: 100%; background: #70dfc9; }
    .meta-card {
      justify-content: space-between; gap: 1rem; margin-top: 1rem; padding: 1rem 1.1rem;
      border: 1px solid #ffffff12; border-radius: 18px; background: #080a10a8;
    }
    .file-block { min-width: 0; display: grid; gap: .28rem; }
    .meta-label { color: #757d91; font-size: .66rem; font-weight: 800; letter-spacing: .12em; text-transform: uppercase; }
    code { overflow: hidden; color: #dce1eb; font: 650 .84rem/1.4 ui-monospace, SFMono-Regular, monospace; text-overflow: ellipsis; white-space: nowrap; }
    .action {
      flex: none; display: inline-flex; align-items: center; gap: .45rem; padding: .72rem .9rem;
      border: 1px solid #70dfc942; border-radius: 12px; color: #8fe3d2;
      background: #70dfc90d; font-size: .8rem; font-weight: 800; text-decoration: none;
    }
    .action:hover { background: #70dfc91c; border-color: #70dfc980; }
    @media (max-width: 600px) {
      body { padding: .75rem; place-items: start center; }
      .player-shell { width: 100%; padding: 1.25rem 1rem; border-radius: 26px; }
      .topline { margin-bottom: 1.6rem; }
      h1 { font-size: clamp(2.1rem, 11vw, 3.25rem); }
      .copy { font-size: .94rem; }
      .player-frame { padding: .38rem; border-radius: 20px; }
      .player-toolbar { align-items: flex-start; flex-direction: column; }
      .player-actions { width: 100%; justify-content: flex-start; }
      video { border-radius: 14px; }
      .meta-card { align-items: stretch; flex-direction: column; padding: .9rem; }
      .action { justify-content: center; }
    }
    @media (prefers-reduced-motion: reduce) {
      ::view-transition-group(*), ::view-transition-old(root), ::view-transition-new(root) { animation-duration: .01ms; }
    }
    body.pip { padding: 0; background: #000; }
    body.pip::before, body.pip .topline, body.pip header, body.pip .player-toolbar, body.pip .meta-card { display: none; }
    body.pip .player-shell, body.pip .player-frame { width: 100vw; height: 100vh; max-width: none; margin: 0; padding: 0; border: 0; border-radius: 0; background: #000; box-shadow: none; }
    body.pip video { width: 100%; height: 100%; max-height: none; border: 0; border-radius: 0; }
    .control-island {
      position: relative; z-index: 8; display: grid; margin-top: .55rem;
      grid-template-columns: auto auto minmax(5rem,1fr) repeat(5,auto); align-items: center; gap: .5rem;
      padding: .62rem; border: 1px solid #ffffff2b; border-radius: 16px; color: #fff;
      background: linear-gradient(180deg,#111621,#090c12); box-shadow: inset 0 1px #ffffff0c;
      transition: opacity .2s ease, transform .2s ease;
    }
    .control-button {
      display: grid; place-items: center; min-width: 2.35rem; height: 2.35rem; padding: 0 .55rem;
      border: 1px solid #ffffff1e; border-radius: 11px; color: #eef1f6; background: #ffffff09;
      font: 800 .72rem/1 system-ui; cursor: pointer;
    }
    .control-button:hover, .control-button[aria-pressed="true"] { color: #07110f; border-color: #70dfc9; background: #70dfc9; }
    .control-time { color: #c9cfda; font: 700 .67rem/1 ui-monospace, monospace; white-space: nowrap; }
    .timeline-shell { position: relative; height: 2.4rem; display: grid; align-items: center; touch-action: none; user-select: none; }
    .timeline-track, .timeline-downloaded, .timeline-played { position: absolute; left: 0; right: 0; height: 4px; border-radius: 99px; pointer-events: none; }
    .timeline-track { background: #ffffff24; }
    .timeline-downloaded { right: auto; width: 0; background: #ffffff58; }
    .timeline-played { right: auto; width: 0; background: #70dfc9; }
    .timeline-input { position: relative; z-index: 2; width: 100%; height: 2.4rem; margin: 0; opacity: 0; cursor: pointer; }
    .scrub-anchor { position: absolute; z-index: 3; left: 0; top: 50%; width: 1px; height: 1px; anchor-name: --scrub-point; pointer-events: none; }
    .scrub-preview {
      position: absolute; z-index: 12; left: 0; bottom: calc(100% + .7rem); display: none; width: 8.5rem;
      overflow: hidden; border: 1px solid #ffffff2b; border-radius: 12px; color: #fff; background: #090b10;
      box-shadow: 0 14px 38px #000b; pointer-events: none; translate: -50% 0;
    }
    .scrub-preview.visible { display: grid; }
    .scrub-preview img { width: 100%; aspect-ratio: 16/9; object-fit: cover; background: #020305; }
    .scrub-preview output { padding: .4rem; font: 800 .67rem/1 ui-monospace, monospace; text-align: center; }
    .download-boundary { position: absolute; z-index: 4; top: .25rem; bottom: .25rem; width: 2px; border-radius: 2px; background: #f6c760; box-shadow: 0 0 0 3px #f6c728; opacity: 0; pointer-events: none; }
    .download-label { display: block; margin: .42rem .5rem .12rem; color: #aeb5c4; font: 700 .63rem/1 system-ui; text-align: right; pointer-events: none; }
    .control-popover { inset: auto .75rem max(.75rem, env(safe-area-inset-bottom)) auto; }
    .control-popover h3 { margin: .35rem .45rem .65rem; font-size: .76rem; }
    .control-popover-grid { display: grid; grid-template-columns: repeat(3,1fr); gap: .35rem; }
    .control-popover button { padding: .7rem .45rem; border: 0; border-radius: 10px; color: #e9edf4; background: #ffffff0a; font: 750 .7rem/1.2 system-ui; }
    .control-popover button.active { color: #07110f; background: #70dfc9; }
    .control-detail { display: grid; gap: .25rem; margin-top: .5rem; padding: .65rem; border-radius: 11px; color: #aeb5c4; background: #ffffff07; font-size: .7rem; }
    .control-detail a { width: max-content; margin-top: .25rem; color: #8fe3d2; font-weight: 800; text-decoration: none; }
    @supports (anchor-name: --speed-control) {
      [data-control-speed] { anchor-name: --speed-control; }
      [data-control-more] { anchor-name: --more-control; }
      #speed-popover { position: fixed; position-anchor: --speed-control; position-area: block-start span-inline-end; position-try-fallbacks: flip-block, flip-inline; }
      #more-popover { position: fixed; position-anchor: --more-control; position-area: block-start span-inline-end; position-try-fallbacks: flip-block, flip-inline; }
      .scrub-preview { position: fixed; position-anchor: --scrub-point; left: auto; bottom: anchor(top); justify-self: anchor-center; translate: 0 -.55rem; }
    }
    .player-frame:fullscreen { width: 100vw; height: 100vh; padding: 0; border: 0; border-radius: 0; background: #000; }
    .player-frame:fullscreen .player-toolbar { display: none; }
    .player-frame:fullscreen video { width: 100%; height: 100%; max-height: none; border: 0; border-radius: 0; }
    .player-frame:fullscreen .control-island { position: absolute; left: 1rem; right: 1rem; bottom: 1rem; margin: 0; background: linear-gradient(180deg,#111621e8,#090c12f2); box-shadow: 0 14px 46px #000b; backdrop-filter: blur(18px); }
    .player-frame:fullscreen.controls-idle:not(:focus-within) .control-island { opacity: 0; transform: translateY(9px); pointer-events: none; }
    .player-frame:fullscreen .download-label { position: absolute; z-index: 4; right: .5rem; bottom: 3.85rem; margin: 0; color: #d5dae4; text-shadow: 0 2px 5px #000; }
    body.pip .control-island, body.pip .download-label { display: none; }
    @media (max-width: 700px) {
      .control-island { grid-template-columns: auto minmax(4rem,1fr) repeat(3,auto); gap: .35rem; padding: .45rem; }
      .player-frame:fullscreen .control-island { left: .55rem; right: .55rem; bottom: .55rem; }
      .control-time, [data-control-mute], [data-control-pip] { display: none; }
      .control-button { min-width: 2.25rem; height: 2.25rem; }
      .player-frame:fullscreen .download-label { bottom: 3.2rem; }
    }
"#;

fn hashed_asset_path(name: &str, extension: &str, content: &str) -> String {
    let digest = blake3::hash(content.as_bytes()).to_hex();
    format!("/__app/{name}.{}.{}", &digest[..16], extension)
}

fn playback_script_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| hashed_asset_path("playback", "js", PLAYBACK_SCRIPT))
}

fn view_transition_script_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| hashed_asset_path("view-transitions", "js", VIEW_TRANSITION_SCRIPT))
}

fn index_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let start = INDEX_HTML.find("<style>").expect("index style start") + "<style>".len();
        let end = INDEX_HTML[start..]
            .find("</style>")
            .map(|offset| start + offset)
            .expect("index style end");
        INDEX_HTML[start..end].to_owned()
    })
}

fn index_css_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| hashed_asset_path("index", "css", index_css()))
}

fn player_css_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| hashed_asset_path("player", "css", PLAYER_CSS))
}

fn index_html_template() -> &'static str {
    static HTML: OnceLock<String> = OnceLock::new();
    HTML.get_or_init(|| {
        let start = INDEX_HTML.find("<style>").expect("index style start");
        let end = INDEX_HTML[start..]
            .find("</style>")
            .map(|offset| start + offset + "</style>".len())
            .expect("index style end");
        let mut html = INDEX_HTML.to_owned();
        html.replace_range(
            start..end,
            &format!(r#"<link rel="stylesheet" href="{}">"#, index_css_path()),
        );
        html
    })
}

fn playback_script_tag() -> String {
    format!(
        r#"<script src="{}" defer></script>"#,
        playback_script_path()
    )
}

fn view_transition_script_tag() -> String {
    format!(
        r#"<script src="{}" defer></script>"#,
        view_transition_script_path()
    )
}

const VIEW_TRANSITION_SCRIPT: &str = r#"(()=>{
  'use strict';
  const supported=typeof document.startViewTransition==='function';
  document.documentElement.dataset.viewTransitions=supported?'supported':'fallback';
  if(!supported)return;

  const transitionName=link=>{
    const thumb=link.querySelector('.media-thumb');
    if(!thumb)return null;
    const declared=thumb.dataset.viewTransitionName||thumb.style.viewTransitionName;
    if(declared&&declared!=='none')return declared;
    let filename='';
    try{filename=decodeURIComponent(new URL(link.href,location.href).pathname.slice(7))}catch(_error){return null}
    const stem=filename.replace(/\.(?:mp4|m4a)$/,'');
    return 'video-'+stem.replace(/[^A-Za-z0-9-]/g,'-');
  };
  const selectSharedElement=link=>{
    const selected=link.querySelector('.media-thumb');
    const name=transitionName(link);
    if(!selected||!name)return;
    document.querySelectorAll('.media-thumb').forEach(thumb=>{thumb.style.viewTransitionName='none'});
    selected.style.viewTransitionName=name;
  };
  const mediaLink=target=>target instanceof Element?target.closest('a.media-card[href^="/watch/"]'):null;
  addEventListener('pointerdown',event=>{
    if(event.button!==0)return;
    const link=mediaLink(event.target);if(link)selectSharedElement(link);
  },{passive:true});
  addEventListener('click',event=>{
    if(event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||event.altKey)return;
    const link=mediaLink(event.target);if(link)selectSharedElement(link);
  });
})();"#;

const PLAYBACK_SCRIPT: &str = r#"(()=>{
  'use strict';
  const bridge=window.RustDLPlayback||null;
  const video=document.querySelector('video[data-filename],audio[data-filename]');
  const finite=value=>Number.isFinite(value);
  const clamp=(value,min,max)=>Math.min(max,Math.max(min,value));
  const formatTime=value=>{
    if(!finite(value)||value<0)return '0:00';
    const seconds=Math.floor(value%60).toString().padStart(2,'0');
    const minutes=Math.floor(value/60)%60;
    const hours=Math.floor(value/3600);
    return hours?hours+':'+minutes.toString().padStart(2,'0')+':'+seconds:minutes+':'+seconds;
  };
  const formatBytes=value=>{
    if(!finite(value)||value<=0)return '0 B';
    const units=['B','KB','MB','GB'];let unit=0;
    while(value>=1024&&unit<units.length-1){value/=1024;unit++}
    return value.toFixed(unit?1:0)+' '+units[unit];
  };
  const localKey=name=>'rustdl:playback:'+name;
  const safeParse=value=>{try{return JSON.parse(value)}catch(_error){return null}};
  const readLocal=name=>safeParse(localStorage.getItem(localKey(name)))||{};
  const saveLocal=(name,position,duration)=>localStorage.setItem(localKey(name),JSON.stringify({position,duration,updated:Date.now()}));
  const loadPosition=name=>bridge?Number(bridge.getPosition(name)):Number(readLocal(name).position||0);
  const savePosition=(name,position,duration)=>{
    if(!finite(position)||!finite(duration)||duration<=0)return;
    if(bridge)bridge.savePosition(name,position,duration);else saveLocal(name,position,duration);
  };
  const clearPosition=name=>{if(bridge)bridge.clearPosition(name);else localStorage.removeItem(localKey(name));};
  const loadRate=()=>bridge?Number(bridge.getPlaybackRate()):Number(localStorage.getItem('rustdl:rate')||1);
  const saveRate=rate=>{if(bridge)bridge.savePlaybackRate(rate);else localStorage.setItem('rustdl:rate',String(rate));};
  const fetchState=filename=>fetch('/__app/state.json'+(filename?'?file='+encodeURIComponent(filename):''),{cache:'no-store'}).then(response=>response.ok?response.json():Promise.reject()).catch(()=>null);
  const supportsPopover='showPopover' in HTMLElement.prototype;
  const connectPopover=(button,popover)=>{
    button.setAttribute('popovertarget',popover.id);
    if(supportsPopover)return;
    popover.hidden=true;
    button.addEventListener('click',event=>{event.preventDefault();popover.hidden=!popover.hidden});
  };

  if(video){
    const audioOnly=video instanceof HTMLAudioElement;
    const filename=video.dataset.filename;
    const frame=video.closest('.player-frame');
    const toast=document.querySelector('.seek-toast');
    const rates=[0.5,0.75,1,1.25,1.5,2];
    let locked=false;
    let lastSaved=0;
    let toastTimer=0;
    let idleTimer=0;
    let downloadFraction=1;
    let currentState=null;
    let sleepTimer=0;
    let seekingPointer=null;
    const showToast=message=>{
      if(!toast)return;
      toast.textContent=message;toast.classList.add('visible');clearTimeout(toastTimer);
      toastTimer=setTimeout(()=>toast.classList.remove('visible'),850);
    };
    const showSeek=delta=>showToast((delta>0?'+':'−')+Math.abs(delta)+' seconds');

    const island=document.createElement('div');
    island.className='control-island';island.setAttribute('role','group');island.setAttribute('aria-label',audioOnly?'Audio controls':'Video controls');
    island.innerHTML='<button class="control-button" type="button" data-control-play aria-label="Play">▶</button><span class="control-time" data-control-time>0:00 / 0:00</span><div class="timeline-shell"><span class="timeline-track"></span><span class="timeline-downloaded"></span><span class="timeline-played"></span><span class="download-boundary"></span><span class="scrub-anchor"></span><span class="scrub-preview"><img alt=""><output>0:00</output></span><input class="timeline-input" type="range" min="0" max="1000" value="0" aria-label="Seek video"></div><button class="control-button" type="button" data-control-mute aria-label="Mute">Vol</button><button class="control-button" type="button" data-control-speed aria-label="Playback speed">1×</button><button class="control-button" type="button" data-control-pip aria-label="Picture in picture">PiP</button><button class="control-button" type="button" data-control-more aria-label="More playback controls">•••</button><button class="control-button" type="button" data-control-fullscreen aria-label="Fullscreen">⛶</button>';
    const downloadLabel=document.createElement('span');downloadLabel.className='download-label';downloadLabel.textContent='Saved locally';
    const speedMenu=document.createElement('div');speedMenu.id='speed-popover';speedMenu.className='control-popover';speedMenu.setAttribute('popover','auto');
    speedMenu.innerHTML='<h3>Playback speed</h3><div class="control-popover-grid"></div>';
    const moreMenu=document.createElement('div');moreMenu.id='more-popover';moreMenu.className='control-popover';moreMenu.setAttribute('popover','auto');
    moreMenu.innerHTML='<h3>Playback options</h3><div class="control-popover-grid"><button type="button" data-option-rotation>Lock rotation</button><button type="button" data-sleep="15">Sleep 15m</button><button type="button" data-sleep="30">Sleep 30m</button><button type="button" data-sleep="60">Sleep 60m</button><button type="button" data-sleep="0">Clear timer</button></div><div class="control-detail"><span data-quality>Quality · detecting</span><span data-audio>Audio · default track</span><span data-captions>Captions · none</span><a data-requality hidden>Choose another quality</a></div>';
    frame.append(island,downloadLabel,speedMenu,moreMenu);
    document.querySelector('.player-actions')?.remove();
    video.controls=false;video.dataset.enhanced='true';

    const play=island.querySelector('[data-control-play]');
    const time=island.querySelector('[data-control-time]');
    const timeline=island.querySelector('.timeline-input');
    const played=island.querySelector('.timeline-played');
    const downloaded=island.querySelector('.timeline-downloaded');
    const boundary=island.querySelector('.download-boundary');
    const scrubAnchor=island.querySelector('.scrub-anchor');
    const preview=island.querySelector('.scrub-preview');
    const previewImage=preview.querySelector('img');
    const previewTime=preview.querySelector('output');
    const mute=island.querySelector('[data-control-mute]');
    const speed=island.querySelector('[data-control-speed]');
    const pip=island.querySelector('[data-control-pip]');
    const more=island.querySelector('[data-control-more]');
    const fullscreen=island.querySelector('[data-control-fullscreen]');
    const rotation=moreMenu.querySelector('[data-option-rotation]');
    if(!audioOnly)previewImage.src='/thumbnail/'+encodeURIComponent(filename)+'.jpg';
    connectPopover(speed,speedMenu);connectPopover(more,moreMenu);

    const revealControls=()=>{
      frame.classList.remove('controls-idle');clearTimeout(idleTimer);
      if(!video.paused)idleTimer=setTimeout(()=>frame.classList.add('controls-idle'),2600);
    };
    ['pointerdown','pointermove','focusin'].forEach(type=>frame.addEventListener(type,revealControls,{passive:true}));
    const updateControls=()=>{
      const duration=finite(video.duration)?video.duration:0;
      const percent=duration?clamp(video.currentTime/duration*100,0,100):0;
      if(seekingPointer===null){timeline.value=String(Math.round(percent*10));played.style.width=percent+'%'}
      time.textContent=formatTime(video.currentTime)+' / '+formatTime(duration);
      play.textContent=video.paused?'▶':'❚❚';play.setAttribute('aria-label',video.paused?'Play':'Pause');
    };
    const togglePlayback=()=>video.paused?video.play().catch(()=>{}):video.pause();
    play.addEventListener('click',togglePlayback);
    video.addEventListener('click',togglePlayback);
    mute.addEventListener('click',()=>{video.muted=!video.muted;mute.textContent=video.muted?'Muted':'Vol';mute.setAttribute('aria-pressed',String(video.muted))});
    const previewSeek=()=>{
      const percent=Number(timeline.value)/10;
      scrubAnchor.style.left=percent+'%';preview.style.left=percent+'%';
      played.style.width=percent+'%';
      previewTime.value=formatTime((video.duration||0)*percent/100);preview.classList.add('visible');
    };
    timeline.addEventListener('input',previewSeek);
    const seekFromPointer=event=>{
      const bounds=timeline.getBoundingClientRect();
      const fraction=bounds.width?clamp((event.clientX-bounds.left)/bounds.width,0,1):0;
      timeline.value=String(Math.round(fraction*1000));previewSeek();
    };
    const commitSeek=()=>{
      const requested=Number(timeline.value)/1000;
      if(currentState&&currentState.phase!=='ready'&&requested>downloadFraction){
        video.currentTime=Math.max(0,(video.duration||0)*downloadFraction-.5);showToast('Waiting for download');
      }else video.currentTime=(video.duration||0)*requested;
      preview.classList.remove('visible');updateControls();
    };
    timeline.addEventListener('pointerdown',event=>{
      if(event.pointerType==='mouse'&&event.button!==0)return;
      event.preventDefault();event.stopPropagation();seekingPointer=event.pointerId;
      timeline.setPointerCapture?.(event.pointerId);seekFromPointer(event);
    });
    timeline.addEventListener('pointermove',event=>{
      if(seekingPointer!==event.pointerId)return;
      event.preventDefault();event.stopPropagation();seekFromPointer(event);
    });
    timeline.addEventListener('pointerup',event=>{
      if(seekingPointer!==event.pointerId)return;
      event.preventDefault();event.stopPropagation();seekFromPointer(event);
      timeline.releasePointerCapture?.(event.pointerId);seekingPointer=null;commitSeek();
    });
    timeline.addEventListener('pointercancel',event=>{
      if(seekingPointer!==event.pointerId)return;
      event.preventDefault();event.stopPropagation();seekingPointer=null;
      preview.classList.remove('visible');updateControls();
    });
    timeline.addEventListener('click',event=>event.preventDefault());
    timeline.addEventListener('change',commitSeek);

    const applyRate=rate=>{
      video.playbackRate=rate;speed.textContent=rate+'×';saveRate(rate);
      speedMenu.querySelectorAll('button').forEach(button=>button.classList.toggle('active',Number(button.dataset.rate)===rate));
    };
    const speedGrid=speedMenu.querySelector('.control-popover-grid');
    rates.forEach(rate=>{
      const button=document.createElement('button');button.type='button';button.dataset.rate=String(rate);button.textContent=rate+'×';
      button.addEventListener('click',()=>{applyRate(rate);if(supportsPopover)speedMenu.hidePopover()});speedGrid.append(button);
    });
    rotation.addEventListener('click',()=>{
      locked=!locked;rotation.classList.toggle('active',locked);rotation.textContent=locked?'Rotation locked':'Lock rotation';
      if(bridge)bridge.setRotationLocked(locked);
    });
    moreMenu.querySelectorAll('[data-sleep]').forEach(button=>button.addEventListener('click',()=>{
      clearTimeout(sleepTimer);const minutes=Number(button.dataset.sleep);
      if(minutes){sleepTimer=setTimeout(()=>{video.pause();showToast('Sleep timer finished')},minutes*60000);showToast('Sleep timer · '+minutes+' minutes')}
      else showToast('Sleep timer cleared');
      if(supportsPopover)moreMenu.hidePopover();
    }));
    const nativePip=bridge&&bridge.supportsPictureInPicture();
    const browserPip=document.pictureInPictureEnabled&&video.requestPictureInPicture;
    pip.hidden=audioOnly;pip.disabled=audioOnly||(!nativePip&&!browserPip);
    rotation.hidden=audioOnly;
    pip.addEventListener('click',()=>{
      if(nativePip)bridge.enterPictureInPicture(video.videoWidth||16,video.videoHeight||9);
      else if(browserPip)video.requestPictureInPicture().catch(()=>{});
    });
    fullscreen.addEventListener('click',()=>{
      if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});
      else if(frame.requestFullscreen)frame.requestFullscreen().catch(()=>video.requestFullscreen?.().catch(()=>{}));
      else video.requestFullscreen?.().catch(()=>{});
    });

    const applySaved=()=>{
      let rate=loadRate();if(!rates.includes(rate))rate=1;applyRate(rate);
      const position=loadPosition(filename);
      if(finite(position)&&position>=5&&position<video.duration-5)video.currentTime=position;
      updateControls();
    };
    if(video.readyState>=1)applySaved();else video.addEventListener('loadedmetadata',applySaved,{once:true});
    video.addEventListener('timeupdate',()=>{
      updateControls();
      if(Date.now()-lastSaved>=2000){lastSaved=Date.now();savePosition(filename,video.currentTime,video.duration)}
      if('mediaSession' in navigator&&finite(video.duration)&&video.duration>0)try{navigator.mediaSession.setPositionState({duration:video.duration,playbackRate:video.playbackRate,position:clamp(video.currentTime,0,video.duration)})}catch(_error){}
    });
    video.addEventListener('pause',()=>{updateControls();revealControls();savePosition(filename,video.currentTime,video.duration);if(bridge)bridge.setPlaying(false);if('mediaSession' in navigator)navigator.mediaSession.playbackState='paused'});
    video.addEventListener('play',()=>{updateControls();revealControls();if(bridge)bridge.setPlaying(!audioOnly);if('mediaSession' in navigator)navigator.mediaSession.playbackState='playing'});
    video.addEventListener('ended',()=>{if(bridge){bridge.markWatched(filename);bridge.setPlaying(false)}else{clearPosition(filename);localStorage.setItem('rustdl:watched:'+filename,'1')}});
    video.addEventListener('dblclick',event=>{const delta=event.offsetX<video.clientWidth/2?-10:10;video.currentTime=clamp(video.currentTime+delta,0,video.duration||0);showSeek(delta)});
    addEventListener('keydown',event=>{
      if(event.target instanceof HTMLInputElement||event.target instanceof HTMLButtonElement)return;
      if(event.code==='Space'){event.preventDefault();togglePlayback()}
      if(event.code==='ArrowLeft'){video.currentTime=clamp(video.currentTime-10,0,video.duration||0);showSeek(-10)}
      if(event.code==='ArrowRight'){video.currentTime=clamp(video.currentTime+10,0,video.duration||0);showSeek(10)}
    });

    const updateDownloadState=state=>{
      currentState=state&&state.current||null;
      if(!currentState)return;
      const total=Number(currentState.total||0);const saved=Number(currentState.downloaded||0);
      downloadFraction=total?clamp(saved/total,0,1):currentState.phase==='ready'?1:0;
      downloaded.style.width=(downloadFraction*100)+'%';boundary.style.left=(downloadFraction*100)+'%';
      boundary.style.opacity=currentState.phase==='ready'?'0':'1';
      downloadLabel.textContent=currentState.phase==='ready'?'Saved locally':formatBytes(saved)+(total?' / '+formatBytes(total):'')+' downloaded';
      moreMenu.querySelector('[data-quality]').textContent='Quality · '+(currentState.quality||currentState.height&&currentState.height+'p'||'original');
      const requality=moreMenu.querySelector('[data-requality]');
      if(currentState.source){requality.href='/discover?source='+encodeURIComponent(currentState.source);requality.hidden=false}
    };
    let downloadStatePending=false;
    const refreshDownloadState=()=>{
      if(downloadStatePending)return;downloadStatePending=true;
      fetchState(filename).then(state=>{if(state)updateDownloadState(state)}).finally(()=>downloadStatePending=false);
    };
    addEventListener('rustdl:state',event=>{if(['queue','peer','activity','sync'].includes(event.detail?.type))refreshDownloadState()});
    refreshDownloadState();setInterval(()=>{if(!document.hidden)refreshDownloadState()},15000);

    if('mediaSession' in navigator&&'MediaMetadata' in window){
      const metadata={title:filename,artist:'RustDL',album:audioOnly?'Saved audio':'Saved videos'};
      if(!audioOnly)metadata.artwork=[{src:'/thumbnail/'+encodeURIComponent(filename)+'.jpg',type:'image/jpeg'}];
      navigator.mediaSession.metadata=new MediaMetadata(metadata);
      const handlers={play:()=>video.play(),pause:()=>video.pause(),stop:()=>{video.pause();video.currentTime=0},seekbackward:event=>{video.currentTime=clamp(video.currentTime-(event.seekOffset||10),0,video.duration||0)},seekforward:event=>{video.currentTime=clamp(video.currentTime+(event.seekOffset||10),0,video.duration||0)},seekto:event=>{if(finite(event.seekTime))video.currentTime=clamp(event.seekTime,0,video.duration||0)},previoustrack:()=>{video.currentTime=0}};
      Object.entries(handlers).forEach(([action,handler])=>{try{navigator.mediaSession.setActionHandler(action,handler)}catch(_error){}});
    }
    addEventListener('pagehide',()=>{clearTimeout(sleepTimer);savePosition(filename,video.currentTime,video.duration);if(bridge){bridge.setPlaying(false);bridge.setRotationLocked(false)}});
    updateControls();revealControls();
    return;
  }

  const gallery=document.querySelector('.library');
  if(!gallery)return;
  const available=new Set(Array.from(document.querySelectorAll('.media-card[href^="/watch/"]')).map(card=>decodeURIComponent(card.getAttribute('href').slice(7))));
  let items=[];
  if(bridge)items=safeParse(bridge.getContinueWatching())||[];
  else for(let index=0;index<localStorage.length;index++){
    const key=localStorage.key(index);if(!key||!key.startsWith('rustdl:playback:'))continue;
    const item=readLocal(key.slice(16));if(item.position)items.push({...item,filename:key.slice(16)});
  }
  items=items.filter(item=>available.has(item.filename)&&item.duration>0&&item.position>=5&&item.position<item.duration-5).sort((a,b)=>(b.updated||0)-(a.updated||0)).slice(0,4);
  if(items.length){
    const section=document.createElement('section');section.className='library continue-watching';
    const head=document.createElement('div');head.className='library-head';
    const title=document.createElement('h2');title.textContent='Continue watching';head.append(title);
    const count=document.createElement('span');count.className='library-count';count.textContent=items.length+(items.length===1?' item':' items');head.append(count);section.append(head);
    const cards=document.createElement('div');cards.className='gallery';
    for(const item of items){
      const card=document.createElement('a');card.className='media-card';card.href='/watch/'+encodeURIComponent(item.filename);
      const thumb=document.createElement('div');thumb.className='media-thumb';thumb.dataset.viewTransitionName='video-'+item.filename.replace(/\.(?:mp4|m4a)$/,'').replace(/[^A-Za-z0-9-]/g,'-');
      if(item.filename.endsWith('.m4a'))card.classList.add('audio');else{const image=document.createElement('img');image.className='media-art';image.src='/thumbnail/'+encodeURIComponent(item.filename)+'.jpg';image.alt='';thumb.append(image)}
      const progress=document.createElement('span');progress.className='watch-progress';const bar=document.createElement('i');bar.style.width=Math.min(100,item.position/item.duration*100)+'%';progress.append(bar);thumb.append(progress);card.append(thumb);
      const info=document.createElement('div');info.className='media-info';const state=document.createElement('span');state.className='media-state';state.textContent='Resume';const name=document.createElement('span');name.className='media-title';name.textContent=item.filename;info.append(state,name);card.append(info);cards.append(card);
    }
    section.append(cards);gallery.before(section);
  }

  let stateCache={jobs:[],active:0};
  const enhanceGallery=()=>document.querySelectorAll('.media-card[href^="/watch/"]').forEach((card,index)=>{
    if(card.closest('.media-card-shell'))return;
    const filename=decodeURIComponent(card.getAttribute('href').slice(7));
    const shell=document.createElement('div');shell.className='media-card-shell';card.replaceWith(shell);shell.append(card);
    const button=document.createElement('button');button.type='button';button.className='card-menu-button';button.textContent='•••';button.setAttribute('aria-label','Actions for '+filename);
    const menu=document.createElement('div');menu.className='card-popover';menu.id='card-actions-'+index+'-'+Math.random().toString(36).slice(2);menu.setAttribute('popover','auto');
    const anchor='--card-'+index+'-'+Math.random().toString(36).slice(2);button.style.anchorName=anchor;menu.style.positionAnchor=anchor;
    const nav=document.createElement('nav');
    const play=document.createElement('a');play.href=card.href;play.textContent='Play';nav.append(play);
    const send=document.createElement('a');send.href='/peers/send?file='+encodeURIComponent(filename);send.textContent='Send to device';nav.append(send);
    if(bridge){const share=document.createElement('button');share.type='button';share.textContent=filename.endsWith('.m4a')?'Share audio':'Share video';share.addEventListener('click',()=>bridge.shareVideo(filename));nav.append(share)}
    const watched=document.createElement('button');watched.type='button';watched.textContent='Mark watched';watched.addEventListener('click',()=>{if(bridge)bridge.markWatched(filename);else{clearPosition(filename);localStorage.setItem('rustdl:watched:'+filename,'1')}menu.hidePopover?.()});nav.append(watched);
    const remove=document.createElement('a');remove.href='/storage/confirm?action=delete&file='+encodeURIComponent(filename);remove.className='danger';remove.textContent='Delete…';nav.append(remove);
    menu.append(nav);shell.append(button,menu);connectPopover(button,menu);
  });
  enhanceGallery();

  const galleryTools=document.querySelector('.gallery-tools'),galleryItems=document.getElementById('gallery-items');
  if(galleryTools&&galleryItems){
    const search=galleryTools.querySelector('.gallery-search'),buttons=[...galleryTools.querySelectorAll('[data-gallery-filter]')],status=galleryTools.querySelector('.gallery-filter-status'),empty=document.getElementById('gallery-empty');let filter='all';
    const syncGalleryFilter=()=>{const query=search.value.trim().toLocaleLowerCase();let visible=0,total=0;for(const node of [...galleryItems.children]){if(node===empty)continue;const card=node.matches('.media-card')?node:node.querySelector('.media-card');if(!card)continue;total++;const folder=card.classList.contains('collection-folder'),audio=card.classList.contains('audio'),downloading=card.classList.contains('downloading'),kindMatches=filter==='all'||filter==='playlists'&&folder||filter==='audio'&&audio||filter==='video'&&!folder&&!audio||filter==='downloading'&&downloading;const show=(!query||card.textContent.toLocaleLowerCase().includes(query))&&kindMatches;node.hidden=!show;if(show)visible++}empty.hidden=visible!==0;status.textContent=visible+' of '+total+' shown'};
    search.addEventListener('input',syncGalleryFilter);buttons.forEach(button=>button.addEventListener('click',()=>{filter=button.dataset.galleryFilter;buttons.forEach(item=>item.setAttribute('aria-pressed',String(item===button)));syncGalleryFilter()}));syncGalleryFilter();
  }

  const updateQuickActions=state=>{
    const jobs=new Map((state.jobs||[]).map(job=>[job.filename,job]));
    document.querySelectorAll('.media-card-shell').forEach(shell=>{
      const card=shell.querySelector('.media-card');const filename=decodeURIComponent(card.getAttribute('href').slice(7));const job=jobs.get(filename);const nav=shell.querySelector('.card-popover nav');
      if(!job||!job.source||nav.querySelector('[data-source]'))return;
      const source=document.createElement('a');source.dataset.source='true';source.href=job.source;source.textContent='Open source';
      const quality=document.createElement('a');quality.dataset.source='true';quality.href='/discover?source='+encodeURIComponent(job.source);quality.textContent='Choose another quality';
      nav.insertBefore(source,nav.lastElementChild);nav.insertBefore(quality,nav.lastElementChild);
    });
  };
  const updateActivityBadge=state=>{
    const badge=document.querySelector('#activity-count');if(!badge)return;const count=Number(state.activityActive||0)+Number(state.activityIssues||0);badge.hidden=count<=0;badge.textContent=String(count);
  };
  const updateQueueMini=state=>{
    stateCache=state;const jobs=state.jobs||[];
    const job=jobs.find(item=>['downloading','starting','queued','paused'].includes(item.phase));
    let mini=document.querySelector('.queue-mini');
    if(!job){mini?.remove();return}
    if(!mini){
      mini=document.createElement('aside');mini.className='queue-mini';mini.setAttribute('aria-label','Download queue');
      mini.innerHTML='<a href="/queue" aria-label="Open queue">↓</a><div class="queue-mini-info"><strong></strong><span></span><div class="queue-mini-progress"><i></i></div></div><button type="button"></button>';
      document.body.append(mini);
    }
    mini.querySelector('strong').textContent=job.filename;
    const total=Number(job.total||0),saved=Number(job.downloaded||0),percent=total?clamp(saved/total*100,0,100):0;
    mini.querySelector('.queue-mini-info span').textContent=job.phase+' · '+formatBytes(saved)+(total?' / '+formatBytes(total):'');
    mini.querySelector('.queue-mini-progress i').style.width=percent+'%';
    const action=job.phase==='paused'?'resume':'pause';const button=mini.querySelector('button');button.textContent=action==='pause'?'Pause':'Resume';
    button.onclick=()=>fetch('/queue/action?file='+encodeURIComponent(job.filename)+'&action='+action,{cache:'no-store'}).finally(refreshState);
  };
  let stateRefreshPending=false;
  const refreshState=()=>{
    if(stateRefreshPending)return;stateRefreshPending=true;
    fetchState().then(state=>{if(state){updateQuickActions(state);updateQueueMini(state);updateActivityBadge(state)}}).finally(()=>stateRefreshPending=false);
  };
  addEventListener('rustdl:state',event=>{if(['queue','peer','activity','sync'].includes(event.detail?.type))refreshState()});
  refreshState();setInterval(()=>{if(!document.hidden)refreshState()},15000);
})();"#;

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: u16,
    message: String,
    tweet: Option<Tweet>,
}

#[derive(Debug, Deserialize)]
struct Tweet {
    media: Option<Media>,
}

#[derive(Debug, Deserialize)]
struct Media {
    #[serde(default)]
    videos: Vec<Video>,
}

#[derive(Debug, Deserialize)]
struct Video {
    url: String,
    #[serde(default)]
    variants: Vec<Variant>,
}

#[derive(Debug, Deserialize)]
struct Variant {
    url: String,
    #[serde(default)]
    bitrate: u64,
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct V2ThreadResponse {
    code: u16,
    status: Option<V2Status>,
    thread: Option<Vec<V2Status>>,
}

#[derive(Debug, Deserialize)]
struct V2TimelineResponse {
    code: u16,
    #[serde(default)]
    results: Vec<V2Status>,
}

#[derive(Clone, Debug, Deserialize)]
struct V2Status {
    #[serde(default)]
    id: String,
    #[serde(default)]
    text: String,
    author: Option<V2Author>,
    media: Option<V2Media>,
}

#[derive(Clone, Debug, Deserialize)]
struct V2Author {
    #[serde(default)]
    name: String,
    #[serde(default)]
    screen_name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct V2Media {
    #[serde(default)]
    videos: Vec<V2Video>,
}

#[derive(Clone, Debug, Deserialize)]
struct V2Video {
    url: String,
    #[serde(default)]
    formats: Vec<V2Format>,
}

#[derive(Clone, Debug, Deserialize)]
struct V2Format {
    url: String,
    container: Option<String>,
    #[serde(default)]
    bitrate: u64,
}

struct Args {
    url: String,
    output: Option<PathBuf>,
    force: bool,
}

#[derive(Clone, Debug)]
struct ResolvedVideo {
    filename: String,
    media_url: String,
    audio_url: Option<String>,
    extract_audio: bool,
    quality_label: Option<String>,
    quality_height: Option<u32>,
}

#[derive(Clone, Debug)]
struct DiscoveryCandidate {
    resolved: ResolvedVideo,
    qualities: Vec<ResolvedVideo>,
    source_url: String,
    author: String,
    text: String,
    playlist: Option<PlaylistMembership>,
}

#[derive(Clone, Debug)]
struct DiscoverySession {
    created: u64,
    candidates: Vec<DiscoveryCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct YouTubePlaylistEntry {
    video_id: String,
    title: String,
    author: String,
}

#[derive(Clone, Debug)]
struct PlaylistSession {
    created: u64,
    playlist_id: String,
    title: String,
    entries: Vec<YouTubePlaylistEntry>,
}

#[derive(Clone, Debug)]
struct YouTubePlaylist {
    playlist_id: String,
    title: String,
    entries: Vec<YouTubePlaylistEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PlaylistMembership {
    playlist_id: String,
    title: String,
    position: usize,
    total: usize,
}

#[derive(Clone, Debug)]
struct PeerPairing {
    key: [u8; 32],
    expires: u64,
}

#[derive(Clone, Debug)]
struct OutboundPeerPairing {
    address: String,
    key: [u8; 32],
    expires: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PeerManifest {
    filename: String,
    size: u64,
    hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PeerSendJob {
    phase: String,
    sent: u64,
    total: u64,
    error: Option<String>,
    peer: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct PeerStatus {
    offset: u64,
    complete: bool,
}

#[derive(Clone)]
struct ServeArgs {
    bind: String,
    output_dir: PathBuf,
}

type PublishHook = fn(&Path, &str) -> Result<bool, String>;
type TransferHook = fn(TransferSummary) -> Result<(), String>;
type EventHook = fn(&str) -> Result<(), String>;
type WatchedHook = fn() -> Result<Vec<String>, String>;
type DeleteHook = fn(&str) -> Result<(), String>;
type MuxHook = fn(&Path, &Path, &Path) -> Result<(), String>;
type ExtractAudioHook = fn(&Path, &Path) -> Result<(), String>;
type ThumbnailHook = fn(&Path, &str) -> Result<bool, String>;
static PUBLISH_HOOK: OnceLock<PublishHook> = OnceLock::new();
static TRANSFER_HOOK: OnceLock<TransferHook> = OnceLock::new();
static EVENT_HOOK: OnceLock<EventHook> = OnceLock::new();
static WATCHED_HOOK: OnceLock<WatchedHook> = OnceLock::new();
static DELETE_HOOK: OnceLock<DeleteHook> = OnceLock::new();
static MUX_HOOK: OnceLock<MuxHook> = OnceLock::new();
static EXTRACT_AUDIO_HOOK: OnceLock<ExtractAudioHook> = OnceLock::new();
static THUMBNAIL_HOOK: OnceLock<ThumbnailHook> = OnceLock::new();
static INSPECTION_MODE: OnceLock<bool> = OnceLock::new();
static DOWNLOAD_JOBS: OnceLock<Mutex<HashMap<String, DownloadJob>>> = OnceLock::new();
static QUEUE_OUTPUT_DIR: OnceLock<PathBuf> = OnceLock::new();
static DOWNLOAD_GATE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
static SCHEDULED_WORKERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static LAST_TRANSFER_NOTICE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static EVENT_REVISION: AtomicU64 = AtomicU64::new(0);
static DISCOVERY_SESSIONS: OnceLock<Mutex<HashMap<String, DiscoverySession>>> = OnceLock::new();
static PLAYLIST_SESSIONS: OnceLock<Mutex<HashMap<String, PlaylistSession>>> = OnceLock::new();
static PEER_PAIRING: OnceLock<Mutex<Option<PeerPairing>>> = OnceLock::new();
static OUTBOUND_PEER_PAIRING: OnceLock<Mutex<Option<OutboundPeerPairing>>> = OnceLock::new();
static PEER_SEND_JOBS: OnceLock<Mutex<HashMap<String, PeerSendJob>>> = OnceLock::new();
static PEER_RECEIVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static FINGERPRINT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static RUNTIME_TUNING: OnceLock<Mutex<RuntimeTuning>> = OnceLock::new();
static ACTION_TOKEN: OnceLock<String> = OnceLock::new();
static PEER_SERVER_STARTED: Once = Once::new();
static PEER_PORT: OnceLock<u16> = OnceLock::new();
const MAX_PLAYLIST_ITEMS: usize = 5_000;
const MAX_PLAYLIST_PAGES: usize = 50;
const MAX_PLAYLIST_SELECTIONS: usize = 50;
const PEER_CHUNK_BYTES: usize = 1024 * 1024;
const PEER_PAIRING_SECONDS: u64 = 10 * 60;

#[derive(Clone, Copy, Debug)]
struct RuntimeTuning {
    unmetered: bool,
    charging: bool,
    power_save: bool,
    thermal_status: i32,
    free_bytes: u64,
    processors: usize,
}

impl Default for RuntimeTuning {
    fn default() -> Self {
        Self {
            unmetered: false,
            charging: false,
            power_save: false,
            thermal_status: 0,
            free_bytes: u64::MAX,
            processors: thread::available_parallelism().map_or(2, usize::from),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TransferSummary {
    pub(crate) count: u32,
    pub(crate) downloaded: u64,
    pub(crate) total: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum DownloadPhase {
    Queued,
    Starting,
    Downloading,
    Paused,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DownloadJob {
    phase: DownloadPhase,
    downloaded: u64,
    total: Option<u64>,
    error: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    media_url: Option<String>,
    #[serde(default)]
    audio_url: Option<String>,
    #[serde(default)]
    extract_audio: bool,
    #[serde(default)]
    quality_label: Option<String>,
    #[serde(default)]
    quality_height: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DownloadOutcome {
    Started,
    InProgress,
    Duplicate,
}

fn download_jobs() -> &'static Mutex<HashMap<String, DownloadJob>> {
    DOWNLOAD_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn scheduled_workers() -> &'static Mutex<HashSet<String>> {
    SCHEDULED_WORKERS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn download_gate() -> &'static (Mutex<usize>, Condvar) {
    DOWNLOAD_GATE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

fn runtime_tuning() -> RuntimeTuning {
    *RUNTIME_TUNING
        .get_or_init(|| Mutex::new(RuntimeTuning::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn adaptive_download_limit() -> usize {
    const GIB: u64 = 1024 * 1024 * 1024;
    let tuning = runtime_tuning();
    if tuning.free_bytes < GIB || tuning.power_save || tuning.thermal_status >= 3 {
        1
    } else if tuning.unmetered
        && tuning.thermal_status <= 1
        && tuning.free_bytes >= 3 * GIB
        && tuning.processors >= 6
        && (tuning.charging || tuning.processors >= 8)
    {
        3
    } else {
        2
    }
}

fn adaptive_download_buffer_bytes() -> usize {
    const GIB: u64 = 1024 * 1024 * 1024;
    let tuning = runtime_tuning();
    if tuning.free_bytes < GIB || tuning.power_save || tuning.thermal_status >= 3 {
        64 * 1024
    } else if tuning.unmetered && tuning.thermal_status <= 1 {
        512 * 1024
    } else {
        256 * 1024
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    if raw_args.first().is_some_and(|arg| arg == "serve") {
        return serve(parse_serve_args(&raw_args[1..])?);
    }
    if raw_args.first().is_some_and(|arg| arg == "dev") {
        return dev(parse_serve_args(&raw_args[1..])?);
    }
    download_command(parse_download_args(raw_args.into_iter())?)
}

fn download_command(args: Args) -> Result<(), Box<dyn Error>> {
    let client = build_client()?;
    let resolved = resolve_video(&client, &args.url)?;
    let output = args
        .output
        .unwrap_or_else(|| default_download_dir().join(resolved.filename()));

    if is_complete_download(&output)? && !args.force {
        eprintln!("Duplicate detected; already saved at {}", output.display());
        return Ok(());
    }
    if output.exists() && !args.force {
        fs::remove_file(&output)?;
    }
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    eprintln!("Downloading best MP4 to {}...", output.display());
    download(&client, &resolved.media_url, &output, args.force)?;
    eprintln!("Saved {}", output.display());
    Ok(())
}

fn build_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
}

fn resolve_video(client: &Client, source_url: &str) -> Result<ResolvedVideo, Box<dyn Error>> {
    if youtube_video_id(source_url).is_some() {
        return Ok(resolve_youtube_candidate(source_url)?.resolved);
    }
    if snapchat_spotlight_id(source_url).is_some() {
        return Ok(resolve_snapchat_candidate(client, source_url)?.resolved);
    }
    let status_id = status_id_from_url(source_url)
        .ok_or("expected a supported X, YouTube, or Snapchat Spotlight URL")?;
    let video_number = video_number_from_url(source_url).unwrap_or(1);

    eprintln!("Resolving X post {status_id}...");
    let metadata_url = format!("https://api.fxtwitter.com/status/{status_id}");
    let response = client.get(metadata_url).send()?.error_for_status()?;
    let metadata: ApiResponse = response.json()?;
    if metadata.code != 200 {
        return Err(format!(
            "metadata service returned {}: {}",
            metadata.code, metadata.message
        )
        .into());
    }

    let videos = metadata
        .tweet
        .and_then(|tweet| tweet.media)
        .map(|media| media.videos)
        .ok_or("the post has no downloadable video")?;
    let video = videos.get(video_number - 1).ok_or_else(|| {
        format!(
            "the post contains {} video(s), so video {video_number} does not exist",
            videos.len()
        )
    })?;
    let media_url = best_mp4_url(video).to_owned();

    Ok(ResolvedVideo {
        filename: format!("{status_id}-{video_number}.mp4"),
        media_url,
        audio_url: None,
        extract_audio: false,
        quality_label: Some("Best available".to_owned()),
        quality_height: None,
    })
}

impl ResolvedVideo {
    fn filename(&self) -> String {
        self.filename.clone()
    }
}

fn parse_download_args(mut args: impl Iterator<Item = String>) -> Result<Args, Box<dyn Error>> {
    let mut url = None;
    let mut output = None;
    let mut force = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-o" | "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output needs a file path")?,
                ));
            }
            "-f" | "--force" => force = true,
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}").into()),
            _ if url.is_none() => url = Some(arg),
            _ => return Err("only one video URL may be supplied".into()),
        }
    }

    Ok(Args {
        url: url.ok_or("missing video URL; run with --help for usage")?,
        output,
        force,
    })
}

fn print_help() {
    println!(
        "rustdl - download a public social video\n\n\
         Usage:\n  \
           rustdl [OPTIONS] <VIDEO-URL>\n  \
           rustdl serve [--bind <ADDRESS>] [--output-dir <DIRECTORY>]\n\n\
           rustdl dev [--bind <ADDRESS>] [--output-dir <DIRECTORY>]\n\n\
         Options:\n  \
           -o, --output <FILE>  Override the default Downloads/RustDL output path\n  \
           -f, --force          Overwrite an existing output file\n  \
               --bind <ADDRESS> Web server address (default: 127.0.0.1:8080)\n  \
               --output-dir <DIRECTORY>  Web download folder\n  \
           -h, --help           Show this help"
    );
}

fn parse_serve_args(args: &[String]) -> Result<ServeArgs, Box<dyn Error>> {
    let mut bind = DEFAULT_BIND.to_owned();
    let mut output_dir = default_download_dir();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--bind" => {
                index += 1;
                bind = args.get(index).ok_or("--bind needs an address")?.to_owned();
            }
            "--output-dir" => {
                index += 1;
                output_dir =
                    PathBuf::from(args.get(index).ok_or("--output-dir needs a directory")?);
            }
            option => return Err(format!("unknown serve option: {option}").into()),
        }
        index += 1;
    }
    Ok(ServeArgs { bind, output_dir })
}

fn serve(args: ServeArgs) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&args.output_dir)?;
    let server = Server::http(&args.bind)
        .map_err(|error| format!("could not bind {}: {error}", args.bind))?;
    let client = build_client()?;
    initialize_download_queue(&client, &args.output_dir)?;
    if !inspection_mode() {
        start_peer_server(peer_bind_for(&args.bind)?, args.output_dir.clone());
    }
    eprintln!("rustdl web server listening at http://{}", args.bind);
    eprintln!("Downloads will be saved in {}", args.output_dir.display());
    eprintln!("Press Ctrl+C to stop.");

    for request in server.incoming_requests() {
        let request_client = client.clone();
        let output_dir = args.output_dir.clone();
        thread::spawn(move || {
            if let Err(error) = handle_request(request, &request_client, &output_dir) {
                eprintln!("request error: {error}");
            }
        });
    }
    Ok(())
}

fn peer_bind_for(app_bind: &str) -> Result<String, Box<dyn Error>> {
    let port = app_bind
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        .ok_or("the app bind address has no valid port")?;
    let peer_port = port.checked_add(2).ok_or("the peer port is out of range")?;
    Ok(format!("0.0.0.0:{peer_port}"))
}

fn start_peer_server(bind: String, output_dir: PathBuf) {
    if let Some(port) = bind
        .rsplit_once(':')
        .and_then(|(_, value)| value.parse().ok())
    {
        let _ = PEER_PORT.set(port);
    }
    PEER_SERVER_STARTED.call_once(move || {
        thread::spawn(move || match Server::http(&bind) {
            Ok(server) => {
                eprintln!("RustDL encrypted peer receiver listening on {bind}");
                for request in server.incoming_requests() {
                    let output_dir = output_dir.clone();
                    thread::spawn(move || {
                        if let Err(error) = handle_peer_request(request, &output_dir) {
                            eprintln!("peer request error: {error}");
                        }
                    });
                }
            }
            Err(error) => eprintln!("peer receiver could not bind {bind}: {error}"),
        });
    });
}

fn args_peer_port() -> u16 {
    PEER_PORT.get().copied().unwrap_or(37_660)
}

#[allow(dead_code)]
pub(crate) fn run_embedded_server(bind: String, output_dir: PathBuf) {
    if let Err(error) = serve(ServeArgs { bind, output_dir }) {
        eprintln!("embedded server error: {error}");
    }
}

#[allow(dead_code)]
pub(crate) fn set_publish_hook(hook: PublishHook) {
    let _ = PUBLISH_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_transfer_hook(hook: TransferHook) {
    let _ = TRANSFER_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_event_hook(hook: EventHook) {
    let _ = EVENT_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_watched_hook(hook: WatchedHook) {
    let _ = WATCHED_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_delete_hook(hook: DeleteHook) {
    let _ = DELETE_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_mux_hook(hook: MuxHook) {
    let _ = MUX_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_extract_audio_hook(hook: ExtractAudioHook) {
    let _ = EXTRACT_AUDIO_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_thumbnail_hook(hook: ThumbnailHook) {
    let _ = THUMBNAIL_HOOK.set(hook);
}

#[allow(dead_code)]
pub(crate) fn set_runtime_tuning(
    unmetered: bool,
    charging: bool,
    power_save: bool,
    thermal_status: i32,
    free_bytes: u64,
    processors: usize,
) {
    *RUNTIME_TUNING
        .get_or_init(|| Mutex::new(RuntimeTuning::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = RuntimeTuning {
        unmetered,
        charging,
        power_save,
        thermal_status: thermal_status.max(0),
        free_bytes,
        processors: processors.max(1),
    };
    download_gate().1.notify_all();
}

#[allow(dead_code)]
pub(crate) fn set_inspection_mode(enabled: bool) {
    let _ = INSPECTION_MODE.set(enabled);
}

fn inspection_mode() -> bool {
    INSPECTION_MODE.get().copied().unwrap_or(false)
}

fn dev(args: ServeArgs) -> Result<(), Box<dyn Error>> {
    let executable = env::current_exe()?;
    let mut snapshot = source_snapshot()?;
    let mut generation = 0_u64;
    let mut child = spawn_dev_server(&executable, &args, generation)?;
    eprintln!("Watching src/ and Cargo.toml for changes...");

    loop {
        thread::sleep(Duration::from_millis(500));
        if let Some(status) = child.try_wait()? {
            return Err(format!("development server exited with {status}").into());
        }

        let next_snapshot = source_snapshot()?;
        if next_snapshot == snapshot {
            continue;
        }
        snapshot = next_snapshot;
        eprintln!("Change detected; rebuilding...");
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = Command::new(cargo).arg("build").status()?;
        if !status.success() {
            eprintln!("Build failed; the previous server is still running.");
            continue;
        }

        stop_child(&mut child)?;
        generation += 1;
        child = spawn_dev_server(&executable, &args, generation)?;
        eprintln!("Build succeeded; server restarted and browsers will reload.");
    }
}

fn spawn_dev_server(
    executable: &Path,
    args: &ServeArgs,
    generation: u64,
) -> Result<Child, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let token = format!("{timestamp}-{generation}");
    Ok(Command::new(executable)
        .arg("serve")
        .arg("--bind")
        .arg(&args.bind)
        .arg("--output-dir")
        .arg(&args.output_dir)
        .env(DEV_TOKEN_ENV, token)
        .spawn()?)
}

fn stop_child(child: &mut Child) -> io::Result<()> {
    child.kill()?;
    child.wait()?;
    Ok(())
}

fn source_snapshot() -> io::Result<u64> {
    let mut hasher = DefaultHasher::new();
    hash_watch_path(Path::new("Cargo.toml"), &mut hasher)?;
    hash_watch_path(Path::new("src"), &mut hasher)?;
    Ok(hasher.finish())
}

fn hash_watch_path(path: &Path, hasher: &mut DefaultHasher) -> io::Result<()> {
    path.hash(hasher);
    let metadata = fs::metadata(path)?;
    metadata.len().hash(hasher);
    metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(hasher);
    if metadata.is_dir() {
        let mut children = fs::read_dir(path)?
            .map(|entry| entry.map(|value| value.path()))
            .collect::<io::Result<Vec<_>>>()?;
        children.sort_unstable();
        for child in children {
            hash_watch_path(&child, hasher)?;
        }
    }
    Ok(())
}

fn handle_request(
    request: Request,
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let method = request.method().clone();
    let request_url = request.url().to_owned();
    eprintln!("{method} {request_url}");

    let parsed = Url::parse(&format!("http://localhost{request_url}"))?;
    if method == Method::Post && parsed.path() == "/storage/action" && !inspection_mode() {
        return respond_storage_action(request, output_dir);
    }
    if method == Method::Post && parsed.path() == "/peers/send/start" && !inspection_mode() {
        return respond_peer_send_post(request, client, output_dir);
    }
    if method == Method::Post && parsed.path() == "/peers/send/paired" && !inspection_mode() {
        return respond_peer_send_paired_post(request, client, output_dir);
    }
    if method != Method::Get {
        return respond_text(request, 405, "Method not allowed");
    }
    match parsed.path() {
        "/__app/mode" => respond_text(
            request,
            200,
            if inspection_mode() {
                "inspection"
            } else {
                "normal"
            },
        ),
        "/__dev/version" => respond_dev_version(request),
        "/__app/activity.json" if !inspection_mode() => activity_state::respond_state(request),
        "/__app/state.json" => {
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            respond_app_state(request, output_dir, filename.as_deref())
        }
        path if path == view_transition_script_path() => respond_view_transition_script(request),
        path if path == playback_script_path() && !inspection_mode() => {
            respond_playback_script(request)
        }
        path if path == index_css_path() => {
            respond_immutable_asset(request, index_css(), "text/css; charset=utf-8")
        }
        path if path == player_css_path() => {
            respond_immutable_asset(request, PLAYER_CSS, "text/css; charset=utf-8")
        }
        "/__inspect/result" if inspection_mode() => {
            respond_inspection_page(request, InspectionScreen::Result)
        }
        "/__inspect/player" if inspection_mode() => {
            respond_inspection_page(request, InspectionScreen::Player)
        }
        "/__inspect/poster.svg" if inspection_mode() => respond_inspection_poster(request),
        "/__inspect/capture.png" if inspection_mode() => {
            respond_inspection_capture(request, output_dir)
        }
        path if path.starts_with("/thumbnail/") && !inspection_mode() => {
            respond_thumbnail(request, output_dir, &path[11..])
        }
        "/" => {
            let response = Response::from_string(render_index(output_dir)?)
                .with_status_code(StatusCode(200))
                .with_header(header("Content-Type", "text/html; charset=utf-8"))
                .with_header(html_csp())
                .with_header(header("X-Content-Type-Options", "nosniff"));
            request.respond(response)?;
            Ok(())
        }
        path if path.starts_with("/gallery/playlist/") && !inspection_mode() => {
            let playlist_id = &path[18..];
            if !valid_youtube_playlist_id(playlist_id) {
                return respond_text(request, 404, "Playlist folder not found");
            }
            let response = Response::from_string(render_index_view(output_dir, Some(playlist_id))?)
                .with_status_code(StatusCode(200))
                .with_header(header("Content-Type", "text/html; charset=utf-8"))
                .with_header(html_csp())
                .with_header(header("X-Content-Type-Options", "nosniff"));
            request.respond(response)?;
            Ok(())
        }
        "/activity" if !inspection_mode() => activity_state::respond_page(request),
        "/diagnostics" if !inspection_mode() => respond_diagnostics_page(request),
        "/settings" if !inspection_mode() => respond_settings_page(request),
        "/changelog" => respond_changelog_page(request),
        "/peers/refresh" if !inspection_mode() => {
            respond_peer_pairing_refresh(request, args_peer_port())
        }
        "/peers" if !inspection_mode() => respond_peer_receive_page(request, args_peer_port()),
        "/peers/connected" if !inspection_mode() => {
            respond_peer_connected_page(request, output_dir)
        }
        "/peers/send" if !inspection_mode() => {
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            respond_peer_send_page(request, output_dir, filename.as_deref())
        }
        "/__peer/state" if !inspection_mode() => respond_peer_send_state(request),
        "/download" => {
            if inspection_mode() {
                return respond_inspection_page(request, InspectionScreen::Result);
            }
            let submitted = parsed
                .query_pairs()
                .find(|(key, _)| key == "urls" || key == "url")
                .map(|(_, value)| value.into_owned());
            let Some(submitted) = submitted.filter(|value| !value.trim().is_empty()) else {
                return respond_text(request, 400, "Missing video URL");
            };
            let urls = extract_download_urls(&submitted);
            if urls.is_empty() {
                return respond_text(request, 422, "No supported video URLs were found");
            }
            if urls.len() == 1 {
                return match start_web_download(client, &urls[0], output_dir) {
                    Ok((output, outcome)) => respond_download_result(request, &output, outcome),
                    Err(error) => respond_text(request, 422, &format!("Download failed: {error}")),
                };
            }
            let mut errors = Vec::new();
            for source_url in urls {
                if let Err(error) = start_web_download(client, &source_url, output_dir) {
                    errors.push(format!("{}: {error}", escape_html(&source_url)));
                }
            }
            respond_queue_page(request, &errors)
        }
        "/discover" if !inspection_mode() => {
            let submitted = parsed
                .query_pairs()
                .find(|(key, _)| key == "source" || key == "urls" || key == "url")
                .map(|(_, value)| value.into_owned());
            let Some(submitted) = submitted.filter(|value| !value.trim().is_empty()) else {
                return respond_text(request, 400, "Missing video link");
            };
            let sources = extract_supported_urls(&submitted);
            if let Some(playlist_id) = sources
                .iter()
                .find_map(|source| youtube_playlist_id(source))
            {
                if sources.len() != 1 {
                    return respond_text(
                        request,
                        422,
                        "Open one playlist at a time so its entries can be selected",
                    );
                }
                return match fetch_youtube_playlist_entries(client, &playlist_id) {
                    Ok(playlist) => respond_playlist_selection_page(request, playlist),
                    Err(error) => {
                        respond_text(request, 422, &format!("Playlist discovery failed: {error}"))
                    }
                };
            }
            match discover_videos(client, &submitted) {
                Ok(candidates) => respond_discovery_page(request, candidates),
                Err(error) => respond_text(request, 422, &format!("Discovery failed: {error}")),
            }
        }
        "/playlist/quality" if !inspection_mode() => {
            let picks = parsed
                .query_pairs()
                .filter(|(key, _)| key == "pick")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>();
            respond_playlist_quality_page(request, &picks)
        }
        "/quality" if !inspection_mode() => {
            let picks = parsed
                .query_pairs()
                .filter(|(key, _)| key == "pick")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>();
            respond_quality_page(request, &picks)
        }
        "/import" if !inspection_mode() => {
            let picks = parsed
                .query_pairs()
                .filter(|(key, _)| key == "pick")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>();
            respond_discovery_import(request, client, output_dir, &picks)
        }
        "/queue" if !inspection_mode() => respond_queue_page(request, &[]),
        "/queue/action" if !inspection_mode() => {
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            let action = parsed
                .query_pairs()
                .find(|(key, _)| key == "action")
                .map(|(_, value)| value.into_owned());
            let (Some(filename), Some(action)) = (filename, action) else {
                return respond_text(request, 400, "Missing queue action");
            };
            if let Err(error) = apply_queue_action(client, output_dir, &filename, &action) {
                return respond_text(request, 422, &error);
            }
            respond_queue_page(request, &[])
        }
        "/storage" if !inspection_mode() => respond_storage_page(request, output_dir, None),
        "/storage/confirm" if !inspection_mode() => {
            let action = parsed
                .query_pairs()
                .find(|(key, _)| key == "action")
                .map(|(_, value)| value.into_owned());
            let filename = parsed
                .query_pairs()
                .find(|(key, _)| key == "file")
                .map(|(_, value)| value.into_owned());
            respond_storage_confirmation(
                request,
                output_dir,
                action.as_deref(),
                filename.as_deref(),
            )
        }
        path if path.starts_with("/watch/") => {
            let filename = path.trim_start_matches("/watch/");
            respond_watch_page(request, output_dir, filename)
        }
        path if path.starts_with("/media/") => {
            let filename = path.trim_start_matches("/media/");
            respond_media(request, output_dir, filename)
        }
        path if path.starts_with("/stream/") => {
            let filename = path.trim_start_matches("/stream/");
            respond_growing_media(request, output_dir, filename)
        }
        _ => respond_text(request, 404, "Not found"),
    }
}

fn render_index(output_dir: &Path) -> io::Result<String> {
    render_index_view(output_dir, None)
}

#[derive(Default)]
struct PlaylistGalleryGroup {
    title: String,
    members: Vec<String>,
    ready: Vec<String>,
    downloading: usize,
    total: usize,
}

fn render_index_view(output_dir: &Path, selected_playlist: Option<&str>) -> io::Result<String> {
    let mut filenames = Vec::new();
    if !inspection_mode() && output_dir.is_dir() {
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_file() && valid_video_filename(&filename) {
                filenames.push(filename);
            }
        }
    }
    filenames.sort_unstable_by(|left, right| right.cmp(left));
    let mut active = if inspection_mode() {
        Vec::new()
    } else {
        download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|(_, job)| {
                matches!(
                    job.phase,
                    DownloadPhase::Queued
                        | DownloadPhase::Starting
                        | DownloadPhase::Downloading
                        | DownloadPhase::Paused
                )
            })
            .map(|(filename, _)| filename.clone())
            .filter(|filename| valid_video_filename(filename) && !filenames.contains(filename))
            .collect::<Vec<_>>()
    };
    active.sort_unstable_by(|left, right| right.cmp(left));
    let item_count = active.len() + filenames.len();
    let memberships = if inspection_mode() {
        HashMap::new()
    } else {
        load_playlist_memberships(output_dir)?
    };
    let mut folder_cards = String::new();
    let mut collection_nav = String::new();
    let mut library_title = "Your gallery".to_owned();
    let mut library_summary = format!("{item_count} media items");
    if let Some(playlist_id) = selected_playlist {
        let title = memberships
            .values()
            .find(|membership| membership.playlist_id == playlist_id)
            .map(|membership| membership.title.clone())
            .unwrap_or_else(|| "Playlist folder".to_owned());
        filenames.retain(|filename| {
            memberships
                .get(filename)
                .is_some_and(|membership| membership.playlist_id == playlist_id)
        });
        active.retain(|filename| {
            memberships
                .get(filename)
                .is_some_and(|membership| membership.playlist_id == playlist_id)
        });
        filenames.sort_by_key(|filename| {
            memberships
                .get(filename)
                .map_or(usize::MAX, |membership| membership.position)
        });
        active.sort_by_key(|filename| {
            memberships
                .get(filename)
                .map_or(usize::MAX, |membership| membership.position)
        });
        library_title = title;
        library_summary = format!("{} playlist items", filenames.len() + active.len());
        collection_nav = r#"<div class="collection-nav"><a href="/">← All media</a><span class="media-state">Playlist folder</span></div>"#.to_owned();
    } else if !inspection_mode() {
        let mut groups = HashMap::<String, PlaylistGalleryGroup>::new();
        for filename in filenames.iter().chain(active.iter()) {
            let Some(membership) = memberships.get(filename) else {
                continue;
            };
            let group = groups.entry(membership.playlist_id.clone()).or_default();
            group.title = membership.title.clone();
            group.total = group.total.max(membership.total);
            group.members.push(filename.clone());
            if filenames.contains(filename) {
                group.ready.push(filename.clone());
            } else {
                group.downloading += 1;
            }
        }
        let grouped = groups
            .values()
            .flat_map(|group| group.members.iter().cloned())
            .collect::<HashSet<_>>();
        filenames.retain(|filename| !grouped.contains(filename));
        active.retain(|filename| !grouped.contains(filename));
        let folder_count = groups.len();
        let mut groups = groups.into_iter().collect::<Vec<_>>();
        groups.sort_by(|left, right| {
            left.1
                .title
                .to_lowercase()
                .cmp(&right.1.title.to_lowercase())
        });
        folder_cards = groups
            .into_iter()
            .map(|(playlist_id, mut group)| {
                group.members.sort_by_key(|filename| {
                    memberships.get(filename).map_or(usize::MAX, |value| value.position)
                });
                let art = group
                    .members
                    .iter()
                    .find(|filename| group.ready.contains(filename) && !is_audio_filename(filename))
                    .map_or_else(String::new, |filename| {
                        format!(r#"<img class="media-art" src="/thumbnail/{filename}.jpg" loading="lazy" decoding="async" alt="">"#)
                    });
                let saved = group.ready.len();
                let progress = if group.downloading == 0 {
                    format!("{saved} saved of {}", group.total.max(saved))
                } else {
                    format!("{saved} saved · {} downloading", group.downloading)
                };
                format!(
                    r#"<a class="media-card collection-folder" href="/gallery/playlist/{playlist_id}"><div class="media-thumb">{art}</div><div class="media-info"><span class="media-state">Playlist folder</span><span class="media-title">{}</span><span class="media-file">{progress}</span></div></a>"#,
                    escape_html(&group.title)
                )
            })
            .collect();
        library_summary = if folder_count == 0 {
            format!("{item_count} media items")
        } else {
            format!("{item_count} media · {folder_count} folders")
        };
    }
    let library = if inspection_mode() {
        r#"<section class="library"><div class="library-head"><h2>Synthetic gallery</h2><span class="library-count">2 previews</span></div><div class="gallery"><div class="media-card-shell"><a class="media-card downloading" href="/__inspect/result"><div class="media-thumb" style="view-transition-name:video-synthetic-download"><img class="media-art" src="/__inspect/poster.svg" alt=""></div><div class="media-info"><span class="media-state">Downloading</span><span class="media-title">Progressive stream</span><span class="media-file">synthetic-download.mp4</span></div></a><button class="card-menu-button" type="button" aria-label="Synthetic actions">•••</button></div><div class="media-card-shell"><a class="media-card" href="/__inspect/player"><div class="media-thumb" style="view-transition-name:video-synthetic-preview"><img class="media-art" src="/__inspect/poster.svg" alt=""></div><div class="media-info"><span class="media-state">Ready</span><span class="media-title">Video player</span><span class="media-file">synthetic-preview.mp4</span></div></a><button class="card-menu-button" type="button" aria-label="Synthetic actions">•••</button></div></div></section>"#.to_owned()
    } else if filenames.is_empty() && active.is_empty() && folder_cards.is_empty() {
        String::new()
    } else {
        let downloading_cards = active
            .iter()
            .map(|filename| {
                let transition_name = view_transition_name(filename);
                let kind = if is_audio_filename(filename) { " audio" } else { "" };
                format!(
                    r#"<a class="media-card downloading{kind}" href="/watch/{filename}"><div class="media-thumb" style="view-transition-name:{transition_name}"></div><div class="media-info"><span class="media-state">Downloading</span><span class="media-title">Stream while saving</span><span class="media-file">{}</span></div></a>"#,
                    escape_html(filename)
                )
            })
            .collect::<String>();
        let ready_cards = filenames
            .iter()
            .map(|filename| {
                let transition_name = view_transition_name(filename);
                let (kind, art, title) = if is_audio_filename(filename) {
                    (" audio", String::new(), "Open audio player")
                } else {
                    (
                        "",
                        format!(r#"<img class="media-art" src="/thumbnail/{filename}.jpg" loading="lazy" decoding="async" alt="">"#),
                        "Open player",
                    )
                };
                format!(
                    r#"<a class="media-card{kind}" href="/watch/{filename}"><div class="media-thumb" style="view-transition-name:{transition_name}">{art}</div><div class="media-info"><span class="media-state">Ready</span><span class="media-title">{title}</span><span class="media-file">{}</span></div></a>"#,
                    escape_html(filename)
                )
            })
            .collect::<String>();
        format!(
            r#"<section class="library" id="gallery-library">{collection_nav}<div class="library-head"><h2>{}</h2><span class="library-count">{library_summary}</span></div><div class="gallery-tools"><input class="gallery-search" type="search" placeholder="Search gallery or playlists" aria-label="Search gallery" autocomplete="off"><div class="gallery-filters" aria-label="Gallery filters"><button type="button" data-gallery-filter="all" aria-pressed="true">All</button><button type="button" data-gallery-filter="playlists" aria-pressed="false">Playlists</button><button type="button" data-gallery-filter="video" aria-pressed="false">Video</button><button type="button" data-gallery-filter="audio" aria-pressed="false">Audio</button><button type="button" data-gallery-filter="downloading" aria-pressed="false">Downloading</button></div><span class="gallery-filter-status" aria-live="polite"></span></div><div class="gallery" id="gallery-items">{folder_cards}{downloading_cards}{ready_cards}<p class="gallery-empty" id="gallery-empty" hidden>No gallery items match this search.</p></div></section>"#,
            escape_html(&library_title)
        )
    };
    let banner = if inspection_mode() {
        r#"<div class="inspection">Inspection mode · user data and network downloads are disabled</div>"#
    } else {
        ""
    };
    let mode_switch = if PUBLISH_HOOK.get().is_some() {
        if inspection_mode() {
            r#"<a class="mode-switch" href="rustdl://mode/normal">← Return to my gallery</a>"#
        } else {
            r#"<a class="mode-switch" href="rustdl://mode/inspection">◇ Preview safe UI</a>"#
        }
    } else {
        ""
    };
    let playback_script = playback_script_tag();
    let view_transition_script = view_transition_script_tag();
    let mut html = index_html_template()
        .replace("<!--SAVED_VIDEOS-->", &library)
        .replace("<!--INSPECTION_BANNER-->", banner)
        .replace("<!--MODE_SWITCH-->", mode_switch)
        .replace(
            "<!--PLAYBACK_SCRIPT-->",
            if inspection_mode() {
                ""
            } else {
                &playback_script
            },
        )
        .replace("<!--VIEW_TRANSITIONS-->", &view_transition_script)
        .replace("<!--DEV_RELOAD-->", &dev_reload_script());
    if inspection_mode() {
        html = html.replace(
            r#"<a class="queue-link" id="activity-link" href="/activity">Activity <span class="activity-count" id="activity-count" hidden></span> →</a>"#,
            "",
        );
        html = html.replace(
            r#"<a class="queue-link" href="/peers">Device transfer →</a>"#,
            "",
        );
        html = html.replace(
            r#"<a class="queue-link" href="/settings">Settings →</a>"#,
            "",
        );
        html = html.replace(" required autofocus", " required");
        html = html.replace(
            "</main>",
            r#"</main><aside class="queue-mini" aria-label="Synthetic download queue"><a href="/__inspect/result">↓</a><div class="queue-mini-info"><strong>synthetic-download.mp4</strong><span>downloading · 26.0 MB / 64.0 MB</span><div class="queue-mini-progress"><i style="width:41%"></i></div></div><button type="button">Pause</button></aside>"#,
        );
    }
    Ok(html)
}

const PEER_CSS: &str = r#"
@view-transition{navigation:auto}:root{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*{box-sizing:border-box}body{min-height:100vh;margin:0;padding:clamp(1rem,4vw,3rem);color:#f7f7f8;background:radial-gradient(circle at 15% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f}main{width:min(100%,720px);margin:auto}.top{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin-bottom:1.3rem}.eyebrow{color:#8fe3d2;font-size:.7rem;font-weight:850;letter-spacing:.12em;text-transform:uppercase}h1{margin:.4rem 0;font-size:clamp(2.3rem,8vw,4.2rem);letter-spacing:-.055em}p{margin:0;color:#9ca3b3;line-height:1.55}a{color:#dfe5ef}.back{padding:.65rem .8rem;border:1px solid #ffffff24;border-radius:10px;text-decoration:none;font-size:.76rem;font-weight:800}.panel{display:grid;gap:1rem;padding:1.1rem;border:1px solid #ffffff18;border-radius:18px;background:#11131bde}.pair{display:grid;gap:.4rem}.pair span,label span{color:#7f8797;font-size:.68rem;font-weight:850;letter-spacing:.09em;text-transform:uppercase}.pair code{min-height:3.2rem;overflow-wrap:anywhere;padding:.8rem;border:1px solid #70dfc944;border-radius:12px;color:#8fe3d2;background:#080a10;font-size:.8rem}.qr-wrap{display:grid;place-items:center;gap:.7rem;min-height:344px;padding:1rem;contain:layout paint;border-radius:16px;background:#f7f8fa}.qr-wrap p{color:#28303a;font-size:.78rem;font-weight:800;text-align:center}.pairing-qr{display:block;width:min(100%,290px);aspect-ratio:1;height:auto;contain:layout paint;view-transition-name:pairing-code}.actions{display:flex;flex-wrap:wrap;gap:.6rem}.actions a,.actions button,button{padding:.8rem 1rem;border:0;border-radius:12px;color:#07110f;background:#70dfc9;font:850 .82rem system-ui;text-decoration:none;cursor:pointer}.actions .secondary{color:#dfe5ef;border:1px solid #ffffff24;background:#181b24}.actions [aria-busy="true"]{pointer-events:none}form{display:grid;gap:.9rem}label{display:grid;gap:.4rem}input{width:100%;padding:.85rem .9rem;border:1px solid #ffffff28;border-radius:11px;color:#fff;background:#080a10;font:inherit;outline:none}input:focus{border-color:#70dfc9}.progress{height:.55rem;overflow:hidden;border-radius:999px;background:#ffffff14}.progress i{display:block;width:0;height:100%;background:#70dfc9}.status{color:#aeb6c5;font-size:.84rem}.media-list{display:grid;gap:.65rem}.media-row{display:flex;align-items:center;gap:.75rem;padding:.8rem;border:1px solid #ffffff16;border-radius:13px;background:#090a10}.media-row strong{min-width:0;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:.82rem}.media-row form{margin:0}.empty{padding:1rem;border:1px dashed #ffffff24;border-radius:13px;text-align:center}::view-transition-group(pairing-code){animation-duration:100ms;animation-timing-function:linear}::view-transition-old(pairing-code){animation:70ms linear both pairing-out}::view-transition-new(pairing-code){animation:90ms linear both pairing-in}@keyframes pairing-out{to{opacity:0}}@keyframes pairing-in{from{opacity:0}}@media(max-width:560px){.top{align-items:stretch;flex-direction:column}.actions{display:grid}.media-row{align-items:stretch;flex-direction:column}.media-row strong{width:100%}.media-row form,.media-row button{width:100%}}@media(prefers-reduced-motion:reduce){::view-transition-group(pairing-code),::view-transition-old(pairing-code),::view-transition-new(pairing-code){animation-duration:.01ms}}
:root{view-transition-name:none}
"#;

const PEER_PAIRING_SCRIPT: &str = r#"<script>(()=>{
const copy=document.getElementById('copy-pairing'),refresh=document.getElementById('new-pairing');
copy.addEventListener('click',async event=>{const value=document.getElementById('peer-address').textContent+'\n'+document.getElementById('peer-key').textContent;try{await navigator.clipboard.writeText(value);event.currentTarget.textContent='Copied'}catch(_error){const area=document.createElement('textarea');area.value=value;document.body.append(area);area.select();document.execCommand('copy');area.remove();event.currentTarget.textContent='Copied'}});
refresh.addEventListener('click',async event=>{event.preventDefault();if(refresh.getAttribute('aria-busy')==='true')return;refresh.setAttribute('aria-busy','true');try{const response=await fetch('/peers/refresh',{cache:'no-store'});if(!response.ok)throw new Error('pairing refresh failed');const next=await response.json();if(!next.qr||!next.address||!next.key)throw new Error('pairing response is incomplete');const swap=()=>{document.querySelector('.pairing-qr').outerHTML=next.qr;document.getElementById('peer-address').textContent=next.address;document.getElementById('peer-key').textContent=next.key};if(document.startViewTransition)await document.startViewTransition(swap).finished;else swap()}catch(_error){}finally{refresh.removeAttribute('aria-busy')}});
})();</script>"#;

#[derive(Serialize)]
struct PeerPairingView {
    qr: String,
    address: String,
    key: String,
}

fn generate_peer_pairing_view(peer_port: u16) -> Result<PeerPairingView, Box<dyn Error>> {
    let key = enable_peer_pairing()?;
    let address = format!("{}:{peer_port}", local_ipv4());
    let mut pairing_url = Url::parse("rustdl://pair")?;
    pairing_url
        .query_pairs_mut()
        .append_pair("address", &address)
        .append_pair("key", &key);
    let qr = render_pairing_qr(pairing_url.as_str())?;
    Ok(PeerPairingView { qr, address, key })
}

fn respond_peer_pairing_refresh(request: Request, peer_port: u16) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(serde_json::to_string(&generate_peer_pairing_view(
        peer_port,
    )?)?)
    .with_status_code(StatusCode(200))
    .with_header(header("Content-Type", "application/json; charset=utf-8"))
    .with_header(header("Cache-Control", "no-store"))
    .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_peer_receive_page(request: Request, peer_port: u16) -> Result<(), Box<dyn Error>> {
    let PeerPairingView { qr, address, key } = generate_peer_pairing_view(peer_port)?;
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Receive from RustDL</title><style>{PEER_CSS}</style></head><body><main><div class="top"><div><span class="eyebrow">Encrypted device transfer</span><h1>Scan to pair.</h1><p>Open the other device's camera and scan this code. RustDL opens directly with a searchable send screen.</p></div><a class="back" href="/">← Gallery</a></div><section class="panel"><div class="qr-wrap">{qr}<p>Scan with the other device's camera</p></div><div class="pair"><span>Receiver address</span><code id="peer-address">{address}</code></div><div class="pair"><span>One-time pairing key · manual fallback</span><code id="peer-key">{key}</code></div><div class="actions"><button id="copy-pairing" type="button">Copy pairing details</button><a class="secondary" id="new-pairing" href="/peers">Generate a new code</a></div><p class="status">The code expires after 10 minutes. Media is encrypted before leaving the sender, can resume after an interruption, and appears only after verification.</p></section></main>{PEER_PAIRING_SCRIPT}{}</body></html>"#,
        dev_reload_script()
    );
    respond_html(request, body)
}

fn render_pairing_qr(payload: &str) -> Result<String, Box<dyn Error>> {
    let code = QrCode::new(payload.as_bytes())?;
    let quiet = 4_usize;
    let size = code.width() + quiet * 2;
    let mut path = String::new();
    for y in 0..code.width() {
        for x in 0..code.width() {
            if code[(x, y)] == QrColor::Dark {
                path.push_str(&format!("M{} {}h1v1h-1z", x + quiet, y + quiet));
            }
        }
    }
    Ok(format!(
        r##"<svg class="pairing-qr" viewBox="0 0 {size} {size}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="RustDL pairing QR code" shape-rendering="crispEdges"><rect width="{size}" height="{size}" rx="2" fill="#fff"/><path d="{path}" fill="#05070a"/></svg>"##
    ))
}

fn respond_peer_connected_page(request: Request, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let Some(pairing) = current_outbound_peer_pairing() else {
        let body = format!(
            r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Pair with RustDL</title><style>{PEER_CSS}</style></head><body><main><div class="top"><div><span class="eyebrow">Device transfer</span><h1>Scan a code.</h1><p>On the receiving device, open Device transfer and scan its QR code with this device's camera.</p></div><a class="back" href="/">← Gallery</a></div><section class="panel"><p class="empty">No active device pairing. Pairing codes expire after 10 minutes.</p></section></main>{}</body></html>"#,
            dev_reload_script()
        );
        return respond_html(request, body);
    };
    let mut filenames = Vec::new();
    if output_dir.is_dir() {
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().into_owned();
            if valid_video_filename(&filename) && is_complete_download(&entry.path())? {
                filenames.push(filename);
            }
        }
    }
    filenames.sort_by_key(|filename| filename.to_ascii_lowercase());
    let rows = if filenames.is_empty() {
        r#"<p class="empty">No completed media yet. Finish a download, then return here.</p>"#
            .to_owned()
    } else {
        filenames
            .iter()
            .map(|filename| {
                let filename = escape_html(filename);
                format!(
                    r#"<article class="media-row" data-name="{filename}"><strong>{filename}</strong><form action="/peers/send/paired" method="post"><input type="hidden" name="file" value="{filename}"><button type="submit">Send</button></form></article>"#
                )
            })
            .collect::<String>()
    };
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Send with RustDL</title><style>{PEER_CSS}</style></head><body><main><div class="top"><div><span class="eyebrow">Paired · {}</span><h1>Choose media.</h1><p>Search your completed downloads and send one directly to the paired device.</p></div><a class="back" href="/">← Gallery</a></div><section class="panel"><label><span>Search saved media</span><input id="media-search" type="search" placeholder="Type a filename…" autocomplete="off" autofocus></label><div class="media-list" id="media-list">{rows}</div><p class="status" id="search-count">{} saved items · pairing expires in 10 minutes</p></section></main><script>(()=>{{const input=document.getElementById('media-search'),rows=[...document.querySelectorAll('.media-row')],status=document.getElementById('search-count');input?.addEventListener('input',()=>{{const query=input.value.trim().toLowerCase();let visible=0;for(const row of rows){{const show=!query||row.dataset.name.toLowerCase().includes(query);row.hidden=!show;if(show)visible++}}status.textContent=visible+' matching items · paired with {}'}})}})();</script>{}</body></html>"#,
        escape_html(&pairing.address),
        filenames.len(),
        escape_html(&pairing.address),
        dev_reload_script()
    );
    respond_html(request, body)
}

fn respond_peer_send_page(
    request: Request,
    output_dir: &Path,
    filename: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(filename) = filename.filter(|value| valid_video_filename(value)) else {
        return respond_text(
            request,
            400,
            "Choose Send to device from a saved media item",
        );
    };
    if !is_complete_download(&output_dir.join(filename))? {
        return respond_text(
            request,
            409,
            "Finish downloading this item before sending it",
        );
    }
    let paired = current_outbound_peer_pairing().map_or_else(String::new, |pairing| {
        format!(
            r#"<form action="/peers/send/paired" method="post"><input type="hidden" name="file" value="{}"><button type="submit">Send to paired device · {}</button></form><p class="status">Or use manual pairing below.</p>"#,
            escape_html(filename),
            escape_html(&pairing.address)
        )
    });
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Send to RustDL</title><style>{PEER_CSS}</style></head><body><main><div class="top"><div><span class="eyebrow">Encrypted device transfer</span><h1>Send nearby.</h1><p>Sending <strong>{}</strong> directly over your local network.</p></div><a class="back" href="/">← Gallery</a></div><section class="panel">{paired}<form action="/peers/send/start" method="post"><input type="hidden" name="file" value="{}"><label><span>Receiver address</span><input name="address" inputmode="url" placeholder="192.168.1.20:37660" required></label><label><span>One-time pairing key</span><input name="key" autocapitalize="off" autocomplete="off" spellcheck="false" minlength="64" maxlength="64" required></label><button type="submit">Start encrypted transfer</button></form><p class="status">Both devices must be on the same Wi-Fi or hotspot. The receiver must have its Device transfer page open.</p></section></main>{}</body></html>"#,
        escape_html(filename),
        escape_html(filename),
        dev_reload_script()
    );
    respond_html(request, body)
}

fn respond_peer_send_paired_post(
    mut request: Request,
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    const MAX_PAIRED_FORM_BYTES: u64 = 1024;
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take(MAX_PAIRED_FORM_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PAIRED_FORM_BYTES {
        return respond_text(request, 413, "Send form is too large");
    }
    let form = String::from_utf8(bytes)?;
    let parsed = Url::parse(&format!("http://localhost/?{form}"))?;
    let filename = parsed
        .query_pairs()
        .find(|(name, _)| name == "file")
        .map(|(_, value)| value.into_owned());
    let Some(pairing) = current_outbound_peer_pairing() else {
        return respond_text(request, 410, "Pairing expired; scan a new RustDL QR code");
    };
    let key = hex_encode(&pairing.key);
    start_peer_send(
        request,
        client,
        output_dir,
        filename.as_deref(),
        Some(&pairing.address),
        Some(&key),
    )
}

fn respond_peer_send_post(
    mut request: Request,
    client: &Client,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    const MAX_PAIRING_FORM_BYTES: u64 = 4 * 1024;
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take(MAX_PAIRING_FORM_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PAIRING_FORM_BYTES {
        return respond_text(request, 413, "Pairing form is too large");
    }
    let form = String::from_utf8(bytes)?;
    let parsed = Url::parse(&format!("http://localhost/?{form}"))?;
    let values = parsed
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<HashMap<_, _>>();
    start_peer_send(
        request,
        client,
        output_dir,
        values.get("file").map(String::as_str),
        values.get("address").map(String::as_str),
        values.get("key").map(String::as_str),
    )
}

fn start_peer_send(
    request: Request,
    client: &Client,
    output_dir: &Path,
    filename: Option<&str>,
    address: Option<&str>,
    key: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(filename) = filename.filter(|value| valid_video_filename(value)) else {
        return respond_text(request, 400, "Invalid media filename");
    };
    let path = output_dir.join(filename);
    if !is_complete_download(&path)? {
        return respond_text(
            request,
            409,
            "Finish downloading this item before sending it",
        );
    }
    let Some(address) = address else {
        return respond_text(request, 400, "Missing receiver address");
    };
    let base = match peer_base_url(address) {
        Ok(base) => base,
        Err(error) => return respond_text(request, 400, &error.to_string()),
    };
    let Some(key) = key else {
        return respond_text(request, 400, "Missing pairing key");
    };
    let key = match decode_peer_key(key) {
        Ok(key) => key,
        Err(error) => return respond_text(request, 400, &error.to_string()),
    };
    let total = fs::metadata(&path)?.len();
    set_peer_send_job(
        filename,
        PeerSendJob {
            phase: "hashing".to_owned(),
            sent: 0,
            total,
            error: None,
            peer: address.to_owned(),
        },
    );
    let client = client.clone();
    let filename_owned = filename.to_owned();
    thread::spawn(move || {
        if let Err(error) = send_file_to_peer(&client, &base, &key, &path, &filename_owned) {
            update_peer_send_job(&filename_owned, |job| {
                job.phase = "failed".to_owned();
                job.error = Some(error.to_string());
            });
        }
    });
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Sending with RustDL</title><style>{PEER_CSS}</style></head><body><main><div class="top"><div><span class="eyebrow">Device transfer</span><h1>Sending.</h1><p>{}</p></div><a class="back" href="/">← Gallery</a></div><section class="panel"><div class="progress"><i id="bar"></i></div><p class="status" id="status">Preparing secure transfer…</p><div class="actions"><a class="secondary" href="/">Continue using RustDL</a></div></section></main><script>(()=>{{const file={},bar=document.getElementById('bar'),status=document.getElementById('status');const poll=async()=>{{try{{const response=await fetch('/__peer/state?file='+encodeURIComponent(file),{{cache:'no-store'}}),job=await response.json();const percent=job.total?Math.min(100,job.sent/job.total*100):0;bar.style.width=percent+'%';status.textContent=job.phase==='ready'?'Transfer complete':job.phase==='failed'?'Transfer failed · '+(job.error||'Unknown error'):job.phase+' · '+Math.round(percent)+'%';if(job.phase==='ready'||job.phase==='failed')return}}catch(_error){{}}setTimeout(poll,700)}};poll()}})();</script>{}</body></html>"#,
        escape_html(filename),
        serde_json::to_string(filename)?,
        dev_reload_script()
    );
    respond_html(request, body)
}

fn respond_peer_send_state(request: Request) -> Result<(), Box<dyn Error>> {
    let parsed = Url::parse(&format!("http://localhost{}", request.url()))?;
    let filename = parsed
        .query_pairs()
        .find(|(key, _)| key == "file")
        .map(|(_, value)| value.into_owned())
        .filter(|value| valid_video_filename(value));
    let jobs = PEER_SEND_JOBS.get_or_init(|| Mutex::new(HashMap::new()));
    let jobs = jobs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(job) = filename.as_deref().and_then(|filename| jobs.get(filename)) else {
        return respond_text(request, 404, "Transfer not found");
    };
    let response = Response::from_string(serde_json::to_string(job)?)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_html(request: Request, body: String) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn set_peer_send_job(filename: &str, job: PeerSendJob) {
    PEER_SEND_JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(filename.to_owned(), job);
    notify_simple_event("peer");
}

fn update_peer_send_job(filename: &str, update: impl FnOnce(&mut PeerSendJob)) {
    if let Some(job) = PEER_SEND_JOBS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_mut(filename)
    {
        update(job);
    }
    notify_simple_event("peer");
}

fn enable_peer_pairing() -> Result<String, Box<dyn Error>> {
    let mut key = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut key)?;
    let expires = unix_seconds().saturating_add(PEER_PAIRING_SECONDS);
    *PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(PeerPairing { key, expires });
    Ok(hex_encode(&key))
}

#[allow(dead_code)]
pub(crate) fn set_outbound_peer_pairing(address: &str, key: &str) -> Result<(), String> {
    let address = address.trim();
    peer_base_url(address).map_err(|error| error.to_string())?;
    let key = decode_peer_key(key).map_err(|error| error.to_string())?;
    let pairing = OutboundPeerPairing {
        address: address.to_owned(),
        key,
        expires: unix_seconds().saturating_add(PEER_PAIRING_SECONDS),
    };
    *OUTBOUND_PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(pairing);
    Ok(())
}

fn current_outbound_peer_pairing() -> Option<OutboundPeerPairing> {
    let mut pairing = OUTBOUND_PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if pairing
        .as_ref()
        .is_some_and(|value| value.expires <= unix_seconds())
    {
        *pairing = None;
    }
    pairing.clone()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn local_ipv4() -> Ipv4Addr {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(192, 0, 2, 1), 80))?;
            socket.local_addr()
        })
        .ok()
        .and_then(|address| match address.ip() {
            IpAddr::V4(ip) => Some(ip),
            IpAddr::V6(_) => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

fn peer_base_url(address: &str) -> Result<Url, Box<dyn Error>> {
    let url = Url::parse(&format!("http://{address}/"))?;
    if url.username() != "" || url.password().is_some() || url.port().is_none() {
        return Err("receiver address must be a local IP address and port".into());
    }
    let ip = url
        .host_str()
        .and_then(|host| host.parse::<Ipv4Addr>().ok())
        .filter(|ip| ip.is_private() || ip.is_loopback() || ip.is_link_local())
        .ok_or("receiver must use a private, loopback, or link-local IPv4 address")?;
    let port = url.port().ok_or("receiver port is missing")?;
    Url::parse(&format!("http://{ip}:{port}/")).map_err(Into::into)
}

fn decode_peer_key(value: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if value.len() != 64 {
        return Err("pairing key must contain 64 hexadecimal characters".into());
    }
    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "pairing key is not valid hexadecimal")?;
    }
    Ok(key)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn send_file_to_peer(
    client: &Client,
    base: &Url,
    key: &[u8; 32],
    path: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let total = fs::metadata(path)?.len();
    let hash = blake3_file(path)?;
    let authorization = format!("RustDL {}", hex_encode(key));
    let mut status_url = base.join("v1/status")?;
    append_peer_manifest_query(&mut status_url, filename, total, &hash);
    let status: PeerStatus = client
        .post(status_url)
        .header("Authorization", &authorization)
        .send()?
        .error_for_status()?
        .json()?;
    if status.complete {
        update_peer_send_job(filename, |job| {
            job.phase = "ready".to_owned();
            job.sent = total;
        });
        return Ok(());
    }
    if status.offset > total {
        return Err("receiver reported an invalid resume offset".into());
    }

    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "could not initialize peer encryption")?;
    let mut input = File::open(path)?;
    input.seek(SeekFrom::Start(status.offset))?;
    let mut offset = status.offset;
    let mut buffer = vec![0_u8; PEER_CHUNK_BYTES];
    update_peer_send_job(filename, |job| {
        job.phase = "sending".to_owned();
        job.sent = offset;
    });
    while offset < total {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Err("the source file ended before its declared size".into());
        }
        let mut nonce = [0_u8; 24];
        File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        let aad = peer_chunk_aad(filename, offset, total, &hash);
        let encrypted = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &buffer[..count],
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| "could not encrypt peer chunk")?;
        let mut body = Vec::with_capacity(nonce.len() + encrypted.len());
        body.extend_from_slice(&nonce);
        body.extend_from_slice(&encrypted);
        let mut chunk_url = base.join("v1/chunk")?;
        append_peer_manifest_query(&mut chunk_url, filename, total, &hash);
        chunk_url
            .query_pairs_mut()
            .append_pair("offset", &offset.to_string());
        let next: PeerStatus = client
            .post(chunk_url)
            .header("Authorization", &authorization)
            .header("Content-Type", "application/octet-stream")
            .body(body)
            .send()?
            .error_for_status()?
            .json()?;
        let expected = offset + count as u64;
        if next.offset != expected {
            return Err("receiver did not acknowledge the complete chunk".into());
        }
        offset = next.offset;
        update_peer_send_job(filename, |job| job.sent = offset);
    }
    let mut finish_url = base.join("v1/finish")?;
    append_peer_manifest_query(&mut finish_url, filename, total, &hash);
    let finished: PeerStatus = client
        .post(finish_url)
        .header("Authorization", authorization)
        .send()?
        .error_for_status()?
        .json()?;
    if !finished.complete || finished.offset != total {
        return Err("receiver did not verify the completed file".into());
    }
    update_peer_send_job(filename, |job| {
        job.phase = "ready".to_owned();
        job.sent = total;
    });
    Ok(())
}

fn append_peer_manifest_query(url: &mut Url, filename: &str, size: u64, hash: &str) {
    url.query_pairs_mut()
        .append_pair("file", filename)
        .append_pair("size", &size.to_string())
        .append_pair("hash", hash);
}

fn blake3_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn blake3_hasher_for_existing(path: &Path, bytes: u64) -> Result<blake3::Hasher, String> {
    let mut hasher = blake3::Hasher::new();
    if bytes == 0 {
        return Ok(hasher);
    }
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut remaining = bytes;
    let mut buffer = vec![0_u8; 256 * 1024];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let count = file
            .read(&mut buffer[..limit])
            .map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("resumable download prefix ended before its saved offset".to_owned());
        }
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    Ok(hasher)
}

fn peer_chunk_aad(filename: &str, offset: u64, total: u64, hash: &str) -> String {
    format!("rustdl-v1\n{filename}\n{offset}\n{total}\n{hash}")
}

fn handle_peer_request(mut request: Request, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let key = match authenticate_peer_request(&request) {
        Some(key) => key,
        None => return respond_text(request, 401, "Pairing is missing, invalid, or expired"),
    };
    if request.method() != &Method::Post {
        return respond_text(request, 405, "Method not allowed");
    }
    let parsed = Url::parse(&format!("http://peer{}", request.url()))?;
    let manifest = match peer_manifest_from_url(&parsed) {
        Ok(manifest) => manifest,
        Err(error) => return respond_text(request, 400, &error),
    };
    fs::create_dir_all(output_dir)?;
    let _guard = PEER_RECEIVE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match parsed.path() {
        "/v1/status" => match prepare_peer_receive(output_dir, &manifest) {
            Ok(status) => respond_peer_status(request, status),
            Err(error) => respond_text(request, 422, &error.to_string()),
        },
        "/v1/chunk" => {
            let offset = parsed
                .query_pairs()
                .find(|(name, _)| name == "offset")
                .and_then(|(_, value)| value.parse::<u64>().ok());
            let Some(offset) = offset else {
                return respond_text(request, 400, "Chunk offset is missing");
            };
            match receive_peer_chunk(&mut request, output_dir, &manifest, offset, &key) {
                Ok(()) => respond_peer_status(
                    request,
                    PeerStatus {
                        offset: peer_part_path(output_dir, &manifest.filename)
                            .metadata()?
                            .len(),
                        complete: false,
                    },
                ),
                Err(error) => respond_text(request, 422, &error.to_string()),
            }
        }
        "/v1/finish" => match finish_peer_receive(output_dir, &manifest) {
            Ok(status) => respond_peer_status(request, status),
            Err(error) => respond_text(request, 422, &error.to_string()),
        },
        _ => respond_text(request, 404, "Peer endpoint not found"),
    }
}

fn authenticate_peer_request(request: &Request) -> Option<[u8; 32]> {
    let supplied = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))?
        .value
        .as_str()
        .strip_prefix("RustDL ")
        .and_then(|value| decode_peer_key(value).ok())?;
    let now = unix_seconds();
    let mut pairing = PEER_PAIRING
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let active = pairing.as_mut()?;
    if active.expires < now {
        *pairing = None;
        return None;
    }
    let different = active
        .key
        .iter()
        .zip(supplied.iter())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        });
    if different != 0 {
        return None;
    }
    active.expires = now.saturating_add(PEER_PAIRING_SECONDS);
    Some(active.key)
}

fn peer_manifest_from_url(url: &Url) -> Result<PeerManifest, String> {
    let values = url.query_pairs().collect::<HashMap<_, _>>();
    let filename = values
        .get("file")
        .map(|value| value.to_string())
        .filter(|value| valid_video_filename(value))
        .ok_or_else(|| "Media filename is invalid".to_owned())?;
    let size = values
        .get("size")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|size| *size > 0)
        .ok_or_else(|| "Media size is invalid".to_owned())?;
    let hash = values
        .get("hash")
        .map(|value| value.to_string())
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| "Media digest is invalid".to_owned())?;
    Ok(PeerManifest {
        filename,
        size,
        hash: hash.to_ascii_lowercase(),
    })
}

fn prepare_peer_receive(
    output_dir: &Path,
    manifest: &PeerManifest,
) -> Result<PeerStatus, Box<dyn Error>> {
    let output = output_dir.join(&manifest.filename);
    if output.is_file() {
        if fs::metadata(&output)?.len() == manifest.size && blake3_file(&output)? == manifest.hash {
            if let Some(publish) = PUBLISH_HOOK.get() {
                publish(&output, &manifest.filename)
                    .map_err(|error| format!("could not publish received media: {error}"))?;
            }
            return Ok(PeerStatus {
                offset: manifest.size,
                complete: true,
            });
        }
        return Err("a different local file already uses this media name".into());
    }
    let partial = peer_part_path(output_dir, &manifest.filename);
    let metadata = peer_manifest_path(output_dir, &manifest.filename);
    let existing = fs::read_to_string(&metadata)
        .ok()
        .and_then(|value| serde_json::from_str::<PeerManifest>(&value).ok());
    if existing.as_ref() != Some(manifest) {
        remove_if_exists(&partial)?;
        remove_if_exists(&metadata)?;
        fs::write(&metadata, serde_json::to_vec(manifest)?)?;
    }
    let offset = fs::metadata(partial).map(|value| value.len()).unwrap_or(0);
    if offset > manifest.size {
        return Err("the saved peer partial exceeds the declared media size".into());
    }
    Ok(PeerStatus {
        offset,
        complete: false,
    })
}

fn receive_peer_chunk(
    request: &mut Request,
    output_dir: &Path,
    manifest: &PeerManifest,
    offset: u64,
    key: &[u8; 32],
) -> Result<(), Box<dyn Error>> {
    let partial = peer_part_path(output_dir, &manifest.filename);
    let saved = fs::metadata(&partial).map(|value| value.len()).unwrap_or(0);
    if saved != offset {
        return Err(format!("resume offset mismatch: receiver has {saved} bytes").into());
    }
    let stored_manifest: PeerManifest = serde_json::from_slice(&fs::read(peer_manifest_path(
        output_dir,
        &manifest.filename,
    ))?)?;
    if &stored_manifest != manifest {
        return Err("peer manifest changed during transfer".into());
    }
    let max_encrypted = PEER_CHUNK_BYTES + 24 + 16;
    let mut body = Vec::new();
    request
        .as_reader()
        .take((max_encrypted + 1) as u64)
        .read_to_end(&mut body)?;
    if body.len() <= 24 || body.len() > max_encrypted {
        return Err("encrypted peer chunk has an invalid size".into());
    }
    let (nonce, encrypted) = body.split_at(24);
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| "could not initialize peer decryption")?;
    let aad = peer_chunk_aad(&manifest.filename, offset, manifest.size, &manifest.hash);
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: encrypted,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| "peer chunk authentication failed")?;
    if plaintext.is_empty()
        || plaintext.len() > PEER_CHUNK_BYTES
        || offset + plaintext.len() as u64 > manifest.size
    {
        return Err("decrypted peer chunk has an invalid size".into());
    }
    let mut file = OpenOptions::new().create(true).append(true).open(partial)?;
    file.write_all(&plaintext)?;
    file.flush()?;
    Ok(())
}

fn finish_peer_receive(
    output_dir: &Path,
    manifest: &PeerManifest,
) -> Result<PeerStatus, Box<dyn Error>> {
    let partial = peer_part_path(output_dir, &manifest.filename);
    if fs::metadata(&partial)?.len() != manifest.size || blake3_file(&partial)? != manifest.hash {
        return Err("received media failed its BLAKE3 verification".into());
    }
    let output = output_dir.join(&manifest.filename);
    fs::rename(&partial, &output)?;
    if let Err(error) = record_file_fingerprint(output_dir, &manifest.filename, &manifest.hash) {
        eprintln!("could not cache received media fingerprint: {error}");
    }
    remove_if_exists(&peer_manifest_path(output_dir, &manifest.filename))?;
    if let Some(publish) = PUBLISH_HOOK.get() {
        publish(&output, &manifest.filename)
            .map_err(|error| format!("could not publish received media: {error}"))?;
    }
    set_download_job(
        &manifest.filename,
        DownloadJob {
            phase: DownloadPhase::Ready,
            downloaded: manifest.size,
            total: Some(manifest.size),
            error: None,
            source_url: None,
            media_url: None,
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Received from RustDL".to_owned()),
            quality_height: None,
        },
    );
    persist_download_jobs();
    Ok(PeerStatus {
        offset: manifest.size,
        complete: true,
    })
}

fn peer_part_path(output_dir: &Path, filename: &str) -> PathBuf {
    output_dir.join(format!(".{filename}.peer.part"))
}

fn peer_manifest_path(output_dir: &Path, filename: &str) -> PathBuf {
    output_dir.join(format!(".{filename}.peer.json"))
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn respond_peer_status(request: Request, status: PeerStatus) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(serde_json::to_string(&status)?)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

const CHANGELOG: &[(&str, &[&str])] = &[
    (
        "0.1.31",
        &[
            "Added a live Activity Center for downloads, device transfers, storage pressure, and app updates.",
            "Added active and attention badges plus immediate typed events for transfer and updater state changes.",
            "Added safe activity filters and direct actions without exposing peer addresses, thumbnails, or media contents.",
        ],
    ),
    (
        "0.1.30",
        &[
            "Added a typed Rust-to-Java-to-WebView event bridge for immediate app-state updates.",
            "Made queue progress, phases, errors, and actions update in place without two-second page reloads.",
            "Moved the player and gallery to event-driven refreshes with coalescing and a low-frequency recovery fallback.",
        ],
    ),
    (
        "0.1.29",
        &[
            "Added a persistent Settings page backed entirely by the installed APK.",
            "Added a safe custom Downloads subfolder for newly exported video and audio while preserving existing file locations.",
            "Added playback screen-awake and Diagnostics refresh controls with one-tap default restore.",
        ],
    ),
    (
        "0.1.28",
        &[
            "Rebuilt Diagnostics as a fail-soft dashboard where one unavailable Android source can no longer blank the entire page.",
            "Added explicit source coverage, safe value formatting, useful unavailable states, and clearer refresh feedback.",
            "Kept all telemetry APK-local with no companion process or external installation.",
        ],
    ),
    (
        "0.1.27",
        &[
            "Removed the ADB shell companion, Device control page, privileged receiver, and secure-settings permission.",
            "Moved diagnostics into the bundled APK using Android APIs and app-readable system totals.",
            "RustDL now exposes only features that work directly after installing the APK, with no external bootstrap.",
        ],
    ),
    (
        "0.1.26",
        &[
            "Made the Thermal diagnostics card show a real device-temperature reading instead of an apparently empty normal-status bar.",
            "Moved thermal collection onto lightweight battery sysfs and Android PowerManager sources so diagnostics responds immediately.",
            "Kept Android's throttling status visible beside the temperature.",
        ],
    ),
    (
        "0.1.25",
        &[
            "Added contextual Go to actions that open the screen where each changelog feature lives.",
            "Added precise deep links for downloader, device-control, queue, storage, transfer, diagnostics, gallery, and inspection features.",
            "Kept the sticky version picker for quickly moving through the full release history.",
        ],
    ),
    (
        "0.1.24",
        &[
            "Added sticky Go to version navigation to the in-app changelog.",
            "Every release now has a stable deep-link anchor and selected-release highlight.",
            "Added smooth one-tap jumps with reduced-motion and no-JavaScript anchor fallbacks.",
        ],
    ),
    (
        "0.1.23",
        &[
            "Added a live, privacy-scoped diagnostics dashboard backed by the authenticated shell companion.",
            "Added battery, temperature, thermal, CPU load, memory, storage, uptime, and RustDL process health.",
            "Kept diagnostics out of inspection mode and excluded logs, user media, Wi-Fi identity, and other-app enumeration.",
        ],
    ),
    (
        "0.1.22",
        &[
            "Added real allowlisted controls for system animation speed, global media transport, and music volume.",
            "Added live microphone and camera privacy state with explicit Block and Allow actions.",
            "Added a Motorola FM launcher while keeping protected tuning internals and arbitrary shell commands inaccessible.",
        ],
    ),
    (
        "0.1.21",
        &[
            "Added a token-authenticated ADB shell companion, shell-only bootstrap, and an explicit capability allowlist.",
            "Added an in-app device-control page with live companion and persistent-control status.",
            "Kept device controls and their JavaScript bridge completely out of no-user-data inspection mode.",
        ],
    ),
    (
        "0.1.20",
        &[
            "Replaced the browser's default pointer behavior on the seeker with RustDL-controlled scrubbing.",
            "Added pointer capture for stable mobile drags and prevented seeker gestures from leaking into page interaction.",
            "Kept keyboard seeking accessible while previewing and enforcing progressive-download boundaries.",
        ],
    ),
    (
        "0.1.19",
        &[
            "Moved custom playback controls into a dedicated dock below video and audio so they never obscure media during normal playback.",
            "Kept a compact auto-hiding control overlay only while fullscreen.",
            "Moved download status outside the picture and corrected desktop/mobile control grid sizing.",
        ],
    ),
    (
        "0.1.18",
        &[
            "Gallery thumbnails are now generated visible-first through lazy WebView requests, with two bounded Android decoders and direct scaled-frame extraction.",
            "RustDL now records BLAKE3 fingerprints while media downloads and caches validated fingerprints for fast duplicate storage scans.",
            "Index/player CSS and playback transitions now use content-hashed immutable assets for instant repeat navigation.",
            "Download concurrency and buffers now adapt to the phone's network, charging, power-save, thermal, free-storage, and CPU conditions.",
        ],
    ),
    (
        "0.1.17",
        &[
            "Pairing QR refreshes now use a compact Rust JSON endpoint instead of downloading and parsing the full page.",
            "The replacement SVG is applied directly while the current QR remains visible and the controls stay visually unchanged.",
            "Shortened the QR-only transition to about 100 ms for an effectively instant switch.",
        ],
    ),
    (
        "0.1.16",
        &[
            "Generating a new pairing QR now keeps the current page and code visible while Rust creates the replacement.",
            "Reserved QR and pairing-value geometry prevents layout movement during refresh.",
            "Added an in-place QR-only view transition with reduced-motion support and no changing button label.",
        ],
    ),
    (
        "0.1.15",
        &[
            "Added instant gallery search across media filenames and playlist folder titles.",
            "Added one-tap gallery filters for playlists, video, audio, and active downloads.",
            "Added live visible-result counts and an accessible empty-results state.",
        ],
    ),
    (
        "0.1.14",
        &[
            "Added Download selected as to apply one format choice across an entire playlist queue batch.",
            "Bulk presets cover best MP4, MP4 up to 1080p, 720p, or 480p, and audio-only M4A.",
            "Per-item format selectors remain available after applying a bulk preset for exceptions.",
        ],
    ),
    (
        "0.1.13",
        &[
            "Playlist downloads now stay together as named folders in the main gallery.",
            "Opening a playlist folder shows only its saved and downloading items in the original playlist order.",
            "Playlist grouping persists across restarts and additional queue batches without moving playable media out of Android Downloads.",
        ],
    ),
    (
        "0.1.12",
        &[
            "Added camera-scannable QR pairing between RustDL devices without adding camera permission to the app.",
            "Pairing deep links now hand the short-lived secret directly from Android to Rust instead of placing it in localhost URLs.",
            "Added a searchable completed-media picker after pairing, with one-tap encrypted sending and manual pairing as a fallback.",
        ],
    ),
    (
        "0.1.11",
        &[
            "Added direct encrypted transfers between RustDL devices on the same local network.",
            "Added short-lived manual pairing and a Send to device action for saved media.",
            "Added resumable 1 MiB chunks with XChaCha20-Poly1305 authentication and final BLAKE3 verification.",
        ],
    ),
    (
        "0.1.10",
        &[
            "Load complete public YouTube playlists across continuation pages.",
            "Search playlists and choose individual, visible, or first-10 entries before resolving formats.",
            "Added this in-app cumulative changelog.",
        ],
    ),
    (
        "0.1.9",
        &[
            "Added the first validated YouTube playlist discovery path.",
            "Confirmed ordered playlist deduplication with a temporary five-entry validation limit.",
        ],
    ),
    (
        "0.1.8",
        &[
            "Added Snapchat Spotlight support.",
            "Added Audio-only M4A choices for every supported video source.",
            "Upgraded playback with anchored controls, resume, speed, Picture-in-Picture, and media-session actions.",
        ],
    ),
    (
        "0.1.7",
        &[
            "Added YouTube videos and Shorts.",
            "Added the per-item quality wizard and safer multi-item queue controls.",
            "Added storage management for completed, partial, duplicate, watched, and thumbnail data.",
        ],
    ),
    (
        "0.1.6",
        &[
            "Added signed in-app APK update checks, downloads, verification, and install handoff.",
            "Repaired Android package versioning and install compatibility.",
        ],
    ),
    (
        "0.1.5",
        &[
            "Added an in-app switch between normal user mode and synthetic inspection mode.",
            "Kept screenshots blocked in user mode while preserving thumbnail generation for the gallery.",
        ],
    ),
    (
        "0.1.4",
        &[
            "Redesigned the video player and repaired fullscreen playback.",
            "Added shared-thumbnail View Transitions between the gallery and single-item player.",
        ],
    ),
    (
        "0.1.3",
        &[
            "Made active downloads streamable while bytes are still arriving.",
            "Added a gallery that opens an optimized single-media player instead of embedding every video.",
        ],
    ),
    (
        "0.1.2",
        &[
            "Added the Android APK and system share target.",
            "Added secure user mode and a separate no-user-data UI inspection process.",
        ],
    ),
    (
        "0.1.1",
        &[
            "Added the browser-based video player and gallery foundation.",
            "Added Rust development hot reload for faster UI iteration.",
        ],
    ),
    (
        "0.1.0",
        &[
            "Created the pure-Rust web app and X video downloader.",
            "Added the RustDL download folder and duplicate detection.",
        ],
    ),
];

fn changelog_destinations(version: &str) -> &'static [(&'static str, &'static str)] {
    match version {
        "0.1.31" => &[("/activity", "Activity Center")],
        "0.1.30" => &[("/queue", "Live queue"), ("/#gallery-library", "Gallery")],
        "0.1.29" => &[("/settings", "Settings")],
        "0.1.28" | "0.1.27" | "0.1.26" | "0.1.23" => &[("/diagnostics", "Diagnostics")],
        "0.1.25" | "0.1.24" => &[("/changelog#version-jump", "version navigation")],
        "0.1.22" | "0.1.21" => &[],
        "0.1.20" | "0.1.19" | "0.1.4" | "0.1.3" => &[("/#gallery-library", "Gallery & player")],
        "0.1.18" => &[
            ("/#gallery-library", "Gallery"),
            ("/storage", "Storage"),
            ("/diagnostics", "Diagnostics"),
        ],
        "0.1.17" | "0.1.16" | "0.1.12" | "0.1.11" => &[("/peers", "Device transfer")],
        "0.1.15" => &[("/#gallery-library", "Gallery search")],
        "0.1.14" | "0.1.13" | "0.1.10" | "0.1.9" => &[("/#downloader", "Playlist downloader")],
        "0.1.8" => &[
            ("/#downloader", "Downloader"),
            ("/#gallery-library", "Gallery & player"),
        ],
        "0.1.7" => &[("/queue", "Download queue"), ("/storage", "Storage")],
        "0.1.6" => &[("/", "Update status")],
        "0.1.5" => &[("rustdl://mode/inspection", "Safe UI preview")],
        "0.1.2" => &[("/#downloader", "Share/download home")],
        "0.1.1" => &[("/#gallery-library", "Gallery & player")],
        "0.1.0" => &[("/#downloader", "Downloader")],
        _ => &[],
    }
}

fn render_changelog() -> String {
    let current = env!("CARGO_PKG_VERSION");
    let options = CHANGELOG
        .iter()
        .map(|(version, _)| {
            let id = format!("version-{}", version.replace('.', "-"));
            let selected = if *version == current { " selected" } else { "" };
            format!(
                r#"<option value="{id}"{selected}>Version {}</option>"#,
                escape_html(version)
            )
        })
        .collect::<String>();
    let releases = CHANGELOG
        .iter()
        .map(|(version, changes)| {
            let id = format!("version-{}", version.replace('.', "-"));
            let badge = if *version == current {
                r#"<span class="current">Current</span>"#
            } else {
                ""
            };
            let items = changes
                .iter()
                .map(|change| format!("<li>{}</li>", escape_html(change)))
                .collect::<String>();
            let destinations = changelog_destinations(version)
                .iter()
                .map(|(href, label)| {
                    format!(
                        r#"<a class="goto" href="{}">Go to {} <span aria-hidden="true">→</span></a>"#,
                        escape_html(href),
                        escape_html(label)
                    )
                })
                .collect::<String>();
            let actions = if destinations.is_empty() {
                String::new()
            } else {
                format!(r#"<footer class="release-actions"><span>Find it in RustDL</span><div>{destinations}</div></footer>"#)
            };
            format!(
                r##"<article id="{id}"><header><h2><a class="version-link" href="#{id}">Version {version}</a></h2>{badge}</header><ul>{items}</ul>{actions}</article>"##
            )
        })
        .collect::<String>();
    format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>What’s new in RustDL</title><style>
:root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif;scroll-behavior:smooth}}*{{box-sizing:border-box}}body{{min-height:100vh;margin:0;padding:clamp(1rem,4vw,3rem);color:#f7f7f8;background:radial-gradient(circle at 15% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f}}main{{width:min(100%,760px);margin:auto}}.top{{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin-bottom:1rem}}.eyebrow{{color:#8fe3d2;font-size:.7rem;font-weight:850;letter-spacing:.12em;text-transform:uppercase}}h1{{margin:.4rem 0;font-size:clamp(2.4rem,8vw,4.4rem);letter-spacing:-.055em}}p{{margin:0;color:#9ca3b3;line-height:1.55}}a,button,select{{min-height:2.65rem;padding:.65rem .8rem;border:1px solid #ffffff24;border-radius:10px;color:#dfe5ef;background:#11131bf2;text-decoration:none;font:800 .76rem/1.2 system-ui}}button,select{{cursor:pointer}}.jump{{position:sticky;z-index:10;top:max(.6rem,env(safe-area-inset-top));display:grid;grid-template-columns:auto minmax(0,1fr) auto auto;align-items:center;gap:.55rem;margin:0 0 1rem;padding:.65rem;border:1px solid #ffffff20;border-radius:15px;background:#0d0f16e8;box-shadow:0 16px 40px #0007;backdrop-filter:blur(18px)}}.jump label{{padding-left:.2rem;color:#8fe3d2;font-size:.7rem;font-weight:850}}.jump select{{width:100%}}.releases{{display:grid;gap:.8rem}}article{{scroll-margin-top:6rem;padding:1.1rem 1.15rem;border:1px solid #ffffff18;border-radius:18px;background:#11131bde;transition:border-color .2s ease,box-shadow .2s ease}}article:target{{border-color:#70dfc988;box-shadow:0 0 0 3px #70dfc916,0 22px 55px #0007}}article header{{display:flex;align-items:center;justify-content:space-between;gap:1rem}}h2{{margin:0;font-size:1rem}}.version-link{{min-height:0;padding:0;border:0;background:transparent;font-size:inherit}}.current{{padding:.35rem .55rem;border-radius:999px;color:#07110f;background:#70dfc9;font-size:.64rem;font-weight:900;letter-spacing:.08em;text-transform:uppercase}}ul{{display:grid;gap:.45rem;margin:.8rem 0 0;padding-left:1.2rem;color:#aeb5c4;font-size:.84rem;line-height:1.45}}.release-actions{{display:flex;align-items:center;justify-content:space-between;gap:.75rem;margin-top:1rem;padding-top:.85rem;border-top:1px solid #ffffff12}}.release-actions>span{{color:#747d8f;font-size:.67rem;font-weight:800;letter-spacing:.04em;text-transform:uppercase}}.release-actions>div{{display:flex;flex-wrap:wrap;justify-content:flex-end;gap:.45rem}}.goto{{display:inline-flex;align-items:center;gap:.3rem;min-height:2.25rem;padding:.5rem .65rem;color:#8fe3d2;border-color:#70dfc933;background:#70dfc90b}}.goto:active{{transform:scale(.97)}}@media(max-width:560px){{.top{{align-items:stretch;flex-direction:column}}.jump{{grid-template-columns:1fr auto}}.jump label{{grid-column:1/-1}}.jump .latest{{display:none}}.release-actions{{align-items:stretch;flex-direction:column}}.release-actions>div{{justify-content:stretch}}.goto{{flex:1;justify-content:center}}}}@media(prefers-reduced-motion:reduce){{:root{{scroll-behavior:auto}}}}
</style></head><body id="top"><main><div class="top"><div><span class="eyebrow">Release history</span><h1>What’s new.</h1><p>Every RustDL version, newest first.</p></div><a href="/">← Gallery</a></div><nav class="jump" aria-label="Changelog version navigation"><label for="version-jump">Go to version</label><select id="version-jump">{options}</select><button id="version-go" type="button">Go</button><a class="latest" href="#top">Latest</a></nav><section class="releases">{releases}</section></main><script>(()=>{{const picker=document.querySelector('#version-jump'),go=document.querySelector('#version-go');const jump=()=>{{const target=document.getElementById(picker.value);if(!target)return;history.replaceState(null,'','#'+picker.value);target.scrollIntoView({{block:'start',behavior:matchMedia('(prefers-reduced-motion: reduce)').matches?'auto':'smooth'}})}};go.addEventListener('click',jump);picker.addEventListener('change',jump);const selected=location.hash.slice(1);if(selected&&document.getElementById(selected))picker.value=selected}})();</script>{}</body></html>"##,
        dev_reload_script()
    )
}

fn render_diagnostics_page() -> String {
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>RustDL diagnostics</title><style>
:root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}}*{{box-sizing:border-box}}body{{min-height:100vh;margin:0;padding:clamp(1rem,4vw,3rem);color:#f7f7f8;background:radial-gradient(circle at 12% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f}}main{{width:min(100%,880px);margin:auto}}.top{{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin-bottom:1.3rem}}.eyebrow{{color:#8fe3d2;font-size:.7rem;font-weight:850;letter-spacing:.12em;text-transform:uppercase}}h1{{margin:.4rem 0;font-size:clamp(2.4rem,8vw,4.4rem);letter-spacing:-.055em}}p{{margin:0;color:#9ca3b3;line-height:1.5}}a,button{{min-height:2.7rem;padding:.7rem .85rem;border:1px solid #ffffff24;border-radius:11px;color:#dfe5ef;background:#ffffff0a;text-decoration:none;font:800 .76rem/1.2 system-ui;cursor:pointer}}button:disabled{{opacity:.45;cursor:default}}.toolbar{{display:flex;align-items:center;justify-content:space-between;gap:.8rem;margin-bottom:.8rem;padding:.8rem 1rem;border:1px solid #ffffff18;border-radius:16px;background:#11131bde}}.live{{display:inline-flex;align-items:center;gap:.45rem;color:#8fe3d2;font-size:.74rem;font-weight:850}}.live::before{{content:"";width:.5rem;height:.5rem;border-radius:50%;background:currentColor;box-shadow:0 0 0 .3rem #70dfc918}}.live.offline{{color:#ffb6a8}}.actions{{display:flex;gap:.5rem}}.grid{{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:.8rem}}article{{min-height:10.6rem;padding:1rem;border:1px solid #ffffff18;border-radius:20px;background:linear-gradient(145deg,#151821e8,#0d0f16ee)}}article header{{display:flex;align-items:center;justify-content:space-between;gap:.6rem}}h2{{margin:0;color:#aeb5c4;font-size:.72rem;letter-spacing:.08em;text-transform:uppercase}}.value{{min-height:2.55rem;margin:.65rem 0 .15rem;font-size:clamp(1.55rem,5vw,2.35rem);font-weight:900;letter-spacing:-.045em;font-variant-numeric:tabular-nums}}.value.unavailable{{color:#747d8f;font-size:1.1rem;letter-spacing:0}}.sub{{min-height:1.15rem;color:#7f8798;font-size:.72rem}}.bar{{height:.32rem;margin-top:1rem;overflow:hidden;border-radius:99px;background:#ffffff15}}.bar i{{display:block;width:0;height:100%;border-radius:inherit;background:#70dfc9;transition:width .35s ease}}.health{{display:grid;grid-template-columns:1fr 1fr;gap:.6rem;margin-top:.8rem}}.health div{{padding:.7rem;border:1px solid #ffffff12;border-radius:12px;background:#090b11}}.health strong,.health span{{display:block}}.health strong{{font-size:.68rem;color:#7f8798}}.health span{{margin-top:.2rem;font:800 .82rem ui-monospace,monospace}}article.wide{{grid-column:span 2}}.privacy{{grid-column:1/-1;min-height:auto;color:#91aaa5;font-size:.76rem}}@media(max-width:720px){{.grid{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}@media(max-width:520px){{.top,.toolbar{{align-items:stretch;flex-direction:column}}.grid{{grid-template-columns:1fr}}article.wide{{grid-column:auto}}.actions button{{flex:1}}}}
</style></head><body><main><div class="top"><div><span class="eyebrow">Bundled device telemetry</span><h1>Diagnostics.</h1><p>Live system health from Android APIs built into this APK.</p></div><a href="/">← Gallery</a></div>
<div class="toolbar"><span id="live" class="live" role="status" aria-live="polite">Connecting</span><div class="actions"><button id="refresh" type="button">Refresh</button><button id="copy" type="button" disabled>Copy snapshot</button></div></div><section class="grid" id="diagnostics-grid" aria-busy="true">
<article><header><h2>Battery</h2></header><div id="battery" class="value">—</div><p id="battery-sub" class="sub">Reading battery…</p><div class="bar"><i id="battery-bar"></i></div></article>
<article><header><h2>Thermal</h2></header><div id="thermal" class="value">—</div><p id="thermal-sub" class="sub">Reading temperature…</p><div class="bar"><i id="thermal-bar"></i></div></article>
<article><header><h2>CPU load</h2></header><div id="cpu" class="value">—</div><p id="cpu-sub" class="sub">1 / 5 / 15 minute load</p><div class="bar"><i id="cpu-bar"></i></div></article>
<article><header><h2>Memory</h2></header><div id="memory" class="value">—</div><p id="memory-sub" class="sub">Reading available RAM…</p><div class="bar"><i id="memory-bar"></i></div></article>
<article><header><h2>Internal storage</h2></header><div id="storage" class="value">—</div><p id="storage-sub" class="sub">Reading /data…</p><div class="bar"><i id="storage-bar"></i></div></article>
<article><header><h2>Uptime</h2></header><div id="uptime" class="value">—</div><p id="uptime-sub" class="sub">Since the last boot</p></article>
<article class="wide"><header><h2>RustDL health</h2></header><div class="health"><div><strong>Android app process</strong><span id="app-pid">—</span></div><div><strong>Rust backend</strong><span>Bundled · local</span></div></div><p id="updated" class="sub" style="margin-top:.8rem">Waiting for first sample…</p></article>
<article class="privacy"><strong>Privacy boundary:</strong> this APK-local snapshot contains system totals and RustDL’s process ID only. It excludes logs, notifications, media filenames, Wi-Fi identity, location, other apps, and screen content. Diagnostics are unavailable in inspection mode. RustDL {}</article>
</section></main><script>
(()=>{{const bridge=window.RustDLDiagnostics,settings=window.RustDLSettings,$=selector=>document.querySelector(selector);let refreshSeconds=5;try{{if(settings)refreshSeconds=Number(settings.diagnosticsRefreshSeconds())||5}}catch(_error){{refreshSeconds=5}}const refresh=$('#refresh'),copy=$('#copy'),grid=$('#diagnostics-grid');let latest=null,busy=false;const finite=value=>Number.isFinite(Number(value))?Number(value):-1,clamp=value=>Math.max(0,Math.min(100,value)),fixed=(value,digits)=>finite(value)>=0?finite(value).toFixed(digits):'—',bytes=value=>{{value=finite(value);if(value<0)return'Unavailable';const units=['B','KB','MB','GB','TB'];let index=0;while(value>=1024&&index<units.length-1){{value/=1024;index++}}return value.toFixed(index>2?1:0)+' '+units[index]}},duration=seconds=>{{seconds=finite(seconds);if(seconds<0)return'Unavailable';const days=Math.floor(seconds/86400),hours=Math.floor(seconds%86400/3600),minutes=Math.floor(seconds%3600/60);return days?days+'d '+hours+'h':hours?hours+'h '+minutes+'m':minutes+'m'}},thermalNames=['None','Light','Moderate','Severe','Critical','Emergency','Shutdown'],setBar=(id,value)=>$(id).style.width=clamp(value)+'%',setValue=(id,value,available)=>{{const element=$(id);element.textContent=value;element.classList.toggle('unavailable',!available)}};
const render=data=>{{latest=data;const batteryLevel=finite(data.batteryLevel),temperature=finite(data.batteryTemperatureC),thermalStatus=finite(data.thermalStatus),load1=finite(data.load1),load5=finite(data.load5),load15=finite(data.load15),processors=Math.max(1,finite(data.processors)),memoryTotal=finite(data.memoryTotalBytes),memoryAvailable=finite(data.memoryAvailableBytes),storageTotal=finite(data.storageTotalBytes),storageAvailable=finite(data.storageAvailableBytes),memoryUsed=memoryTotal>0&&memoryAvailable>=0?100*(1-memoryAvailable/memoryTotal):-1,storageUsed=storageTotal>0&&storageAvailable>=0?100*(1-storageAvailable/storageTotal):-1,cpuUsed=load1>=0?100*load1/processors:-1,thermalName=thermalStatus>=0?(thermalNames[thermalStatus]||'Status '+thermalStatus):'Status unavailable';setValue('#battery',batteryLevel>=0?batteryLevel+'%':'Unavailable',batteryLevel>=0);$('#battery-sub').textContent=(data.batteryStatus||'Unknown')+' · '+(temperature>=0?temperature.toFixed(1)+' °C':'temperature unavailable');setBar('#battery-bar',batteryLevel);setValue('#thermal',temperature>=0?temperature.toFixed(1)+' °C':thermalName,temperature>=0||thermalStatus>=0);$('#thermal-sub').textContent=(temperature>=0?'Battery sensor · ':'')+thermalName;setBar('#thermal-bar',temperature>=0?(temperature-20)/30*100:thermalStatus>=0?thermalStatus/6*100:0);setValue('#cpu',load1>=0?load1.toFixed(2):'Unavailable',load1>=0);$('#cpu-sub').textContent=load1>=0?fixed(load1,2)+' / '+fixed(load5,2)+' / '+fixed(load15,2)+' · '+processors+' cores':'Load average restricted by Android';setBar('#cpu-bar',cpuUsed);setValue('#memory',memoryAvailable>=0?bytes(memoryAvailable):'Unavailable',memoryAvailable>=0);$('#memory-sub').textContent=memoryUsed>=0?memoryUsed.toFixed(0)+'% used of '+bytes(memoryTotal):'Memory totals unavailable';setBar('#memory-bar',memoryUsed);setValue('#storage',storageAvailable>=0?bytes(storageAvailable):'Unavailable',storageAvailable>=0);$('#storage-sub').textContent=storageUsed>=0?storageUsed.toFixed(0)+'% used of '+bytes(storageTotal):'Storage totals unavailable';setBar('#storage-bar',storageUsed);setValue('#uptime',duration(data.uptimeSeconds),finite(data.uptimeSeconds)>=0);$('#app-pid').textContent=finite(data.rustdlPid)>0?'PID '+finite(data.rustdlPid):'Unavailable';$('#updated').textContent='Updated '+new Date(data.timestamp).toLocaleTimeString();copy.disabled=false;grid.setAttribute('aria-busy','false')}};
const load=()=>{{if(busy)return;busy=true;refresh.disabled=true;grid.setAttribute('aria-busy','true');$('#live').textContent='Refreshing…';if(!bridge){{$('#live').textContent='Open inside the installed RustDL app';$('#live').classList.add('offline');grid.setAttribute('aria-busy','false');refresh.disabled=false;busy=false;return}}try{{const response=JSON.parse(bridge.diagnostics());if(!response.ok||!response.data)throw new Error(response.detail||'No diagnostics sample');render(response.data);$('#live').textContent=(response.detail||'Live')+' · updates every '+refreshSeconds+' seconds';$('#live').classList.remove('offline')}}catch(error){{$('#live').textContent='Could not read diagnostics · tap Refresh';$('#live').classList.add('offline');grid.setAttribute('aria-busy','false')}}finally{{refresh.disabled=false;busy=false}}}};refresh.addEventListener('click',load);copy.addEventListener('click',event=>{{if(!latest||!bridge)return;const copied=bridge.copySnapshot(JSON.stringify(latest,null,2));event.currentTarget.textContent=copied?'Copied':'Copy failed';setTimeout(()=>event.currentTarget.textContent='Copy snapshot',1000)}});document.addEventListener('visibilitychange',()=>{{if(!document.hidden)load()}});load();setInterval(()=>{{if(!document.hidden)load()}},refreshSeconds*1000)}})();
</script>{}</body></html>"#,
        env!("CARGO_PKG_VERSION"),
        dev_reload_script()
    )
}

fn respond_settings_page(request: Request) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(settings::render(&dev_reload_script()))
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_diagnostics_page(request: Request) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(render_diagnostics_page())
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_changelog_page(request: Request) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(render_changelog())
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

enum InspectionScreen {
    Result,
    Player,
}

fn respond_inspection_page(
    request: Request,
    screen: InspectionScreen,
) -> Result<(), Box<dyn Error>> {
    let (title, eyebrow, heading, detail, status, filename, transition_name) = match screen {
        InspectionScreen::Result => (
            "Synthetic progressive download",
            "Download status",
            "Streaming now.",
            "Synthetic state only. The player demonstrates watching while bytes arrive without resolving a link, making a network request, or reading a user file.",
            "Downloading",
            "Downloads/RustDL/synthetic-preview.mp4",
            "video-synthetic-download",
        ),
        InspectionScreen::Player => (
            "Synthetic video player",
            "RustDL player",
            "Ready when you are.",
            "This poster and filename are generated by RustDL for safe UI inspection.",
            "No user data",
            "synthetic-preview.mp4",
            "video-synthetic-preview",
        ),
    };
    let player_css = player_css_path();
    let view_transition_script = view_transition_script_tag();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title}</title>
<link rel="stylesheet" href="{player_css}"></head><body><main class="player-shell">
<div class="topline"><a class="brand" href="/"><span class="brand-mark">↓</span><span>RustDL</span></a><span class="context-pill"><i></i>{status}</span></div>
<header><span class="eyebrow">{eyebrow}</span><h1>{heading}</h1><p class="copy">{detail}</p></header>
<section class="player-frame" style="view-transition-name:{transition_name}"><div class="player-toolbar"><span>Synthetic preview</span><span class="codec">MP4</span></div><div class="synthetic-video" role="img" aria-label="Synthetic video player preview"><span class="play-button"></span></div><span class="download-label">26.0 MB / 64.0 MB downloaded</span><div class="control-island" role="group" aria-label="Synthetic video controls"><button class="control-button" type="button">▶</button><span class="control-time">0:00 / 0:42</span><div class="timeline-shell"><span class="timeline-track"></span><span class="timeline-downloaded" style="width:41%"></span><span class="timeline-played" style="width:0"></span><span class="download-boundary" style="left:41%;opacity:1"></span><span class="scrub-anchor"></span><input class="timeline-input" type="range" value="0" aria-label="Synthetic seek"></div><button class="control-button" type="button">Vol</button><button class="control-button" type="button">1×</button><button class="control-button" type="button">PiP</button><button class="control-button" type="button">•••</button><button class="control-button" type="button">⛶</button></div></section>
<div class="meta-card"><div class="file-block"><span class="meta-label">Generated fixture</span><code>{filename}</code></div><a class="action" href="/"><span>←</span>Inspection home</a></div>
</main>{view_transition_script}</body></html>"#
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_inspection_poster(request: Request) -> Result<(), Box<dyn Error>> {
    const POSTER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720"><defs><linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#171d36"/><stop offset=".52" stop-color="#080a10"/><stop offset="1" stop-color="#0b332e"/></linearGradient><radialGradient id="glow"><stop stop-color="#70dfc9" stop-opacity=".28"/><stop offset="1" stop-color="#70dfc9" stop-opacity="0"/></radialGradient></defs><rect width="1280" height="720" fill="url(#bg)"/><circle cx="1020" cy="80" r="390" fill="url(#glow)"/><circle cx="170" cy="650" r="350" fill="url(#glow)" opacity=".45"/><g stroke="#fff" stroke-opacity=".045"><path d="M0 120h1280M0 240h1280M0 360h1280M0 480h1280M0 600h1280M160 0v720M320 0v720M480 0v720M640 0v720M800 0v720M960 0v720M1120 0v720"/></g><text x="640" y="492" text-anchor="middle" fill="#f4f6fa" font-family="sans-serif" font-size="30" font-weight="700" letter-spacing="7">RUSTDL PREVIEW</text><text x="640" y="540" text-anchor="middle" fill="#9ba4b7" font-family="sans-serif" font-size="23">Synthetic media · no user data</text></svg>"##;
    let response = Response::from_string(POSTER)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "image/svg+xml; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_inspection_capture(request: Request, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let capture = output_dir.join("inspection-capture.png");
    if !capture.is_file() {
        return respond_text(request, 404, "Inspection render is not ready");
    }
    let response = Response::from_data(fs::read(capture)?)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "image/png"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_thumbnail(
    request: Request,
    output_dir: &Path,
    thumbnail_name: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(filename) = thumbnail_name.strip_suffix(".jpg") else {
        return respond_text(request, 404, "Thumbnail not found");
    };
    if !valid_video_filename(filename) {
        return respond_text(request, 404, "Thumbnail not found");
    }
    let thumbnail = output_dir.join(".thumbnails").join(thumbnail_name);
    if !thumbnail.is_file()
        && let Some(generate) = THUMBNAIL_HOOK.get()
    {
        let source = output_dir.join(filename);
        let _ = generate(&source, filename);
    }
    if !thumbnail.is_file() {
        return respond_text(request, 404, "Thumbnail not found");
    }
    let response = Response::from_data(fs::read(thumbnail)?)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "image/jpeg"))
        .with_header(header("Cache-Control", "private, max-age=86400"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn discover_videos(
    client: &Client,
    submitted: &str,
) -> Result<Vec<DiscoveryCandidate>, Box<dyn Error>> {
    let sources = extract_supported_urls(submitted);
    if sources.is_empty() {
        return Err("no public X, YouTube, or Snapchat Spotlight links were found".into());
    }
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for source in sources.into_iter().take(10) {
        if youtube_video_id(&source).is_some() {
            let candidate = resolve_youtube_candidate(&source)?;
            if seen.insert(candidate.resolved.filename()) {
                candidates.push(candidate);
            }
            continue;
        }
        if snapchat_spotlight_id(&source).is_some() {
            let candidate = resolve_snapchat_candidate(client, &source)?;
            if seen.insert(candidate.resolved.filename()) {
                candidates.push(candidate);
            }
            continue;
        }
        let statuses = if let Some(status_id) = status_id_from_url(&source) {
            let endpoint = format!("https://api.fxtwitter.com/2/thread/{status_id}");
            let response = client.get(endpoint).send()?.error_for_status()?;
            let payload: V2ThreadResponse = response.json()?;
            if payload.code != 200 {
                return Err(format!("thread service returned status {}", payload.code).into());
            }
            let mut statuses = payload.thread.unwrap_or_default();
            if let Some(status) = payload.status {
                statuses.insert(0, status);
            }
            statuses
        } else {
            let handle = profile_handle_from_url(&source)
                .ok_or_else(|| format!("unsupported X link: {source}"))?;
            let endpoint = format!("https://api.fxtwitter.com/2/profile/{handle}/media?count=40");
            let response = client.get(endpoint).send()?.error_for_status()?;
            let payload: V2TimelineResponse = response.json()?;
            if payload.code != 200 {
                return Err(format!("profile service returned status {}", payload.code).into());
            }
            payload.results
        };
        for status in statuses {
            append_status_candidates(&status, &mut candidates, &mut seen);
            if candidates.len() >= 50 {
                break;
            }
        }
        if candidates.len() >= 50 {
            break;
        }
    }
    Ok(candidates)
}

fn append_status_candidates(
    status: &V2Status,
    candidates: &mut Vec<DiscoveryCandidate>,
    seen: &mut HashSet<String>,
) {
    if status.id.is_empty() {
        return;
    }
    let Some(media) = &status.media else {
        return;
    };
    let author = status.author.as_ref();
    let handle = author
        .map(|value| value.screen_name.as_str())
        .filter(|value| valid_x_handle(value))
        .unwrap_or("i");
    let author_label = author
        .map(|value| {
            if value.name.is_empty() {
                format!("@{}", value.screen_name)
            } else {
                format!("{} · @{}", value.name, value.screen_name)
            }
        })
        .unwrap_or_else(|| "X post".to_owned());
    for (index, video) in media.videos.iter().enumerate() {
        let video_number = index + 1;
        let filename = format!("{}-{video_number}.mp4", status.id);
        if !seen.insert(filename) {
            continue;
        }
        let source_url = format!(
            "https://x.com/{handle}/status/{}/video/{video_number}",
            status.id
        );
        let qualities = quality_variants_for_x(video, &format!("{}-{video_number}.mp4", status.id));
        candidates.push(DiscoveryCandidate {
            resolved: qualities[0].clone(),
            qualities,
            source_url,
            author: author_label.clone(),
            text: status.text.clone(),
            playlist: None,
        });
    }
}

fn quality_variants_for_x(video: &V2Video, filename: &str) -> Vec<ResolvedVideo> {
    let mut formats = video
        .formats
        .iter()
        .filter(|format| {
            !format.url.is_empty()
                && format
                    .container
                    .as_deref()
                    .is_none_or(|value| value == "mp4")
        })
        .collect::<Vec<_>>();
    formats.sort_unstable_by_key(|format| std::cmp::Reverse(format.bitrate));
    formats.dedup_by(|left, right| left.url == right.url);
    let count = formats.len();
    let mut variants = formats
        .into_iter()
        .enumerate()
        .map(|(index, format)| ResolvedVideo {
            filename: filename.to_owned(),
            media_url: format.url.clone(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some(quality_label(index, count, None, format.bitrate)),
            quality_height: None,
        })
        .collect::<Vec<_>>();
    if variants.is_empty() {
        variants.push(ResolvedVideo {
            filename: filename.to_owned(),
            media_url: video.url.clone(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Original".to_owned()),
            quality_height: None,
        });
    }
    if let Some(smallest_source) = variants.last().cloned() {
        variants.push(audio_only_variant(&smallest_source, true));
    }
    variants
}

fn audio_only_variant(source: &ResolvedVideo, extract_audio: bool) -> ResolvedVideo {
    ResolvedVideo {
        filename: replace_media_extension(&source.filename, "m4a"),
        media_url: source.media_url.clone(),
        audio_url: None,
        extract_audio,
        quality_label: Some("Audio only · M4A".to_owned()),
        quality_height: None,
    }
}

fn replace_media_extension(filename: &str, extension: &str) -> String {
    let stem = filename
        .strip_suffix(".mp4")
        .or_else(|| filename.strip_suffix(".m4a"))
        .unwrap_or(filename);
    format!("{stem}.{extension}")
}

fn quality_label(index: usize, count: usize, height: Option<u32>, bitrate: u64) -> String {
    let tier = if count <= 1 {
        "Original"
    } else if index == 0 {
        "Best"
    } else if index + 1 == count {
        "Data saver"
    } else {
        "Balanced"
    };
    let details = match (height, bitrate) {
        (Some(height), bitrate) if bitrate > 0 => {
            format!("{height}p · {}", format_bitrate(bitrate))
        }
        (Some(height), _) => format!("{height}p"),
        (None, bitrate) if bitrate > 0 => format_bitrate(bitrate),
        _ => String::new(),
    };
    if details.is_empty() {
        tier.to_owned()
    } else {
        format!("{tier} · {details}")
    }
}

fn format_bitrate(bits_per_second: u64) -> String {
    if bits_per_second >= 1_000_000 {
        format!("{:.1} Mbps", bits_per_second as f64 / 1_000_000.0)
    } else {
        format!("{} Kbps", bits_per_second / 1_000)
    }
}

fn resolve_youtube_candidate(source_url: &str) -> Result<DiscoveryCandidate, Box<dyn Error>> {
    let video_id = youtube_video_id(source_url).ok_or("invalid YouTube video URL")?;
    let canonical = format!("https://www.youtube.com/watch?v={video_id}");
    eprintln!("Resolving YouTube video {video_id}...");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let player = runtime
        .block_on(async {
            let client = RustyPipe::builder()
                .no_storage()
                .no_reporter()
                .build()
                .map_err(|error| error.to_string())?;
            let mut last_error = None;
            for client_type in [
                YouTubeClientType::Ios,
                YouTubeClientType::Android,
                YouTubeClientType::Tv,
            ] {
                match client
                    .query()
                    .player_from_client(&video_id, client_type)
                    .await
                {
                    Ok(player)
                        if player.video_streams.iter().any(|stream| {
                            stream.format == YouTubeVideoFormat::Mp4 && !stream.url.is_empty()
                        }) || (player.video_only_streams.iter().any(|stream| {
                            stream.format == YouTubeVideoFormat::Mp4
                                && stream.codec == YouTubeVideoCodec::Avc1
                                && !stream.url.is_empty()
                        }) && player.audio_streams.iter().any(|stream| {
                            stream.format == YouTubeAudioFormat::M4a
                                && stream.codec == YouTubeAudioCodec::Mp4a
                                && !stream.url.is_empty()
                        })) =>
                    {
                        return Ok(player);
                    }
                    Ok(_) => {
                        last_error = Some(format!("{client_type:?} returned no progressive MP4"));
                    }
                    Err(error) => last_error = Some(format!("{client_type:?}: {error}")),
                }
            }
            Err(last_error.unwrap_or_else(|| "no YouTube player clients were available".to_owned()))
        })
        .map_err(|error: String| -> Box<dyn Error> { error.into() })?;
    let details = player.details;
    let audio_url = player
        .audio_streams
        .iter()
        .filter(|stream| {
            stream.format == YouTubeAudioFormat::M4a
                && stream.codec == YouTubeAudioCodec::Mp4a
                && !stream.url.is_empty()
        })
        .max_by_key(|stream| stream.bitrate)
        .map(|stream| stream.url.clone());
    let has_progressive = !player.video_streams.is_empty();
    let mut streams = player
        .video_streams
        .into_iter()
        .filter(|stream| stream.format == YouTubeVideoFormat::Mp4 && !stream.url.is_empty())
        .collect::<Vec<_>>();
    if streams.is_empty() && audio_url.is_some() {
        streams = player
            .video_only_streams
            .into_iter()
            .filter(|stream| {
                stream.format == YouTubeVideoFormat::Mp4
                    && stream.codec == YouTubeVideoCodec::Avc1
                    && !stream.url.is_empty()
            })
            .collect();
    }
    streams.sort_unstable_by_key(|stream| {
        std::cmp::Reverse((stream.width.min(stream.height), stream.bitrate))
    });
    let mut seen_resolutions = HashSet::new();
    streams.retain(|stream| seen_resolutions.insert(stream.width.min(stream.height)));
    let count = streams.len();
    let qualities = streams
        .into_iter()
        .enumerate()
        .map(|(index, stream)| {
            let resolution = stream.width.min(stream.height);
            ResolvedVideo {
                filename: format!("youtube-{video_id}.mp4"),
                media_url: stream.url,
                audio_url: if has_progressive {
                    None
                } else {
                    audio_url.clone()
                },
                extract_audio: false,
                quality_label: Some(quality_label(
                    index,
                    count,
                    Some(resolution),
                    u64::from(stream.bitrate),
                )),
                quality_height: Some(resolution),
            }
        })
        .collect::<Vec<_>>();
    let mut qualities = qualities;
    if let Some(audio_url) = audio_url {
        qualities.push(ResolvedVideo {
            filename: format!("youtube-{video_id}.m4a"),
            media_url: audio_url,
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Audio only · M4A".to_owned()),
            quality_height: None,
        });
    }
    let resolved = qualities
        .first()
        .cloned()
        .ok_or("YouTube did not provide a progressive MP4 with audio")?;
    Ok(DiscoveryCandidate {
        resolved,
        qualities,
        source_url: canonical,
        author: details
            .channel_name
            .unwrap_or_else(|| "YouTube Short".to_owned()),
        text: details.name.unwrap_or_else(|| "YouTube video".to_owned()),
        playlist: None,
    })
}

const MAX_YOUTUBE_PLAYLIST_PAGE_BYTES: u64 = 6 * 1024 * 1024;

fn fetch_youtube_playlist_entries(
    client: &Client,
    playlist_id: &str,
) -> Result<YouTubePlaylist, Box<dyn Error>> {
    eprintln!("Loading all entries in YouTube playlist {playlist_id}...");
    let canonical = format!("https://www.youtube.com/playlist?list={playlist_id}");
    let response = client.get(canonical).send()?.error_for_status()?;
    let html = read_youtube_playlist_response(response)?;
    let initial_data = youtube_initial_data(&html)?;
    let data: serde_json::Value = serde_json::from_str(initial_data)?;
    let title = youtube_playlist_title(&data).unwrap_or_else(|| {
        format!(
            "YouTube playlist · {}",
            &playlist_id[..playlist_id.len().min(12)]
        )
    });
    let (mut entries, mut continuation) = initial_playlist_entries(&data)?;
    let api_key = youtube_config_string(&html, "INNERTUBE_API_KEY")
        .ok_or("YouTube playlist API key was not found")?;
    let client_version = youtube_config_string(&html, "INNERTUBE_CONTEXT_CLIENT_VERSION")
        .ok_or("YouTube playlist client version was not found")?;
    let mut seen = entries
        .iter()
        .map(|entry| entry.video_id.clone())
        .collect::<HashSet<_>>();

    for page in 1..MAX_PLAYLIST_PAGES {
        let Some(token) = continuation.take() else {
            break;
        };
        if entries.len() >= MAX_PLAYLIST_ITEMS {
            return Err(format!(
                "playlist exceeds the safety limit of {MAX_PLAYLIST_ITEMS} entries"
            )
            .into());
        }
        eprintln!("Loading playlist page {}...", page + 1);
        let mut endpoint = Url::parse("https://www.youtube.com/youtubei/v1/browse")?;
        endpoint.query_pairs_mut().append_pair("key", &api_key);
        let response = client
            .post(endpoint)
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "1")
            .header("X-YouTube-Client-Version", &client_version)
            .json(&serde_json::json!({
                "context": {
                    "client": {
                        "clientName": "WEB",
                        "clientVersion": client_version
                    }
                },
                "continuation": token
            }))
            .send()?
            .error_for_status()?;
        let body = read_youtube_playlist_response(response)?;
        let page_data: serde_json::Value = serde_json::from_str(&body)?;
        let (page_entries, next) = continuation_playlist_entries(&page_data)?;
        for entry in page_entries {
            if seen.insert(entry.video_id.clone()) {
                entries.push(entry);
            }
        }
        continuation = next;
    }
    if continuation.is_some() {
        return Err(
            format!("playlist exceeds the safety limit of {MAX_PLAYLIST_PAGES} pages").into(),
        );
    }
    if entries.is_empty() {
        return Err("the YouTube playlist has no selectable videos".into());
    }
    Ok(YouTubePlaylist {
        playlist_id: playlist_id.to_owned(),
        title,
        entries,
    })
}

fn youtube_playlist_title(data: &serde_json::Value) -> Option<String> {
    [
        "/metadata/playlistMetadataRenderer/title",
        "/header/playlistHeaderRenderer/title/simpleText",
        "/header/playlistHeaderRenderer/title/runs/0/text",
        "/header/pageHeaderRenderer/pageTitle",
        "/header/pageHeaderRenderer/content/pageHeaderViewModel/title/dynamicTextViewModel/text/content",
    ]
    .iter()
    .find_map(|pointer| data.pointer(pointer).and_then(serde_json::Value::as_str))
    .map(str::trim)
    .filter(|title| !title.is_empty())
    .map(|title| truncate_text(title, 120))
}

fn read_youtube_playlist_response(
    response: reqwest::blocking::Response,
) -> Result<String, Box<dyn Error>> {
    let final_url = response.url();
    if final_url.scheme() != "https"
        || !matches!(
            final_url.host_str(),
            Some("youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com")
        )
    {
        return Err("YouTube redirected the playlist to an unsupported host".into());
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_YOUTUBE_PLAYLIST_PAGE_BYTES)
    {
        return Err("the YouTube playlist response is unexpectedly large".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_YOUTUBE_PLAYLIST_PAGE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_YOUTUBE_PLAYLIST_PAGE_BYTES {
        return Err("the YouTube playlist response is unexpectedly large".into());
    }
    Ok(String::from_utf8(bytes)?)
}

fn youtube_initial_data(html: &str) -> Result<&str, Box<dyn Error>> {
    const MARKERS: [&str; 2] = ["var ytInitialData = ", "window[\"ytInitialData\"] = "];
    MARKERS
        .into_iter()
        .find_map(|marker| json_object_after(html, marker))
        .ok_or_else(|| "YouTube playlist data was not found".into())
}

fn youtube_config_string(html: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":");
    let start = html.find(&marker)? + marker.len();
    serde_json::Deserializer::from_str(&html[start..])
        .into_iter::<String>()
        .next()?
        .ok()
}

fn initial_playlist_entries(
    data: &serde_json::Value,
) -> Result<(Vec<YouTubePlaylistEntry>, Option<String>), Box<dyn Error>> {
    let tabs = data
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs")
        .and_then(serde_json::Value::as_array)
        .ok_or("YouTube playlist tabs were not found")?;
    let mut entries = Vec::new();
    let mut continuation = None;
    for tab in tabs {
        let Some(sections) = tab
            .pointer("/tabRenderer/content/sectionListRenderer/contents")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for section in sections {
            let Some(items) = section
                .pointer("/itemSectionRenderer/contents")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            let (page_entries, next) = playlist_entries_from_items(items);
            entries.extend(page_entries);
            continuation = next.or(continuation);
        }
    }
    Ok((entries, continuation))
}

fn continuation_playlist_entries(
    data: &serde_json::Value,
) -> Result<(Vec<YouTubePlaylistEntry>, Option<String>), Box<dyn Error>> {
    let actions = data
        .get("onResponseReceivedActions")
        .or_else(|| data.get("onResponseReceivedEndpoints"))
        .and_then(serde_json::Value::as_array)
        .ok_or("YouTube playlist continuation actions were not found")?;
    let mut entries = Vec::new();
    let mut continuation = None;
    for action in actions {
        let items = action
            .pointer("/appendContinuationItemsAction/continuationItems")
            .or_else(|| action.pointer("/reloadContinuationItemsCommand/continuationItems"))
            .and_then(serde_json::Value::as_array);
        if let Some(items) = items {
            let (page_entries, next) = playlist_entries_from_items(items);
            entries.extend(page_entries);
            continuation = next.or(continuation);
        }
    }
    Ok((entries, continuation))
}

fn playlist_entries_from_items(
    items: &[serde_json::Value],
) -> (Vec<YouTubePlaylistEntry>, Option<String>) {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut continuation = None;
    for item in items {
        if let Some(entry) = youtube_playlist_entry(item)
            && seen.insert(entry.video_id.clone())
        {
            entries.push(entry);
        }
        continuation = playlist_continuation_token(item).or(continuation);
    }
    (entries, continuation)
}

fn youtube_playlist_entry(item: &serde_json::Value) -> Option<YouTubePlaylistEntry> {
    let video_id = item
        .pointer("/playlistVideoRenderer/videoId")
        .or_else(|| item.pointer("/lockupViewModel/contentId"))
        .and_then(serde_json::Value::as_str)?;
    if !youtube_id_is_valid(video_id) {
        return None;
    }
    let title = item
        .pointer("/lockupViewModel/metadata/lockupMetadataViewModel/title/content")
        .or_else(|| item.pointer("/playlistVideoRenderer/title/runs/0/text"))
        .or_else(|| item.pointer("/playlistVideoRenderer/title/simpleText"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("YouTube video")
        .to_owned();
    let author = item
        .pointer("/lockupViewModel/metadata/lockupMetadataViewModel/metadata/contentMetadataViewModel/metadataRows/0/metadataParts/0/text/content")
        .or_else(|| item.pointer("/playlistVideoRenderer/shortBylineText/runs/0/text"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("YouTube")
        .to_owned();
    Some(YouTubePlaylistEntry {
        video_id: video_id.to_owned(),
        title,
        author,
    })
}

fn playlist_continuation_token(item: &serde_json::Value) -> Option<String> {
    item.pointer(
        "/continuationItemViewModel/continuationCommand/innertubeCommand/continuationCommand/token",
    )
    .or_else(|| {
        item.pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
    })
    .and_then(serde_json::Value::as_str)
    .map(str::to_owned)
}

fn json_object_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let marker_end = text.find(marker)? + marker.len();
    let bytes = text.as_bytes();
    let start = bytes[marker_end..].iter().position(|byte| *byte == b'{')? + marker_end;
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&text[start..=start + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

const MAX_SNAPCHAT_PAGE_BYTES: u64 = 2 * 1024 * 1024;

fn resolve_snapchat_candidate(
    client: &Client,
    source_url: &str,
) -> Result<DiscoveryCandidate, Box<dyn Error>> {
    let spotlight_id = snapchat_spotlight_id(source_url).ok_or("invalid Snapchat Spotlight URL")?;
    let canonical = format!("https://www.snapchat.com/spotlight/{spotlight_id}");
    eprintln!("Resolving Snapchat Spotlight {spotlight_id}...");
    let response = client.get(source_url).send()?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SNAPCHAT_PAGE_BYTES)
    {
        return Err("Snapchat returned an unexpectedly large page".into());
    }
    let final_url = response.url().clone();
    if !matches!(
        final_url.host_str().map(str::to_ascii_lowercase).as_deref(),
        Some("snapchat.com" | "www.snapchat.com")
    ) {
        return Err("Snapchat redirected outside snapchat.com".into());
    }
    let mut page = Vec::new();
    response
        .take(MAX_SNAPCHAT_PAGE_BYTES + 1)
        .read_to_end(&mut page)?;
    if page.len() as u64 > MAX_SNAPCHAT_PAGE_BYTES {
        return Err("Snapchat returned an unexpectedly large page".into());
    }
    let html = String::from_utf8(page)?;
    snapchat_candidate_from_html(&spotlight_id, &canonical, &final_url, &html)
}

fn snapchat_candidate_from_html(
    spotlight_id: &str,
    canonical: &str,
    final_url: &Url,
    html: &str,
) -> Result<DiscoveryCandidate, Box<dyn Error>> {
    let media_url = html_meta_content(html, "og:video")
        .or_else(|| html_meta_content(html, "og:video:secure_url"))
        .ok_or("Snapchat Spotlight page did not expose a video")?;
    let parsed_media = Url::parse(&media_url)?;
    if parsed_media.scheme() != "https" || !is_snapchat_media_host(&parsed_media) {
        return Err("Snapchat returned an untrusted media URL".into());
    }
    let width =
        html_meta_content(html, "og:video:width").and_then(|value| value.parse::<u32>().ok());
    let height =
        html_meta_content(html, "og:video:height").and_then(|value| value.parse::<u32>().ok());
    let quality_height = width.zip(height).map(|(width, height)| width.min(height));
    let quality_label = quality_height
        .map(|height| format!("Original · {height}p"))
        .unwrap_or_else(|| "Original".to_owned());
    let meta_title =
        html_meta_content(html, "og:title").unwrap_or_else(|| "Snapchat Spotlight".to_owned());
    let title_parts = meta_title
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let text = title_parts
        .get(1)
        .or_else(|| title_parts.first())
        .copied()
        .unwrap_or("Snapchat Spotlight")
        .to_owned();
    let author = final_url
        .path_segments()
        .and_then(|segments| {
            segments
                .filter_map(|segment| segment.strip_prefix('@'))
                .find(|handle| !handle.is_empty() && handle.len() <= 64)
        })
        .map(|handle| format!("@{handle} · Snapchat"))
        .unwrap_or_else(|| "Snapchat Spotlight".to_owned());
    let resolved = ResolvedVideo {
        filename: format!("snapchat-{spotlight_id}.mp4"),
        media_url,
        audio_url: None,
        extract_audio: false,
        quality_label: Some(quality_label),
        quality_height,
    };
    let audio = audio_only_variant(&resolved, true);
    Ok(DiscoveryCandidate {
        qualities: vec![resolved.clone(), audio],
        resolved,
        source_url: canonical.to_owned(),
        author,
        text,
        playlist: None,
    })
}

fn html_meta_content(html: &str, wanted: &str) -> Option<String> {
    html.split("<meta").skip(1).find_map(|remainder| {
        let tag = remainder.split_once('>')?.0;
        let key = html_attribute(tag, "property").or_else(|| html_attribute(tag, "name"))?;
        (key.eq_ignore_ascii_case(wanted))
            .then(|| html_attribute(tag, "content").map(html_entity_decode))?
    })
}

fn html_attribute<'a>(tag: &'a str, wanted: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let key_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b'_' | b':'))
        {
            index += 1;
        }
        if key_start == index {
            index += 1;
            continue;
        }
        let key = &tag[key_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if !matches!(quote, b'\'' | b'"') {
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        if key.eq_ignore_ascii_case(wanted) {
            return Some(&tag[value_start..index]);
        }
        index += 1;
    }
    None
}

fn html_entity_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn is_snapchat_media_host(url: &Url) -> bool {
    url.host_str()
        .map(str::to_ascii_lowercase)
        .is_some_and(|host| host == "sc-cdn.net" || host.ends_with(".sc-cdn.net"))
}

fn store_discovery_session(candidates: Vec<DiscoveryCandidate>) -> String {
    let token = random_token();
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while sessions.len() >= 8 {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(token, _)| token.clone())
        else {
            break;
        };
        sessions.remove(&oldest);
    }
    sessions.insert(
        token.clone(),
        DiscoverySession {
            created,
            candidates,
        },
    );
    token
}

fn random_token() -> String {
    let mut bytes = [0_u8; 16];
    let filled = File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_ok();
    if !filled {
        let fallback = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        bytes.copy_from_slice(&fallback.to_le_bytes());
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn store_playlist_session(playlist: &YouTubePlaylist) -> String {
    let token = random_token();
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sessions = PLAYLIST_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    while sessions.len() >= 8 {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(token, _)| token.clone())
        else {
            break;
        };
        sessions.remove(&oldest);
    }
    sessions.insert(
        token.clone(),
        PlaylistSession {
            created,
            playlist_id: playlist.playlist_id.clone(),
            title: playlist.title.clone(),
            entries: playlist.entries.clone(),
        },
    );
    token
}

fn respond_playlist_selection_page(
    request: Request,
    playlist: YouTubePlaylist,
) -> Result<(), Box<dyn Error>> {
    if playlist.entries.is_empty() {
        return respond_text(request, 404, "No selectable playlist videos were found");
    }
    let count = playlist.entries.len();
    let token = store_playlist_session(&playlist);
    let cards = playlist
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            format!(
                r#"<label class="candidate"><input type="checkbox" name="pick" value="{token}:{index}"><span class="check">✓</span><span class="candidate-copy"><strong><i>#{}</i> {}</strong><span>{}</span><code>youtube-{}.mp4</code></span></label>"#,
                index + 1,
                escape_html(&entry.title),
                escape_html(&entry.author),
                entry.video_id
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Choose playlist videos</title><style>{DISCOVERY_CSS}
.playlist-tools{{position:sticky;top:.5rem;z-index:3;display:grid;gap:.65rem;padding:.8rem;border:1px solid #ffffff20;border-radius:16px;background:#10131bf2;box-shadow:0 12px 30px #0007;backdrop-filter:blur(16px)}}.playlist-tools input{{width:100%;padding:.8rem .9rem;border:1px solid #ffffff28;border-radius:11px;color:#fff;background:#080a10;font:inherit;outline:none}}.playlist-tools input:focus{{border-color:#70dfc9}}.playlist-actions{{display:flex;flex-wrap:wrap;gap:.55rem}}.playlist-actions button{{padding:.65rem .8rem;color:#dce3ec;border:1px solid #ffffff24;background:#181b24;font-size:.76rem}}.playlist-actions button:hover{{background:#232833}}.candidate[hidden]{{display:none}}.candidate-copy strong i{{color:#697184;font-style:normal;font-weight:700}}.selection-bar{{position:sticky;bottom:.5rem;z-index:4;display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:.8rem;border:1px solid #70dfc94a;border-radius:16px;background:#0d1716f2;box-shadow:0 -10px 30px #0008;backdrop-filter:blur(16px)}}.selection-bar span{{color:#aeb7c5;font-size:.8rem}}.selection-bar strong{{color:#8fe3d2}}.selection-bar button{{padding:.8rem 1rem}}.selection-bar button:disabled{{cursor:not-allowed;filter:grayscale(1);opacity:.45}}@media(max-width:600px){{.selection-bar{{align-items:stretch;flex-direction:column}}}}
</style></head><body><main><header><div><span class="eyebrow">Playlist · Step 1 of 2</span><h1>{}</h1><p>Loaded all {count} playlist entries. Selected downloads will stay together in one gallery folder. Search, select up to {MAX_PLAYLIST_SELECTIONS} for this queue batch, then choose each format.</p></div><a href="/">Cancel</a></header><form id="playlist-form" action="/playlist/quality" method="get"><div class="playlist-tools"><input id="playlist-filter" type="search" placeholder="Filter title or creator" aria-label="Filter playlist"><div class="playlist-actions"><button id="select-visible" type="button">Select visible</button><button id="select-first" type="button">Select first 10</button><button id="clear-selection" type="button">Clear</button></div></div><section id="playlist-items">{cards}</section><div class="selection-bar"><span><strong id="selected-count">0</strong> selected · maximum {MAX_PLAYLIST_SELECTIONS} per batch</span><button id="playlist-continue" type="submit" disabled>Choose formats</button></div></form></main><script>
(()=>{{const max={MAX_PLAYLIST_SELECTIONS},form=document.getElementById('playlist-form'),cards=[...document.querySelectorAll('.candidate')],boxes=cards.map(card=>card.querySelector('input')),counter=document.getElementById('selected-count'),submit=document.getElementById('playlist-continue'),filter=document.getElementById('playlist-filter');const sync=()=>{{const count=boxes.filter(box=>box.checked).length;counter.textContent=count;submit.disabled=count===0||count>max;}};const clear=()=>{{boxes.forEach(box=>box.checked=false);sync();}};filter.addEventListener('input',()=>{{const query=filter.value.trim().toLocaleLowerCase();cards.forEach(card=>card.hidden=query!==''&&!card.textContent.toLocaleLowerCase().includes(query));}});document.getElementById('select-visible').addEventListener('click',()=>{{let count=boxes.filter(box=>box.checked).length;cards.forEach(card=>{{const box=card.querySelector('input');if(!card.hidden&&!box.checked&&count<max){{box.checked=true;count++;}}}});sync();}});document.getElementById('select-first').addEventListener('click',()=>{{clear();boxes.slice(0,10).forEach(box=>box.checked=true);sync();}});document.getElementById('clear-selection').addEventListener('click',clear);boxes.forEach(box=>box.addEventListener('change',sync));form.addEventListener('submit',event=>{{if(submit.disabled){{event.preventDefault();return;}}submit.disabled=true;submit.textContent='Resolving formats…';}});sync();}})();
</script>{}</body></html>"#,
        escape_html(&playlist.title),
        dev_reload_script()
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_playlist_quality_page(request: Request, picks: &[String]) -> Result<(), Box<dyn Error>> {
    let sessions = PLAYLIST_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for pick in picks {
        let Some((token, index)) = pick.split_once(':') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Some(session) = sessions.get(token) else {
            continue;
        };
        let Some(entry) = session.entries.get(index) else {
            continue;
        };
        if seen.insert(entry.video_id.clone()) {
            selected.push((
                entry.clone(),
                PlaylistMembership {
                    playlist_id: session.playlist_id.clone(),
                    title: session.title.clone(),
                    position: index + 1,
                    total: session.entries.len(),
                },
            ));
        }
    }
    drop(sessions);
    if selected.is_empty() {
        return respond_text(request, 400, "Select at least one playlist video");
    }
    if selected.len() > MAX_PLAYLIST_SELECTIONS {
        return respond_text(
            request,
            422,
            &format!(
                "Select no more than {MAX_PLAYLIST_SELECTIONS} playlist videos per queue batch"
            ),
        );
    }

    let mut candidates = Vec::new();
    let mut errors = Vec::new();
    for (entry, membership) in selected {
        let url = format!("https://www.youtube.com/watch?v={}", entry.video_id);
        match resolve_youtube_candidate(&url) {
            Ok(mut candidate) => {
                candidate.playlist = Some(membership);
                candidates.push(candidate);
            }
            Err(error) => errors.push(format!("{}: {error}", entry.video_id)),
        }
    }
    if candidates.is_empty() {
        return respond_text(
            request,
            422,
            &format!(
                "Selected videos could not be resolved: {}",
                errors.join("; ")
            ),
        );
    }
    if !errors.is_empty() {
        eprintln!(
            "Skipped unavailable playlist selections: {}",
            errors.join("; ")
        );
    }
    let token = store_discovery_session(candidates.clone());
    let quality_picks = (0..candidates.len())
        .map(|index| format!("{token}:{index}"))
        .collect::<Vec<_>>();
    respond_quality_page(request, &quality_picks)
}

fn respond_discovery_page(
    request: Request,
    candidates: Vec<DiscoveryCandidate>,
) -> Result<(), Box<dyn Error>> {
    if candidates.is_empty() {
        return respond_text(request, 404, "No downloadable media was found");
    }
    let count = candidates.len();
    let token = store_discovery_session(candidates.clone());
    let cards = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let text = if candidate.text.trim().is_empty() {
                "Video post".to_owned()
            } else {
                truncate_text(&candidate.text, 180)
            };
            format!(
                r#"<label class="candidate"><input type="checkbox" name="pick" value="{token}:{index}" checked><span class="check">✓</span><span class="candidate-copy"><strong>{}</strong><span>{}</span><code>{}</code></span></label>"#,
                escape_html(&candidate.author),
                escape_html(&text),
                escape_html(&candidate.resolved.filename())
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Choose videos</title><style>{DISCOVERY_CSS}</style></head><body><main><header><div><span class="eyebrow">Step 1 of 2</span><h1>Choose videos.</h1><p>Found {count} downloadable video(s). Uncheck anything you do not want.</p></div><a href="/">Cancel</a></header><form action="/quality" method="get"><section>{cards}</section><button type="submit">Continue to format</button></form></main>{}</body></html>"#,
        dev_reload_script()
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

const DISCOVERY_CSS: &str = r#"
:root{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*{box-sizing:border-box}body{min-height:100vh;margin:0;padding:clamp(1rem,4vw,3rem);color:#f7f7f8;background:radial-gradient(circle at 15% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f}main{width:min(100%,820px);margin:auto}header{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin-bottom:1.25rem}h1{margin:.4rem 0;font-size:clamp(2.2rem,7vw,4rem);letter-spacing:-.05em}p{margin:0;color:#9ca3b3;line-height:1.5}.eyebrow{color:#8fe3d2;font-size:.7rem;font-weight:850;letter-spacing:.12em;text-transform:uppercase}header a{padding:.65rem .8rem;border:1px solid #ffffff24;border-radius:10px;color:#dfe5ef;text-decoration:none;font-size:.76rem;font-weight:800}form{display:grid;gap:1rem}section{display:grid;gap:.7rem}.candidate{position:relative;display:flex;align-items:flex-start;gap:.85rem;padding:1rem;border:1px solid #ffffff18;border-radius:17px;background:#11131bde;cursor:pointer}.candidate:has(input:checked){border-color:#70dfc95c;background:#13201f}.candidate input{position:absolute;opacity:0}.check{display:grid;place-items:center;flex:none;width:1.55rem;height:1.55rem;border:1px solid #ffffff2d;border-radius:7px;color:transparent;background:#090a10}.candidate input:checked+.check{color:#07110f;border-color:#70dfc9;background:#70dfc9}.candidate-copy{min-width:0;display:grid;gap:.35rem}.candidate-copy strong{font-size:.86rem}.candidate-copy span{color:#a4abba;font-size:.8rem;line-height:1.45}.candidate-copy code{overflow:hidden;color:#70798b;font-size:.68rem;text-overflow:ellipsis;white-space:nowrap}button{padding:1rem;border:0;border-radius:14px;color:#07110f;background:#70dfc9;font:850 .92rem system-ui;cursor:pointer}@media(max-width:600px){header{align-items:stretch;flex-direction:column}}
"#;

const BULK_QUALITY_SCRIPT: &str = r#"<script>(()=>{
const preset=document.getElementById('bulk-format'),apply=document.getElementById('apply-bulk-format'),status=document.getElementById('bulk-format-status'),selects=[...document.querySelectorAll('.quality-card select[name="pick"]')];
const choose=(select,value)=>{const options=[...select.options],audio=options.find(option=>option.dataset.kind==='audio'),videos=options.filter(option=>option.dataset.kind==='video');let chosen;if(value==='audio')chosen=audio;else if(value==='best')chosen=videos[0];else{const target=Number(value),sized=videos.filter(option=>Number(option.dataset.height)>0),below=sized.filter(option=>Number(option.dataset.height)<=target).sort((a,b)=>Number(b.dataset.height)-Number(a.dataset.height)),above=sized.filter(option=>Number(option.dataset.height)>target).sort((a,b)=>Number(a.dataset.height)-Number(b.dataset.height));chosen=below[0]||above[0]||videos[0]}if(chosen){select.value=chosen.value;return true}return false};
const applyAll=()=>{let applied=0;for(const select of selects)if(choose(select,preset.value))applied++;status.textContent='Applied to '+applied+' selected '+(applied===1?'item':'items')};
apply.addEventListener('click',applyAll);preset.addEventListener('change',applyAll);
})();</script>"#;

fn render_quality_option(
    token: &str,
    index: usize,
    quality_index: usize,
    quality: &ResolvedVideo,
) -> String {
    let kind = if is_audio_filename(&quality.filename) {
        "audio"
    } else {
        "video"
    };
    let height = quality.quality_height.unwrap_or(0);
    format!(
        r#"<option value="{token}:{index}:{quality_index}" data-kind="{kind}" data-height="{height}">{}</option>"#,
        escape_html(quality.quality_label.as_deref().unwrap_or("Original"))
    )
}

fn respond_quality_page(request: Request, picks: &[String]) -> Result<(), Box<dyn Error>> {
    let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for pick in picks.iter().take(50) {
        let Some((token, index)) = pick.split_once(':') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Some(candidate) = sessions
            .get(token)
            .and_then(|session| session.candidates.get(index))
        else {
            continue;
        };
        if seen.insert(candidate.resolved.filename()) {
            selected.push((token.to_owned(), index, candidate.clone()));
        }
    }
    drop(sessions);
    if selected.is_empty() {
        return respond_text(request, 400, "Select at least one video");
    }
    let count = selected.len();
    let cards = selected
        .iter()
        .map(|(token, index, candidate)| {
            let options = candidate
                .qualities
                .iter()
                .enumerate()
                .map(|(quality_index, quality)| {
                    render_quality_option(token, *index, quality_index, quality)
                })
                .collect::<String>();
            format!(
                r#"<article class="quality-card"><div><strong>{}</strong><code>{}</code></div><label>Format &amp; quality<select name="pick">{options}</select></label></article>"#,
                escape_html(&candidate.author),
                escape_html(&candidate.resolved.filename()),
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Choose format</title><style>{DISCOVERY_CSS}.batch-format{{position:sticky;top:.5rem;z-index:4;display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:end;gap:.7rem;padding:.9rem;border:1px solid #70dfc94a;border-radius:16px;background:#0d1716f2;box-shadow:0 12px 30px #0008;backdrop-filter:blur(16px)}}.batch-format label{{display:grid;gap:.4rem;color:#8fe3d2;font-size:.68rem;font-weight:850;letter-spacing:.09em;text-transform:uppercase}}.batch-format button{{padding:.8rem 1rem}}.batch-format p{{grid-column:1/-1;font-size:.75rem}}.quality-card{{display:grid;grid-template-columns:minmax(0,1fr) minmax(12rem,40%);align-items:center;gap:1rem;padding:1rem;border:1px solid #ffffff18;border-radius:17px;background:#11131bde}}.quality-card div,.quality-card label{{display:grid;gap:.4rem}}.quality-card code{{overflow:hidden;color:#70798b;font-size:.68rem;text-overflow:ellipsis;white-space:nowrap}}.quality-card label{{color:#9ca3b3;font-size:.68rem;font-weight:800;letter-spacing:.08em;text-transform:uppercase}}select{{width:100%;padding:.75rem;border:1px solid #70dfc948;border-radius:11px;color:#f7f7f8;background:#090a10;font:700 .82rem system-ui}}@media(max-width:600px){{.batch-format,.quality-card{{grid-template-columns:1fr}}}}</style></head><body><main><header><div><span class="eyebrow">Step 2 of 2</span><h1>Download selected as.</h1><p>Apply one MP4 quality or M4A audio format to all {count} queue items, then adjust individual exceptions if needed.</p></div><a href="/">Cancel</a></header><form action="/import" method="get"><div class="batch-format"><label>Download selected as<select id="bulk-format"><option value="best">Video · Best available (MP4)</option><option value="1080">Video · Up to 1080p (MP4)</option><option value="720">Video · Up to 720p (MP4)</option><option value="480">Video · Up to 480p (MP4)</option><option value="audio">Audio only (M4A)</option></select></label><button id="apply-bulk-format" type="button">Apply to all</button><p id="bulk-format-status">Best available MP4 is currently selected for every item.</p></div><section>{cards}</section><button type="submit">Add {count} to queue</button></form></main>{BULK_QUALITY_SCRIPT}{}</body></html>"#,
        dev_reload_script()
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_discovery_import(
    request: Request,
    client: &Client,
    output_dir: &Path,
    picks: &[String],
) -> Result<(), Box<dyn Error>> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    let sessions = DISCOVERY_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
    let sessions = sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for pick in picks.iter().take(50) {
        let mut parts = pick.split(':');
        let (Some(token), Some(index), Some(quality_index)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Ok(quality_index) = quality_index.parse::<usize>() else {
            continue;
        };
        let Some(candidate) = sessions
            .get(token)
            .and_then(|session| session.candidates.get(index))
        else {
            continue;
        };
        let Some(resolved) = candidate.qualities.get(quality_index) else {
            continue;
        };
        if seen.insert(resolved.filename()) {
            selected.push((
                candidate.source_url.clone(),
                resolved.clone(),
                candidate.playlist.clone(),
            ));
        }
    }
    drop(sessions);
    if selected.is_empty() {
        return respond_text(request, 400, "Select at least one video");
    }
    let mut errors = Vec::new();
    for (source_url, resolved, membership) in selected {
        let filename = resolved.filename();
        match start_resolved_download(client, &source_url, resolved, output_dir) {
            Ok(_) => {
                if let Some(membership) = membership
                    && let Err(error) =
                        record_playlist_membership(output_dir, &filename, membership)
                {
                    errors.push(escape_html(&format!(
                        "Could not group {filename} into its playlist: {error}"
                    )));
                }
            }
            Err(error) => errors.push(escape_html(&error.to_string())),
        }
    }
    respond_queue_page(request, &errors)
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut value = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        value.push('…');
    }
    value
}

#[derive(Clone, Debug)]
struct StoredVideo {
    filename: String,
    bytes: u64,
    watched: bool,
    duplicate: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredFingerprint {
    bytes: u64,
    modified_nanos: u64,
    blake3: String,
}

#[derive(Clone, Debug)]
struct PartialFile {
    filename: String,
    path: PathBuf,
    bytes: u64,
    stale: bool,
}

#[derive(Debug)]
struct StorageSnapshot {
    videos: Vec<StoredVideo>,
    partials: Vec<PartialFile>,
    video_bytes: u64,
    partial_bytes: u64,
    thumbnail_bytes: u64,
    metadata_bytes: u64,
}

fn storage_snapshot(output_dir: &Path) -> io::Result<StorageSnapshot> {
    let watched = WATCHED_HOOK
        .get()
        .and_then(|hook| hook().ok())
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let now = SystemTime::now();
    let mut videos = Vec::new();
    let mut partials = Vec::new();
    if output_dir.is_dir() {
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata()?;
            if valid_video_filename(&filename) {
                videos.push(StoredVideo {
                    watched: watched.contains(&filename),
                    filename,
                    bytes: metadata.len(),
                    duplicate: false,
                });
                continue;
            }
            let Some(base) = filename
                .strip_suffix(".audio.part")
                .or_else(|| filename.strip_suffix(".part"))
            else {
                continue;
            };
            if !valid_video_filename(base) {
                continue;
            }
            let phase = download_job(base).map(|job| job.phase);
            let active = matches!(
                phase,
                Some(DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading)
            );
            let age = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();
            let stale = !active
                && (age >= Duration::from_secs(24 * 60 * 60)
                    || matches!(
                        phase,
                        Some(DownloadPhase::Failed | DownloadPhase::Cancelled)
                    ));
            partials.push(PartialFile {
                filename: base.to_owned(),
                path: entry.path(),
                bytes: metadata.len(),
                stale,
            });
        }
    }
    mark_duplicate_videos(output_dir, &mut videos)?;
    videos.sort_unstable_by(|left, right| right.filename.cmp(&left.filename));
    partials.sort_unstable_by(|left, right| right.filename.cmp(&left.filename));
    let thumbnail_bytes = directory_file_bytes(&output_dir.join(".thumbnails"))?;
    let metadata_bytes = [
        queue_state_path(output_dir),
        playlist_memberships_path(output_dir),
        fingerprints_path(output_dir),
    ]
    .iter()
    .filter_map(|path| fs::metadata(path).ok())
    .map(|metadata| metadata.len())
    .sum();
    Ok(StorageSnapshot {
        video_bytes: videos.iter().map(|video| video.bytes).sum(),
        partial_bytes: partials.iter().map(|partial| partial.bytes).sum(),
        videos,
        partials,
        thumbnail_bytes,
        metadata_bytes,
    })
}

fn directory_file_bytes(directory: &Path) -> io::Result<u64> {
    if !directory.is_dir() {
        return Ok(0);
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

fn mark_duplicate_videos(output_dir: &Path, videos: &mut [StoredVideo]) -> io::Result<()> {
    let _guard = FINGERPRINT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut stored = load_fingerprints(output_dir)?;
    let mut changed = false;
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, video) in videos.iter().enumerate() {
        by_size.entry(video.bytes).or_default().push(index);
    }
    for indexes in by_size.values().filter(|indexes| indexes.len() > 1) {
        let mut fingerprints: HashMap<String, Vec<usize>> = HashMap::new();
        for index in indexes {
            let filename = &videos[*index].filename;
            let path = output_dir.join(filename);
            let (bytes, modified_nanos) = file_signature(&path)?;
            let fingerprint = match stored.get(filename) {
                Some(value)
                    if value.bytes == bytes
                        && value.modified_nanos == modified_nanos
                        && valid_blake3_hash(&value.blake3) =>
                {
                    value.blake3.clone()
                }
                _ => {
                    let blake3 =
                        blake3_file(&path).map_err(|error| io::Error::other(error.to_string()))?;
                    stored.insert(
                        filename.clone(),
                        StoredFingerprint {
                            bytes,
                            modified_nanos,
                            blake3: blake3.clone(),
                        },
                    );
                    changed = true;
                    blake3
                }
            };
            fingerprints.entry(fingerprint).or_default().push(*index);
        }
        for indexes in fingerprints.values().filter(|indexes| indexes.len() > 1) {
            for index in indexes {
                videos[*index].duplicate = true;
            }
        }
    }
    if changed {
        persist_fingerprints(output_dir, &stored)?;
    }
    Ok(())
}

fn fingerprints_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".fingerprints.json")
}

fn load_fingerprints(output_dir: &Path) -> io::Result<HashMap<String, StoredFingerprint>> {
    match fs::read(fingerprints_path(output_dir)) {
        Ok(data) => Ok(serde_json::from_slice(&data).unwrap_or_default()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(error),
    }
}

fn persist_fingerprints(
    output_dir: &Path,
    fingerprints: &HashMap<String, StoredFingerprint>,
) -> io::Result<()> {
    let data = serde_json::to_vec(fingerprints).map_err(io::Error::other)?;
    let temporary = output_dir.join(".fingerprints.json.part");
    fs::write(&temporary, data)?;
    fs::rename(temporary, fingerprints_path(output_dir))
}

fn file_signature(path: &Path) -> io::Result<(u64, u64)> {
    let metadata = fs::metadata(path)?;
    let modified_nanos = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .try_into()
        .unwrap_or(u64::MAX);
    Ok((metadata.len(), modified_nanos))
}

fn valid_blake3_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn record_file_fingerprint(output_dir: &Path, filename: &str, blake3: &str) -> io::Result<()> {
    if !valid_video_filename(filename) || !valid_blake3_hash(blake3) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid media fingerprint",
        ));
    }
    let (bytes, modified_nanos) = file_signature(&output_dir.join(filename))?;
    let _guard = FINGERPRINT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut fingerprints = load_fingerprints(output_dir)?;
    fingerprints.insert(
        filename.to_owned(),
        StoredFingerprint {
            bytes,
            modified_nanos,
            blake3: blake3.to_ascii_lowercase(),
        },
    );
    persist_fingerprints(output_dir, &fingerprints)
}

fn record_existing_file_fingerprint_async(output: PathBuf, filename: String) {
    thread::spawn(move || {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let blake3 = blake3_file(&output)?;
            let output_dir = output
                .parent()
                .ok_or("media file has no parent directory")?;
            record_file_fingerprint(output_dir, &filename, &blake3)?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("could not cache media fingerprint: {error}");
        }
    });
}

fn respond_storage_page(
    request: Request,
    output_dir: &Path,
    notice: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let snapshot = storage_snapshot(output_dir)?;
    let total = snapshot
        .video_bytes
        .saturating_add(snapshot.partial_bytes)
        .saturating_add(snapshot.thumbnail_bytes)
        .saturating_add(snapshot.metadata_bytes);
    let watched_count = snapshot.videos.iter().filter(|video| video.watched).count();
    let duplicate_count = snapshot
        .videos
        .iter()
        .filter(|video| video.duplicate)
        .count();
    let stale_count = snapshot
        .partials
        .iter()
        .filter(|partial| partial.stale)
        .count();
    let stale_bytes = snapshot
        .partials
        .iter()
        .filter(|partial| partial.stale)
        .map(|partial| partial.bytes)
        .sum::<u64>();
    let rows = if snapshot.videos.is_empty() {
        r#"<div class="empty">No completed videos are stored in RustDL.</div>"#.to_owned()
    } else {
        snapshot
            .videos
            .iter()
            .map(|video| {
                let mut badges = String::new();
                if video.watched {
                    badges.push_str(r#"<span class="badge">Watched</span>"#);
                }
                if video.duplicate {
                    badges.push_str(r#"<span class="badge warning">Duplicate content</span>"#);
                }
                format!(
                    r#"<article><div class="file"><code>{}</code><span>{} {badges}</span></div><div class="actions"><a href="/watch/{}">Play</a><a class="danger" href="/storage/confirm?action=delete&amp;file={}">Delete</a></div></article>"#,
                    escape_html(&video.filename),
                    format_bytes(video.bytes),
                    video.filename,
                    video.filename
                )
            })
            .collect::<String>()
    };
    let partial_rows = snapshot
        .partials
        .iter()
        .map(|partial| {
            let state = if partial.stale {
                "Stale"
            } else {
                "Retained for resume"
            };
            format!(
                r#"<li><code>{}</code><span>{} · {state}</span></li>"#,
                escape_html(&partial.filename),
                format_bytes(partial.bytes)
            )
        })
        .collect::<String>();
    let watched_action = if watched_count > 0 {
        format!(
            r#"<a class="danger" href="/storage/confirm?action=watched">Remove {watched_count} watched</a>"#
        )
    } else {
        String::new()
    };
    let stale_action = if stale_count > 0 {
        format!(
            r#"<a href="/storage/confirm?action=stale">Clean {stale_count} stale partials · {}</a>"#,
            format_bytes(stale_bytes)
        )
    } else {
        String::new()
    };
    let thumbnail_action = if snapshot.thumbnail_bytes > 0 {
        format!(
            r#"<a href="/storage/confirm?action=thumbnails">Clear thumbnail cache · {}</a>"#,
            format_bytes(snapshot.thumbnail_bytes)
        )
    } else {
        String::new()
    };
    let notice = notice
        .map(|notice| format!(r#"<div class="notice">{}</div>"#, escape_html(notice)))
        .unwrap_or_default();
    let partials = if partial_rows.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="panel"><h2>Partial downloads</h2><ul>{partial_rows}</ul></section>"#
        )
    };
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>RustDL storage</title><style>{STORAGE_CSS}</style></head><body><main><header><div><span class="eyebrow">Local files</span><h1>Storage manager</h1><p>RustDL uses {} across completed videos, resumable partials, thumbnails, and queue metadata.</p></div><a href="/">← Gallery</a></header>{notice}<section class="metrics"><div><strong>{}</strong><span>Videos · {}</span></div><div><strong>{}</strong><span>Partials · {}</span></div><div><strong>{duplicate_count}</strong><span>matching files</span></div><div><strong>{watched_count}</strong><span>watched</span></div></section><nav>{watched_action}{stale_action}{thumbnail_action}</nav><section class="panel"><h2>Completed videos</h2><div class="files">{rows}</div></section>{partials}</main>{}</body></html>"#,
        format_bytes(total),
        snapshot.videos.len(),
        format_bytes(snapshot.video_bytes),
        snapshot.partials.len(),
        format_bytes(snapshot.partial_bytes),
        dev_reload_script()
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

const STORAGE_CSS: &str = r#"
:root{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}*{box-sizing:border-box}body{min-height:100vh;margin:0;padding:clamp(1rem,4vw,3rem);color:#f7f7f8;background:radial-gradient(circle at 15% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f}main{width:min(100%,900px);margin:auto}header{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin-bottom:1.25rem}h1{margin:.4rem 0;font-size:clamp(2.2rem,7vw,4rem);letter-spacing:-.05em}h2{margin:0 0 .9rem;font-size:1rem}p{max-width:42rem;margin:0;color:#9ca3b3;line-height:1.5}.eyebrow{color:#8fe3d2;font-size:.7rem;font-weight:850;letter-spacing:.12em;text-transform:uppercase}a{padding:.6rem .75rem;border:1px solid #70dfc945;border-radius:10px;color:#8fe3d2;text-decoration:none;font-size:.74rem;font-weight:800}a.danger{color:#ffaaa5;border-color:#ff706b45}.metrics{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:.65rem}.metrics div,.panel,.notice{padding:1rem;border:1px solid #ffffff18;border-radius:17px;background:#11131bde}.metrics strong{display:block;font-size:1.45rem}.metrics span,.file>span,li>span{color:#7f8798;font-size:.7rem}.notice{margin-bottom:.8rem;color:#8fe3d2;border-color:#70dfc955}nav{display:flex;flex-wrap:wrap;gap:.55rem;margin:1rem 0}.panel{margin-top:.8rem}.files{display:grid;gap:.55rem}article{display:flex;align-items:center;justify-content:space-between;gap:1rem;padding:.8rem;border-radius:12px;background:#080a10}.file{min-width:0;display:grid;gap:.35rem}.file code{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.badge{display:inline-block;margin-left:.35rem;padding:.15rem .35rem;border-radius:99px;color:#8fe3d2;background:#70dfc916;font-size:.6rem;font-weight:800;text-transform:uppercase}.badge.warning{color:#ffd38b;background:#ffb74d16}.actions{display:flex;gap:.4rem}ul{display:grid;gap:.5rem;margin:0;padding:0;list-style:none}li{display:flex;justify-content:space-between;gap:1rem;padding:.65rem;border-radius:10px;background:#080a10}.empty{color:#7f8798}button{padding:.9rem 1rem;border:0;border-radius:12px;color:#07110f;background:#70dfc9;font-weight:850}.confirm-actions{display:flex;gap:.6rem;margin-top:1rem}@media(max-width:650px){header,article,li{align-items:stretch;flex-direction:column}.metrics{grid-template-columns:repeat(2,minmax(0,1fr))}.actions{justify-content:flex-start}}
"#;

fn respond_storage_confirmation(
    request: Request,
    output_dir: &Path,
    action: Option<&str>,
    filename: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let snapshot = storage_snapshot(output_dir)?;
    let (heading, detail, hidden_file) = match action {
        Some("delete") => {
            let filename = filename.filter(|value| valid_video_filename(value));
            let Some(filename) = filename.filter(|value| output_dir.join(value).is_file()) else {
                return respond_text(request, 404, "Video not found");
            };
            (
                "Delete this video?",
                format!(
                    "This permanently removes {} from RustDL and Android Downloads.",
                    escape_html(filename)
                ),
                format!(
                    r#"<input type="hidden" name="file" value="{}">"#,
                    escape_html(filename)
                ),
            )
        }
        Some("watched") => {
            let count = snapshot.videos.iter().filter(|video| video.watched).count();
            if count == 0 {
                return respond_text(request, 404, "No watched videos to remove");
            }
            (
                "Remove watched videos?",
                format!(
                    "This permanently removes {count} completed video(s) from RustDL and Android Downloads."
                ),
                String::new(),
            )
        }
        Some("stale") => {
            let count = snapshot
                .partials
                .iter()
                .filter(|partial| partial.stale)
                .count();
            if count == 0 {
                return respond_text(request, 404, "No stale partials to remove");
            }
            (
                "Clean stale partials?",
                format!(
                    "This removes {count} incomplete transfer file(s). Completed videos are untouched."
                ),
                String::new(),
            )
        }
        Some("thumbnails") if snapshot.thumbnail_bytes > 0 => (
            "Clear thumbnail cache?",
            "Poster images will be removed and generated again when needed.".to_owned(),
            String::new(),
        ),
        _ => return respond_text(request, 400, "Invalid storage action"),
    };
    let action = action.unwrap_or_default();
    let token = action_token();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Confirm cleanup</title><style>{STORAGE_CSS}</style></head><body><main><section class="panel"><span class="eyebrow">Confirmation required</span><h1>{heading}</h1><p>{detail}</p><form class="confirm-actions" action="/storage/action" method="post"><input type="hidden" name="token" value="{token}"><input type="hidden" name="action" value="{action}">{hidden_file}<button type="submit">Confirm</button><a href="/storage">Cancel</a></form></section></main></body></html>"#
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"));
    request.respond(response)?;
    Ok(())
}

fn action_token() -> &'static str {
    ACTION_TOKEN.get_or_init(random_token)
}

fn respond_storage_action(mut request: Request, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let mut body = String::new();
    request
        .as_reader()
        .take(16 * 1024)
        .read_to_string(&mut body)?;
    let form = Url::parse(&format!("http://localhost/?{body}"))?;
    let value = |key: &str| {
        form.query_pairs()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
    };
    if value("token").as_deref() != Some(action_token()) {
        return respond_text(request, 403, "Storage action expired");
    }
    let action = value("action").unwrap_or_default();
    let result = (|| -> Result<String, Box<dyn Error>> {
        let notice = match action.as_str() {
            "delete" => {
                let filename = value("file").ok_or("missing video filename")?;
                delete_stored_video(output_dir, &filename)?;
                format!("Removed {filename} from RustDL and Android Downloads.")
            }
            "watched" => {
                let watched_hook = WATCHED_HOOK
                    .get()
                    .ok_or("watched cleanup is only available in the Android app")?;
                let watched = watched_hook()?;
                let mut removed = 0;
                for filename in watched {
                    if valid_video_filename(&filename) && output_dir.join(&filename).is_file() {
                        delete_stored_video(output_dir, &filename)?;
                        removed += 1;
                    }
                }
                format!("Removed {removed} watched video(s).")
            }
            "stale" => {
                let snapshot = storage_snapshot(output_dir)?;
                let stale = snapshot
                    .partials
                    .into_iter()
                    .filter(|partial| partial.stale)
                    .collect::<Vec<_>>();
                for partial in &stale {
                    if partial.path.is_file() {
                        fs::remove_file(&partial.path)?;
                    }
                    let output = output_dir.join(&partial.filename);
                    for companion in [part_path(&output), audio_part_path(&output)] {
                        if companion.is_file() {
                            fs::remove_file(companion)?;
                        }
                    }
                    download_jobs()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .remove(&partial.filename);
                }
                persist_download_jobs();
                format!("Removed {} stale partial download(s).", stale.len())
            }
            "thumbnails" => {
                let directory = output_dir.join(".thumbnails");
                let mut removed = 0;
                if directory.is_dir() {
                    for entry in fs::read_dir(directory)? {
                        let entry = entry?;
                        if entry.file_type()?.is_file() {
                            fs::remove_file(entry.path())?;
                            removed += 1;
                        }
                    }
                }
                format!("Cleared {removed} cached thumbnail(s).")
            }
            _ => return Err("invalid storage action".into()),
        };
        Ok(notice)
    })();
    match result {
        Ok(notice) => respond_storage_page(request, output_dir, Some(&notice)),
        Err(error) => respond_text(request, 422, &format!("Cleanup failed: {error}")),
    }
}

fn delete_stored_video(output_dir: &Path, filename: &str) -> Result<(), Box<dyn Error>> {
    if !valid_video_filename(filename) {
        return Err("invalid video filename".into());
    }
    if download_job(filename).is_some_and(|job| {
        matches!(
            job.phase,
            DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
        )
    }) {
        return Err("pause or cancel the active download before deleting it".into());
    }
    if let Some(delete) = DELETE_HOOK.get() {
        delete(filename).map_err(|error| format!("Android Downloads deletion failed: {error}"))?;
    }
    for path in [
        output_dir.join(filename),
        part_path(&output_dir.join(filename)),
        output_dir
            .join(".thumbnails")
            .join(format!("{filename}.jpg")),
        output_dir
            .join(".thumbnails")
            .join(format!("{filename}.jpg.part")),
    ] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(filename);
    persist_download_jobs();
    Ok(())
}

fn respond_queue_page(request: Request, errors: &[String]) -> Result<(), Box<dyn Error>> {
    let mut jobs = download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .map(|(filename, job)| (filename.clone(), job.clone()))
        .collect::<Vec<_>>();
    jobs.sort_unstable_by(|left, right| right.0.cmp(&left.0));
    let rows = if jobs.is_empty() {
        r#"<div class="empty">The queue is empty. Paste links on the home screen to begin.</div>"#
            .to_owned()
    } else {
        jobs.into_iter()
            .map(|(filename, job)| {
                let phase_name = download_phase_name(job.phase);
                let phase = match job.phase {
                    DownloadPhase::Queued => "Queued",
                    DownloadPhase::Starting => "Starting",
                    DownloadPhase::Downloading => "Downloading",
                    DownloadPhase::Paused => "Paused",
                    DownloadPhase::Ready => "Ready",
                    DownloadPhase::Failed => "Needs retry",
                    DownloadPhase::Cancelled => "Cancelled",
                };
                let percent = job
                    .total
                    .filter(|total| *total > 0)
                    .map(|total| (job.downloaded.saturating_mul(100) / total).min(100));
                let progress = percent
                    .map(|value| format!(r#"<i style="width:{value}%"></i>"#))
                    .unwrap_or_default();
                let size = match job.total {
                    Some(total) => format!(
                        "{} / {}",
                        format_bytes(job.downloaded),
                        format_bytes(total)
                    ),
                    None => format_bytes(job.downloaded),
                };
                let quality = job
                    .quality_label
                    .as_deref()
                    .map(|quality| {
                        format!(r#"<span class="quality">{}</span>"#, escape_html(quality))
                    })
                    .unwrap_or_default();
                let actions = match job.phase {
                    DownloadPhase::Queued
                    | DownloadPhase::Starting
                    | DownloadPhase::Downloading => format!(
                        r#"<a href="/watch/{filename}">Play</a><a href="/queue/action?file={filename}&amp;action=pause">Pause</a><a class="danger" href="/queue/action?file={filename}&amp;action=cancel">Cancel</a>"#
                    ),
                    DownloadPhase::Paused | DownloadPhase::Failed => format!(
                        r#"<a href="/queue/action?file={filename}&amp;action=resume">Resume</a><a class="danger" href="/queue/action?file={filename}&amp;action=cancel">Cancel</a>"#
                    ),
                    DownloadPhase::Ready => {
                        format!(r#"<a href="/watch/{filename}">Open player</a>"#)
                    }
                    DownloadPhase::Cancelled => format!(
                        r#"<a href="/queue/action?file={filename}&amp;action=resume">Start again</a>"#
                    ),
                };
                let error = job
                    .error
                    .filter(|error| !error.is_empty())
                    .map(|error| format!(r#"<p class="error">{}</p>"#, escape_html(&error)))
                    .unwrap_or_default();
                format!(
                    r#"<article data-filename="{}" data-phase="{phase_name}"><div class="row"><div class="info"><span class="phase">{phase}</span><code>{}</code><span class="size">{size}</span>{quality}</div><nav>{actions}</nav></div><div class="progress">{progress}</div>{error}</article>"#,
                    escape_html(&filename),
                    escape_html(&filename)
                )
            })
            .collect::<String>()
    };
    let errors = errors
        .iter()
        .map(|error| format!(r#"<p class="batch-error">{error}</p>"#))
        .collect::<String>();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>RustDL queue</title><style>
:root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif}}*{{box-sizing:border-box}}body{{min-height:100vh;margin:0;padding:clamp(1rem,4vw,3rem);color:#f7f7f8;background:radial-gradient(circle at 15% 0,#273166,transparent 32rem),radial-gradient(circle at 100% 90%,#173e3a,transparent 28rem),#090a0f}}main{{width:min(100%,820px);margin:auto}}header{{display:flex;align-items:end;justify-content:space-between;gap:1rem;margin-bottom:1.25rem}}h1{{margin:.4rem 0 0;font-size:clamp(2.2rem,7vw,4rem);letter-spacing:-.05em}}.eyebrow,.phase{{color:#8fe3d2;font-size:.7rem;font-weight:850;letter-spacing:.12em;text-transform:uppercase}}header a,nav a{{padding:.65rem .8rem;border:1px solid #70dfc948;border-radius:10px;color:#8fe3d2;text-decoration:none;font-size:.76rem;font-weight:800}}section{{display:grid;gap:.75rem}}article,.empty{{padding:1rem;border:1px solid #ffffff18;border-radius:17px;background:#11131bde}}.row{{display:flex;align-items:center;justify-content:space-between;gap:1rem}}.info{{min-width:0;display:grid;gap:.35rem}}code{{overflow:hidden;color:#e0e4ec;font-size:.76rem;text-overflow:ellipsis;white-space:nowrap}}.size{{color:#7e8595;font-size:.72rem;font-variant-numeric:tabular-nums}}.quality{{width:max-content;padding:.25rem .45rem;border:1px solid #70dfc938;border-radius:999px;color:#8fe3d2;background:#70dfc90c;font-size:.65rem;font-weight:750}}nav{{display:flex;flex-wrap:wrap;justify-content:flex-end;gap:.45rem}}nav a.danger{{color:#ffaaa5;border-color:#ff706b42}}.progress{{height:4px;margin-top:.8rem;overflow:hidden;border-radius:99px;background:#ffffff18}}.progress i{{display:block;height:100%;border-radius:inherit;background:#70dfc9}}.error,.batch-error{{margin:.7rem 0 0;color:#ffaaa5;font-size:.76rem;line-height:1.45}}.batch-error{{padding:.7rem 1rem;border-radius:10px;background:#ff706b12}}.empty{{color:#8d94a5}}@media(max-width:600px){{header,.row{{align-items:stretch;flex-direction:column}}nav{{justify-content:flex-start}}}}
</style></head><body><main><header><div><span class="eyebrow">Persistent downloads</span><h1>Smart queue</h1></div><a href="/">← Gallery</a></header>{errors}<section>{rows}</section></main>{}{}</body></html>"#,
        live_events::QUEUE_SCRIPT,
        dev_reload_script()
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn apply_queue_action(
    client: &Client,
    output_dir: &Path,
    filename: &str,
    action: &str,
) -> Result<(), String> {
    if !valid_video_filename(filename) {
        return Err("Invalid queue item".to_owned());
    }
    let mut should_schedule = false;
    {
        let mut jobs = download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let job = jobs
            .get_mut(filename)
            .ok_or_else(|| "Queue item not found".to_owned())?;
        match action {
            "pause"
                if matches!(
                    job.phase,
                    DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
                ) =>
            {
                job.phase = DownloadPhase::Paused;
            }
            "resume"
                if matches!(
                    job.phase,
                    DownloadPhase::Paused | DownloadPhase::Failed | DownloadPhase::Cancelled
                ) =>
            {
                job.phase = DownloadPhase::Queued;
                job.error = None;
                should_schedule = true;
            }
            "cancel" if job.phase != DownloadPhase::Ready => {
                job.phase = DownloadPhase::Cancelled;
                job.error = None;
            }
            _ => return Err("That action is not available for this queue item".to_owned()),
        }
    }
    persist_download_jobs();
    download_gate().1.notify_all();
    notify_transfer_state(true);
    if should_schedule {
        schedule_download_worker(
            client.clone(),
            output_dir.to_path_buf(),
            filename.to_owned(),
        );
    }
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn start_web_download(
    client: &Client,
    source_url: &str,
    output_dir: &Path,
) -> Result<(PathBuf, DownloadOutcome), Box<dyn Error>> {
    let resolved = resolve_video(client, source_url)?;
    start_resolved_download(client, source_url, resolved, output_dir)
}

fn start_resolved_download(
    client: &Client,
    source_url: &str,
    resolved: ResolvedVideo,
    output_dir: &Path,
) -> Result<(PathBuf, DownloadOutcome), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let filename = resolved.filename();
    let output = output_dir.join(&filename);
    let quality_label = resolved.quality_label.clone();
    let quality_height = resolved.quality_height;
    let audio_url = resolved.audio_url.clone();
    let extract_audio = resolved.extract_audio;

    if is_complete_download(&output)? {
        eprintln!("Duplicate detected; reusing {}", output.display());
        if let Some(publish) = PUBLISH_HOOK.get() {
            publish(&output, &filename)
                .map_err(|error| format!("could not publish to Android Downloads: {error}"))?;
        }
        set_download_job(
            &filename,
            DownloadJob {
                phase: DownloadPhase::Ready,
                downloaded: fs::metadata(&output)?.len(),
                total: Some(fs::metadata(&output)?.len()),
                error: None,
                source_url: Some(source_url.to_owned()),
                media_url: Some(resolved.media_url),
                audio_url,
                extract_audio,
                quality_label,
                quality_height,
            },
        );
        return Ok((output, DownloadOutcome::Duplicate));
    }

    {
        let mut jobs = download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if jobs.get(&filename).is_some_and(|job| {
            matches!(
                job.phase,
                DownloadPhase::Queued
                    | DownloadPhase::Starting
                    | DownloadPhase::Downloading
                    | DownloadPhase::Paused
            )
        }) {
            return Ok((output, DownloadOutcome::InProgress));
        }
        if jobs
            .get(&filename)
            .is_some_and(|job| job.quality_label != quality_label)
        {
            let partial = part_path(&output);
            if partial.is_file() {
                fs::remove_file(partial)?;
            }
            let audio_partial = audio_part_path(&output);
            if audio_partial.is_file() {
                fs::remove_file(audio_partial)?;
            }
        }
        let downloaded = fs::metadata(part_path(&output))
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        jobs.insert(
            filename.clone(),
            DownloadJob {
                phase: DownloadPhase::Queued,
                downloaded,
                total: None,
                error: None,
                source_url: Some(source_url.to_owned()),
                media_url: Some(resolved.media_url),
                audio_url,
                extract_audio,
                quality_label,
                quality_height,
            },
        );
    }
    persist_download_jobs();
    notify_transfer_state(true);
    schedule_download_worker(client.clone(), output_dir.to_path_buf(), filename.clone());

    eprintln!("Download queued for {}", output.display());
    Ok((output, DownloadOutcome::Started))
}

fn schedule_download_worker(client: Client, output_dir: PathBuf, filename: String) {
    let scheduled = {
        let mut workers = scheduled_workers()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        workers.insert(filename.clone())
    };
    if !scheduled {
        return;
    }
    thread::spawn(move || {
        if let Err(error) = run_download_worker(&client, &output_dir, &filename) {
            eprintln!("background download failed for {filename}: {error}");
            mark_download_failed(&filename, error);
        }
        scheduled_workers()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&filename);
    });
}

fn acquire_download_slot(filename: &str) -> bool {
    let (active, available) = download_gate();
    let mut active = active
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        let phase = download_job(filename).map(|job| job.phase);
        if !matches!(phase, Some(DownloadPhase::Queued | DownloadPhase::Starting)) {
            return false;
        }
        if *active < adaptive_download_limit() {
            *active += 1;
            return true;
        }
        active = available
            .wait_timeout(active, Duration::from_millis(250))
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0;
    }
}

fn release_download_slot() {
    let (active, available) = download_gate();
    let mut active = active
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *active = active.saturating_sub(1);
    available.notify_all();
}

fn run_download_worker(client: &Client, output_dir: &Path, filename: &str) -> Result<(), String> {
    if !acquire_download_slot(filename) {
        return Ok(());
    }
    let result = run_download_worker_in_slot(client, output_dir, filename);
    release_download_slot();
    result
}

fn run_download_worker_in_slot(
    client: &Client,
    output_dir: &Path,
    filename: &str,
) -> Result<(), String> {
    let job = download_job(filename).ok_or_else(|| "queue entry disappeared".to_owned())?;
    let media_url = if let Some(source_url) = job
        .source_url
        .as_deref()
        .filter(|source_url| youtube_video_id(source_url).is_some())
    {
        if let Some(media_url) = job
            .media_url
            .clone()
            .filter(|url| youtube_url_is_fresh(url))
        {
            media_url
        } else {
            let candidate =
                resolve_youtube_candidate(source_url).map_err(|error| error.to_string())?;
            let refreshed = candidate
                .qualities
                .iter()
                .filter(|quality| {
                    is_audio_filename(&quality.filename) == is_audio_filename(filename)
                })
                .min_by_key(|quality| {
                    job.quality_height
                        .zip(quality.quality_height)
                        .map(|(wanted, actual)| wanted.abs_diff(actual))
                        .unwrap_or(0)
                })
                .cloned()
                .unwrap_or(candidate.resolved);
            if let Some(current) = download_jobs()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get_mut(filename)
            {
                current.media_url = Some(refreshed.media_url.clone());
                current.audio_url = refreshed.audio_url.clone();
                current.extract_audio = refreshed.extract_audio;
            }
            persist_download_jobs();
            refreshed.media_url
        }
    } else {
        job.media_url
            .clone()
            .ok_or_else(|| "saved download has no media URL; submit the post again".to_owned())?
    };
    let audio_url = download_job(filename).and_then(|job| job.audio_url);
    let output = output_dir.join(filename);
    if let Some(audio_url) = audio_url {
        return finish_adaptive_download(client, &media_url, &audio_url, &output, filename);
    }
    let temporary = part_path(&output);
    let existing = fs::metadata(&temporary)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let (response, file, downloaded, total) =
        open_resumable_download(client, &media_url, &temporary, existing)?;

    update_download_progress(filename, downloaded, total);
    finish_web_download(
        response, file, &temporary, &output, filename, downloaded, total,
    )
}

fn audio_part_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".audio.part");
    PathBuf::from(name)
}

fn download_adaptive_track(
    client: &Client,
    url: &str,
    path: &Path,
    filename: &str,
    progress_offset: u64,
) -> Result<Option<u64>, String> {
    let existing = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let (mut response, mut file, downloaded, total) =
        open_resumable_download(client, url, path, existing)?;
    let mut received = downloaded;
    let mut buffer = vec![0_u8; adaptive_download_buffer_bytes()];
    loop {
        match download_job(filename).map(|job| job.phase) {
            Some(DownloadPhase::Paused) => {
                file.sync_all().map_err(|error| error.to_string())?;
                persist_download_jobs();
                return Ok(None);
            }
            Some(DownloadPhase::Cancelled) | None => {
                file.sync_all().map_err(|error| error.to_string())?;
                persist_download_jobs();
                return Ok(None);
            }
            _ => {}
        }
        let count = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|error| error.to_string())?;
        received += count as u64;
        update_download_progress(
            filename,
            progress_offset.saturating_add(received),
            total.map(|total| progress_offset.saturating_add(total)),
        );
    }
    if let Some(total) = total
        && received != total
    {
        return Err(format!(
            "incomplete adaptive track: received {received} of {total} bytes"
        ));
    }
    file.sync_all().map_err(|error| error.to_string())?;
    Ok(Some(received))
}

fn finish_adaptive_download(
    client: &Client,
    video_url: &str,
    audio_url: &str,
    output: &Path,
    filename: &str,
) -> Result<(), String> {
    let video_part = part_path(output);
    let audio_part = audio_part_path(output);
    let Some(video_bytes) = download_adaptive_track(client, video_url, &video_part, filename, 0)?
    else {
        return Ok(());
    };
    let Some(audio_bytes) =
        download_adaptive_track(client, audio_url, &audio_part, filename, video_bytes)?
    else {
        return Ok(());
    };
    let mux = MUX_HOOK
        .get()
        .ok_or_else(|| "adaptive YouTube muxing is available in the Android app".to_owned())?;
    mux(&video_part, &audio_part, output)?;
    fs::remove_file(&video_part).map_err(|error| error.to_string())?;
    fs::remove_file(&audio_part).map_err(|error| error.to_string())?;
    let downloaded = video_bytes.saturating_add(audio_bytes);
    record_existing_file_fingerprint_async(output.to_path_buf(), filename.to_owned());
    let previous = download_job(filename);
    let publish_error = PUBLISH_HOOK
        .get()
        .and_then(|publish| publish(output, filename).err());
    set_download_job(
        filename,
        DownloadJob {
            phase: DownloadPhase::Ready,
            downloaded,
            total: Some(downloaded),
            error: publish_error,
            source_url: previous.as_ref().and_then(|job| job.source_url.clone()),
            media_url: previous.as_ref().and_then(|job| job.media_url.clone()),
            audio_url: previous.as_ref().and_then(|job| job.audio_url.clone()),
            extract_audio: previous.as_ref().is_some_and(|job| job.extract_audio),
            quality_label: previous.as_ref().and_then(|job| job.quality_label.clone()),
            quality_height: previous.and_then(|job| job.quality_height),
        },
    );
    Ok(())
}

fn youtube_url_is_fresh(url: &str) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(key, _)| key == "expire")
                .and_then(|(_, value)| value.parse::<u64>().ok())
        })
        .is_some_and(|expires| expires > now.saturating_add(5 * 60))
}

fn open_resumable_download(
    client: &Client,
    media_url: &str,
    temporary: &Path,
    existing: u64,
) -> Result<(reqwest::blocking::Response, File, u64, Option<u64>), String> {
    let mut request = client.get(media_url);
    if existing > 0 {
        request = request.header(RANGE, format!("bytes={existing}-"));
    }
    let mut response = request.send().map_err(|error| error.to_string())?;
    if existing > 0 && response.status().as_u16() == 416 {
        response = client
            .get(media_url)
            .send()
            .map_err(|error| error.to_string())?;
    }
    response = response
        .error_for_status()
        .map_err(|error| error.to_string())?;

    let resumed = existing > 0
        && response.status().as_u16() == 206
        && content_range(&response).is_some_and(|(start, _)| start == existing);
    let downloaded = if resumed { existing } else { 0 };
    let total = if resumed {
        content_range(&response)
            .and_then(|(_, total)| total)
            .or_else(|| response.content_length().map(|length| existing + length))
    } else {
        response.content_length()
    };
    let file = if resumed {
        OpenOptions::new().create(true).append(true).open(temporary)
    } else {
        File::create(temporary)
    }
    .map_err(|error| error.to_string())?;
    Ok((response, file, downloaded, total))
}

fn content_range(response: &reqwest::blocking::Response) -> Option<(u64, Option<u64>)> {
    let value = response.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let range = value.strip_prefix("bytes ")?;
    let (bounds, total) = range.split_once('/')?;
    let (start, _) = bounds.split_once('-')?;
    Some((
        start.parse().ok()?,
        (total != "*").then(|| total.parse().ok()).flatten(),
    ))
}

fn finish_web_download(
    mut response: reqwest::blocking::Response,
    mut file: File,
    temporary: &Path,
    output: &Path,
    filename: &str,
    initial_downloaded: u64,
    total: Option<u64>,
) -> Result<(), String> {
    let mut fingerprint = blake3_hasher_for_existing(temporary, initial_downloaded)?;
    let mut buffer = vec![0_u8; adaptive_download_buffer_bytes()];
    let mut downloaded = initial_downloaded;
    loop {
        match download_job(filename).map(|job| job.phase) {
            Some(DownloadPhase::Paused) => {
                file.sync_all().map_err(|error| error.to_string())?;
                persist_download_jobs();
                return Ok(());
            }
            Some(DownloadPhase::Cancelled) | None => {
                drop(file);
                let _ = fs::remove_file(temporary);
                persist_download_jobs();
                return Ok(());
            }
            _ => {}
        }
        let count = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])
            .map_err(|error| error.to_string())?;
        fingerprint.update(&buffer[..count]);
        downloaded += count as u64;
        update_download_progress(filename, downloaded, total);
    }
    if let Some(total) = total
        && downloaded != total
    {
        return Err(format!(
            "incomplete download: received {downloaded} of {total} bytes"
        ));
    }
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    let extract_audio = download_job(filename).is_some_and(|job| job.extract_audio);
    if extract_audio {
        let extract = EXTRACT_AUDIO_HOOK
            .get()
            .ok_or_else(|| "audio-only extraction is available in the Android app".to_owned())?;
        extract(temporary, output)?;
        fs::remove_file(temporary).map_err(|error| error.to_string())?;
    } else {
        fs::rename(temporary, output).map_err(|error| error.to_string())?;
    }

    let fingerprint = if extract_audio {
        blake3_file(output).map_err(|error| error.to_string())?
    } else {
        fingerprint.finalize().to_hex().to_string()
    };
    let output_dir = output
        .parent()
        .ok_or_else(|| "download output has no parent directory".to_owned())?;
    if let Err(error) = record_file_fingerprint(output_dir, filename, &fingerprint) {
        eprintln!("could not cache downloaded media fingerprint: {error}");
    }

    let publish_error = PUBLISH_HOOK
        .get()
        .and_then(|publish| publish(output, filename).err());
    let previous = download_job(filename);
    set_download_job(
        filename,
        DownloadJob {
            phase: DownloadPhase::Ready,
            downloaded,
            total: Some(total.unwrap_or(downloaded)),
            error: publish_error.clone(),
            source_url: previous.as_ref().and_then(|job| job.source_url.clone()),
            media_url: previous.as_ref().and_then(|job| job.media_url.clone()),
            audio_url: previous.as_ref().and_then(|job| job.audio_url.clone()),
            extract_audio: previous.as_ref().is_some_and(|job| job.extract_audio),
            quality_label: previous.as_ref().and_then(|job| job.quality_label.clone()),
            quality_height: previous.and_then(|job| job.quality_height),
        },
    );
    if let Some(error) = publish_error {
        eprintln!("Android publish warning: {error}");
    }
    eprintln!("Streaming download finished at {}", output.display());
    Ok(())
}

fn set_download_job(filename: &str, job: DownloadJob) {
    download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(filename.to_owned(), job);
    persist_download_jobs();
    notify_transfer_state(true);
}

fn update_download_progress(filename: &str, downloaded: u64, total: Option<u64>) {
    if let Some(job) = download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_mut(filename)
    {
        if matches!(job.phase, DownloadPhase::Paused | DownloadPhase::Cancelled) {
            return;
        }
        job.phase = DownloadPhase::Downloading;
        job.downloaded = downloaded;
        job.total = total;
    }
    notify_transfer_state(false);
}

fn notify_transfer_state(force: bool) {
    if TRANSFER_HOOK.get().is_none() && EVENT_HOOK.get().is_none() {
        return;
    }
    let now = Instant::now();
    if !force {
        let mut last = LAST_TRANSFER_NOTICE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if last.is_some_and(|instant| now.duration_since(instant) < Duration::from_millis(750)) {
            return;
        }
        *last = Some(now);
    }
    let summary = download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .filter(|job| {
            matches!(
                job.phase,
                DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
            )
        })
        .fold(
            TransferSummary {
                count: 0,
                downloaded: 0,
                total: 0,
            },
            |mut summary, job| {
                summary.count = summary.count.saturating_add(1);
                summary.downloaded = summary.downloaded.saturating_add(job.downloaded);
                summary.total = summary.total.saturating_add(job.total.unwrap_or(0));
                summary
            },
        );
    if let Some(hook) = TRANSFER_HOOK.get()
        && let Err(error) = hook(summary)
    {
        eprintln!("Android transfer notification warning: {error}");
    }
    if let Some(hook) = EVENT_HOOK.get() {
        let event = serde_json::json!({
            "type": "queue",
            "version": 1,
            "revision": EVENT_REVISION.fetch_add(1, Ordering::Relaxed) + 1,
            "active": summary.count,
            "downloaded": summary.downloaded,
            "total": summary.total,
        })
        .to_string();
        if let Err(error) = hook(&event) {
            eprintln!("Android WebView event warning: {error}");
        }
    }
}

fn notify_simple_event(kind: &str) {
    if let Some(hook) = EVENT_HOOK.get() {
        let event = serde_json::json!({
            "type": kind,
            "version": 1,
            "revision": EVENT_REVISION.fetch_add(1, Ordering::Relaxed) + 1,
        })
        .to_string();
        if let Err(error) = hook(&event) {
            eprintln!("Android WebView event warning: {error}");
        }
    }
}

fn mark_download_failed(filename: &str, error: String) {
    let previous = download_job(filename);
    let downloaded = QUEUE_OUTPUT_DIR
        .get()
        .and_then(|directory| fs::metadata(part_path(&directory.join(filename))).ok())
        .map(|metadata| metadata.len())
        .or_else(|| previous.as_ref().map(|job| job.downloaded))
        .unwrap_or(0);
    set_download_job(
        filename,
        DownloadJob {
            phase: DownloadPhase::Failed,
            downloaded,
            total: previous.as_ref().and_then(|job| job.total),
            error: Some(error),
            source_url: previous.as_ref().and_then(|job| job.source_url.clone()),
            media_url: previous.as_ref().and_then(|job| job.media_url.clone()),
            audio_url: previous.as_ref().and_then(|job| job.audio_url.clone()),
            extract_audio: previous.as_ref().is_some_and(|job| job.extract_audio),
            quality_label: previous.as_ref().and_then(|job| job.quality_label.clone()),
            quality_height: previous.and_then(|job| job.quality_height),
        },
    );
}

fn download_job(filename: &str) -> Option<DownloadJob> {
    download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(filename)
        .cloned()
}

fn queue_state_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".queue.json")
}

fn playlist_memberships_path(output_dir: &Path) -> PathBuf {
    output_dir.join(".playlists.json")
}

fn load_playlist_memberships(output_dir: &Path) -> io::Result<HashMap<String, PlaylistMembership>> {
    match fs::read(playlist_memberships_path(output_dir)) {
        Ok(data) => Ok(
            serde_json::from_slice::<HashMap<String, PlaylistMembership>>(&data)
                .unwrap_or_default()
                .into_iter()
                .filter(|(filename, membership)| {
                    valid_video_filename(filename)
                        && valid_youtube_playlist_id(&membership.playlist_id)
                        && !membership.title.trim().is_empty()
                })
                .collect(),
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(error) => Err(error),
    }
}

fn record_playlist_membership(
    output_dir: &Path,
    filename: &str,
    membership: PlaylistMembership,
) -> io::Result<()> {
    if !valid_video_filename(filename)
        || !valid_youtube_playlist_id(&membership.playlist_id)
        || membership.title.trim().is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid playlist membership",
        ));
    }
    fs::create_dir_all(output_dir)?;
    let mut memberships = load_playlist_memberships(output_dir)?;
    memberships.insert(filename.to_owned(), membership);
    let data = serde_json::to_vec_pretty(&memberships).map_err(io::Error::other)?;
    let destination = playlist_memberships_path(output_dir);
    let temporary = output_dir.join(".playlists.json.part");
    fs::write(&temporary, data)?;
    fs::rename(temporary, destination)
}

fn persist_download_jobs() {
    if inspection_mode() {
        return;
    }
    let Some(output_dir) = QUEUE_OUTPUT_DIR.get() else {
        return;
    };
    let jobs = download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let Ok(data) = serde_json::to_vec_pretty(&jobs) else {
        return;
    };
    let destination = queue_state_path(output_dir);
    let temporary = output_dir.join(".queue.json.part");
    if fs::write(&temporary, data).is_ok() {
        let _ = fs::rename(temporary, destination);
    }
}

fn initialize_download_queue(client: &Client, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    if inspection_mode() {
        return Ok(());
    }
    let _ = QUEUE_OUTPUT_DIR.set(output_dir.to_path_buf());
    let path = queue_state_path(output_dir);
    let mut restored: HashMap<String, DownloadJob> = match fs::read(&path) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(error) => return Err(error.into()),
    };
    restored.retain(|filename, _| valid_video_filename(filename));
    for (filename, job) in &mut restored {
        let output = output_dir.join(filename);
        if is_complete_download(&output)? {
            let length = fs::metadata(output)?.len();
            job.phase = DownloadPhase::Ready;
            job.downloaded = length;
            job.total = Some(length);
            continue;
        }
        job.downloaded = fs::metadata(part_path(&output))
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if matches!(
            job.phase,
            DownloadPhase::Starting | DownloadPhase::Downloading | DownloadPhase::Queued
        ) {
            job.phase = DownloadPhase::Queued;
        }
    }
    *download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = restored;
    persist_download_jobs();
    notify_transfer_state(true);

    let resumable = download_jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|(_, job)| job.phase == DownloadPhase::Queued && job.media_url.is_some())
        .map(|(filename, _)| filename.clone())
        .collect::<Vec<_>>();
    for filename in resumable {
        schedule_download_worker(client.clone(), output_dir.to_path_buf(), filename);
    }
    Ok(())
}

fn is_complete_download(path: &Path) -> io::Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && metadata.len() > 0),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn respond_download_result(
    request: Request,
    output: &Path,
    outcome: DownloadOutcome,
) -> Result<(), Box<dyn Error>> {
    let (heading, detail, status, toolbar, media_route) = match outcome {
        DownloadOutcome::Duplicate => (
            "Already downloaded",
            "The same X video was already in your RustDL folder, so it was not downloaded twice.",
            "Ready locally",
            "Ready to play",
            "media",
        ),
        DownloadOutcome::InProgress => (
            "Streaming now.",
            "This video is already downloading. Playback reads from the file as new bytes arrive.",
            "Download active",
            "Building while you watch",
            "stream",
        ),
        DownloadOutcome::Started => (
            "Streaming now.",
            "RustDL is saving the video in the background. You can start watching while the rest downloads.",
            "Downloading",
            "Building while you watch",
            "stream",
        ),
    };
    let filename = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("downloaded file has an invalid name")?;
    let transition_name = view_transition_name(filename);
    let saved_path = escape_html(&display_output_path(output));
    let dev_reload = dev_reload_script();
    let player_css = player_css_path();
    let playback_script = playback_script_tag();
    let view_transition_script = view_transition_script_tag();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>{heading}</title>
<link rel="stylesheet" href="{player_css}"></head><body><main class="player-shell">
<div class="topline"><a class="brand" href="/"><span class="brand-mark">↓</span><span>RustDL</span></a><span class="context-pill"><i></i>{status}</span></div>
<header><span class="eyebrow">Download status</span><h1>{heading}</h1><p class="copy">{detail}</p></header>
<section class="player-frame" style="view-transition-name:{transition_name}"><div class="player-toolbar"><span>{toolbar}</span><div class="player-actions"><button class="player-control" type="button" data-player-speed>1×</button><button class="player-control" type="button" data-player-rotation aria-pressed="false">Lock rotation</button><button class="player-control" type="button" data-player-pip>PiP</button><span class="codec">MP4</span></div></div><video controls playsinline preload="auto" data-filename="{filename}" src="/{media_route}/{filename}">Your browser does not support HTML5 video.</video><span class="seek-toast" aria-live="polite"></span></section>
<div class="meta-card"><div class="file-block"><span class="meta-label">Download target</span><code>{saved_path}</code></div><a class="action" href="/"><span>▦</span>Open gallery</a></div>
</main>{playback_script}{view_transition_script}{dev_reload}</body></html>"#,
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_watch_page(
    request: Request,
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !valid_video_filename(filename) {
        return respond_text(request, 404, "Video not found");
    }
    let ready = is_complete_download(&output_dir.join(filename))?;
    let active = download_job(filename).is_some_and(|job| {
        matches!(
            job.phase,
            DownloadPhase::Queued
                | DownloadPhase::Starting
                | DownloadPhase::Downloading
                | DownloadPhase::Paused
        )
    });
    if !ready && !active {
        return respond_text(request, 404, "Video not found");
    }
    let (status, toolbar, detail, media_route) = if ready {
        (
            "Local library",
            "Now playing",
            "Streamed directly from your saved RustDL library.",
            "media",
        )
    } else {
        (
            "Downloading",
            "Building while you watch",
            "Playback continues while RustDL writes the remaining bytes.",
            "stream",
        )
    };
    let display_filename = escape_html(filename);
    let transition_name = view_transition_name(filename);
    let dev_reload = dev_reload_script();
    let audio_only = is_audio_filename(filename);
    let codec = if audio_only { "M4A" } else { "MP4" };
    let media_element = if audio_only {
        format!(
            r#"<audio class="audio-player" controls autoplay preload="auto" data-filename="{filename}" src="/{media_route}/{filename}">Your browser does not support HTML5 audio.</audio>"#
        )
    } else {
        format!(
            r#"<video controls autoplay playsinline preload="auto" data-filename="{filename}" poster="/thumbnail/{filename}.jpg" src="/{media_route}/{filename}">Your browser does not support HTML5 video.</video>"#
        )
    };
    let saved_kind = if audio_only {
        "Saved audio"
    } else {
        "Saved video"
    };
    let player_css = player_css_path();
    let playback_script = playback_script_tag();
    let view_transition_script = view_transition_script_tag();
    let body = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>RustDL player</title>
<link rel="stylesheet" href="{player_css}"></head><body><main class="player-shell">
<div class="topline"><a class="brand" href="/"><span class="brand-mark">↓</span><span>RustDL</span></a><span class="context-pill"><i></i>{status}</span></div>
<header><span class="eyebrow">RustDL player</span><h1>Ready when you are.</h1><p class="copy">{detail}</p></header>
<section class="player-frame" style="view-transition-name:{transition_name}"><div class="player-toolbar"><span>{toolbar}</span><div class="player-actions"><button class="player-control" type="button" data-player-speed>1×</button><button class="player-control" type="button" data-player-rotation aria-pressed="false">Lock rotation</button><button class="player-control" type="button" data-player-pip>PiP</button><span class="codec">{codec}</span></div></div>{media_element}<span class="seek-toast" aria-live="polite"></span></section>
<div class="meta-card"><div class="file-block"><span class="meta-label">{saved_kind}</span><code>{display_filename}</code></div><a class="action" href="/"><span>←</span>Back to library</a></div>
</main>{playback_script}{view_transition_script}{dev_reload}</body></html>"#,
    );
    let response = Response::from_string(body)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(html_csp())
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_media(
    request: Request,
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !valid_video_filename(filename) {
        return respond_text(request, 404, "Video not found");
    }
    let path = output_dir.join(filename);
    if !is_complete_download(&path)? {
        return respond_text(request, 404, "Video not found");
    }

    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let range_header = request
        .headers()
        .iter()
        .find(|value| value.field.equiv("Range"))
        .map(|value| value.value.as_str().to_owned());
    let range = match range_header {
        Some(value) => match parse_byte_range(&value, length) {
            Some(range) => Some(range),
            None => {
                let response = Response::from_string("Requested range is not satisfiable")
                    .with_status_code(StatusCode(416))
                    .with_header(header("Content-Range", &format!("bytes */{length}")))
                    .with_header(header("Accept-Ranges", "bytes"));
                request.respond(response)?;
                return Ok(());
            }
        },
        None => None,
    };

    let common_headers = || {
        vec![
            header("Content-Type", media_content_type(filename)),
            header("Accept-Ranges", "bytes"),
            header("Cache-Control", "private, max-age=3600"),
            header("X-Content-Type-Options", "nosniff"),
        ]
    };
    if let Some((start, end)) = range {
        file.seek(SeekFrom::Start(start))?;
        let bytes = end - start + 1;
        let mut headers = common_headers();
        headers.push(header(
            "Content-Range",
            &format!("bytes {start}-{end}/{length}"),
        ));
        let response = Response::new(
            StatusCode(206),
            headers,
            file.take(bytes),
            Some(usize::try_from(bytes)?),
            None,
        )
        .with_chunked_threshold(usize::MAX);
        request.respond(response)?;
    } else {
        let response = Response::new(
            StatusCode(200),
            common_headers(),
            file,
            Some(usize::try_from(length)?),
            None,
        )
        .with_chunked_threshold(usize::MAX);
        request.respond(response)?;
    }
    Ok(())
}

struct GrowingFile {
    file: File,
    filename: String,
    position: u64,
    end: Option<u64>,
}

impl GrowingFile {
    fn open(output_dir: &Path, filename: &str, start: u64, end: Option<u64>) -> io::Result<Self> {
        let output = output_dir.join(filename);
        let temporary = part_path(&output);
        let mut file = match File::open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => File::open(output)?,
            Err(error) => return Err(error),
        };
        file.seek(SeekFrom::Start(start))?;
        Ok(Self {
            file,
            filename: filename.to_owned(),
            position: start,
            end,
        })
    }
}

impl Read for GrowingFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            let limit = match self.end {
                Some(end) if self.position > end => return Ok(0),
                Some(end) => (end - self.position + 1).min(buffer.len() as u64) as usize,
                None => buffer.len(),
            };
            let count = self.file.read(&mut buffer[..limit])?;
            if count > 0 {
                self.position += count as u64;
                return Ok(count);
            }

            match download_job(&self.filename) {
                Some(job) if job.phase == DownloadPhase::Failed => {
                    return Err(io::Error::other(
                        job.error
                            .unwrap_or_else(|| "the background download failed".to_owned()),
                    ));
                }
                Some(job)
                    if matches!(
                        job.phase,
                        DownloadPhase::Queued
                            | DownloadPhase::Starting
                            | DownloadPhase::Downloading
                    ) =>
                {
                    thread::sleep(Duration::from_millis(75));
                }
                _ => return Ok(0),
            }
        }
    }
}

fn respond_growing_media(
    request: Request,
    output_dir: &Path,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    if !valid_video_filename(filename) {
        return respond_text(request, 404, "Video not found");
    }
    if is_complete_download(&output_dir.join(filename))? {
        return respond_media(request, output_dir, filename);
    }
    let Some(job) = download_job(filename) else {
        return respond_text(request, 404, "Active download not found");
    };
    if job.phase == DownloadPhase::Failed {
        return respond_text(request, 422, "The background download failed");
    }

    let range_header = request
        .headers()
        .iter()
        .find(|value| value.field.equiv("Range"))
        .map(|value| value.value.as_str().to_owned());
    let total = job.total.filter(|total| *total > 0);
    let requested_range = match (range_header, total) {
        (Some(value), Some(total)) => match parse_byte_range(&value, total) {
            Some(range) => Some(range),
            None => {
                let response = Response::from_string("Requested range is not satisfiable")
                    .with_status_code(StatusCode(416))
                    .with_header(header("Content-Range", &format!("bytes */{total}")))
                    .with_header(header("Accept-Ranges", "bytes"));
                request.respond(response)?;
                return Ok(());
            }
        },
        _ => None,
    };
    let (status, start, end, length) = match (requested_range, total) {
        (Some((start, end)), _) => (
            StatusCode(206),
            start,
            Some(end),
            Some(usize::try_from(end - start + 1)?),
        ),
        (None, Some(total)) => (
            StatusCode(200),
            0,
            Some(total - 1),
            Some(usize::try_from(total)?),
        ),
        (None, None) => (StatusCode(200), 0, None, None),
    };
    let reader = GrowingFile::open(output_dir, filename, start, end)?;
    let mut headers = vec![
        header("Content-Type", media_content_type(filename)),
        header("Accept-Ranges", "bytes"),
        header("Cache-Control", "no-store"),
        header("X-RustDL-Downloaded", &job.downloaded.to_string()),
        header("X-Content-Type-Options", "nosniff"),
    ];
    if let (Some((start, end)), Some(total)) = (requested_range, total) {
        headers.push(header(
            "Content-Range",
            &format!("bytes {start}-{end}/{total}"),
        ));
    }
    let response = Response::new(status, headers, reader, length, None)
        .with_chunked_threshold(if length.is_some() { usize::MAX } else { 0 });
    request.respond(response)?;
    Ok(())
}

fn valid_video_filename(filename: &str) -> bool {
    let Some(stem) = filename
        .strip_suffix(".mp4")
        .or_else(|| filename.strip_suffix(".m4a"))
    else {
        return false;
    };
    if let Some(video_id) = stem.strip_prefix("youtube-") {
        return video_id.len() == 11
            && video_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    }
    if let Some(spotlight_id) = stem.strip_prefix("snapchat-") {
        return (20..=160).contains(&spotlight_id.len())
            && spotlight_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    }
    let Some((status_id, video_number)) = stem.split_once('-') else {
        return false;
    };
    !status_id.is_empty()
        && status_id.bytes().all(|byte| byte.is_ascii_digit())
        && video_number.parse::<usize>().is_ok_and(|number| number > 0)
}

fn is_audio_filename(filename: &str) -> bool {
    filename.ends_with(".m4a") && valid_video_filename(filename)
}

fn media_content_type(filename: &str) -> &'static str {
    if is_audio_filename(filename) {
        "audio/mp4"
    } else {
        "video/mp4"
    }
}

fn view_transition_name(filename: &str) -> String {
    let stem = filename
        .strip_suffix(".mp4")
        .or_else(|| filename.strip_suffix(".m4a"))
        .unwrap_or(filename);
    let safe = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("video-{safe}")
}

fn parse_byte_range(value: &str, length: u64) -> Option<(u64, u64)> {
    if length == 0 || value.contains(',') {
        return None;
    }
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?.min(length);
        return (suffix > 0).then_some((length - suffix, length - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= length {
        return None;
    }
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().ok()?.min(length - 1)
    };
    (start <= end).then_some((start, end))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn default_download_dir() -> PathBuf {
    let android_downloads = Path::new("/sdcard/Download");
    if android_downloads.is_dir() {
        return android_downloads.join(DOWNLOAD_FOLDER_NAME);
    }
    if let Some(home) = env::var_os("HOME") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.is_dir() {
            return downloads.join(DOWNLOAD_FOLDER_NAME);
        }
    }
    PathBuf::from("downloads").join(DOWNLOAD_FOLDER_NAME)
}

fn display_output_path(output: &Path) -> String {
    if PUBLISH_HOOK.get().is_some()
        && let Some(filename) = output.file_name().and_then(|name| name.to_str())
    {
        return format!("Downloads/{DOWNLOAD_FOLDER_NAME}/{filename}");
    }
    output.display().to_string()
}

fn respond_dev_version(request: Request) -> Result<(), Box<dyn Error>> {
    let Ok(token) = env::var(DEV_TOKEN_ENV) else {
        return respond_text(request, 404, "Hot reload is disabled");
    };
    let response = Response::from_string(token)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "text/plain; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"));
    request.respond(response)?;
    Ok(())
}

fn respond_playback_script(request: Request) -> Result<(), Box<dyn Error>> {
    respond_immutable_asset(
        request,
        PLAYBACK_SCRIPT,
        "application/javascript; charset=utf-8",
    )
}

fn download_phase_name(phase: DownloadPhase) -> &'static str {
    match phase {
        DownloadPhase::Queued => "queued",
        DownloadPhase::Starting => "starting",
        DownloadPhase::Downloading => "downloading",
        DownloadPhase::Paused => "paused",
        DownloadPhase::Ready => "ready",
        DownloadPhase::Failed => "failed",
        DownloadPhase::Cancelled => "cancelled",
    }
}

fn app_state_job(filename: &str, job: &DownloadJob) -> serde_json::Value {
    serde_json::json!({
        "filename": filename,
        "phase": download_phase_name(job.phase),
        "downloaded": job.downloaded,
        "total": job.total,
        "quality": job.quality_label,
        "height": job.quality_height,
        "source": job.source_url,
        "error": job.error,
    })
}

fn respond_app_state(
    request: Request,
    output_dir: &Path,
    requested_filename: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let value = if inspection_mode() {
        serde_json::json!({
            "inspection": true,
            "active": 1,
            "activityActive": 1,
            "activityIssues": 0,
            "downloaded": 27_262_976_u64,
            "total": 67_108_864_u64,
            "jobs": [{
                "filename": "synthetic-preview.mp4",
                "phase": "downloading",
                "downloaded": 27_262_976_u64,
                "total": 67_108_864_u64,
                "quality": "Synthetic 1080p",
                "height": 1080,
                "source": null,
            }],
            "current": requested_filename.map(|_| serde_json::json!({
                "filename": "synthetic-preview.mp4",
                "phase": "downloading",
                "downloaded": 27_262_976_u64,
                "total": 67_108_864_u64,
                "quality": "Synthetic 1080p",
                "height": 1080,
                "source": null,
            })),
        })
    } else {
        let mut jobs = download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|(filename, _)| valid_video_filename(filename))
            .map(|(filename, job)| (filename.clone(), job.clone()))
            .collect::<Vec<_>>();
        jobs.sort_unstable_by(|left, right| right.0.cmp(&left.0));
        let active = jobs
            .iter()
            .filter(|(_, job)| {
                matches!(
                    job.phase,
                    DownloadPhase::Queued | DownloadPhase::Starting | DownloadPhase::Downloading
                )
            })
            .count();
        let downloaded = jobs.iter().fold(0_u64, |total, (_, job)| {
            total.saturating_add(job.downloaded)
        });
        let total = jobs.iter().fold(0_u64, |sum, (_, job)| {
            sum.saturating_add(job.total.unwrap_or(0))
        });
        let current = requested_filename
            .filter(|filename| valid_video_filename(filename))
            .and_then(|filename| {
                jobs.iter()
                    .find(|(candidate, _)| candidate == filename)
                    .map(|(_, job)| app_state_job(filename, job))
                    .or_else(|| {
                        fs::metadata(output_dir.join(filename))
                            .ok()
                            .map(|metadata| {
                                serde_json::json!({
                                    "filename": filename,
                                    "phase": "ready",
                                    "downloaded": metadata.len(),
                                    "total": metadata.len(),
                                    "quality": null,
                                    "height": null,
                                    "source": null,
                                })
                            })
                    })
            });
        let activity = activity_state::snapshot();
        serde_json::json!({
            "inspection": false,
            "active": active,
            "activityActive": activity["counts"]["active"].clone(),
            "activityIssues": activity["counts"]["issues"].clone(),
            "downloaded": downloaded,
            "total": total,
            "jobs": jobs
                .iter()
                .map(|(filename, job)| app_state_job(filename, job))
                .collect::<Vec<_>>(),
            "current": current,
        })
    };
    let response = Response::from_string(serde_json::to_string(&value)?)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", "application/json; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn respond_view_transition_script(request: Request) -> Result<(), Box<dyn Error>> {
    respond_immutable_asset(
        request,
        VIEW_TRANSITION_SCRIPT,
        "application/javascript; charset=utf-8",
    )
}

fn respond_immutable_asset(
    request: Request,
    content: &'static str,
    content_type: &'static str,
) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(content)
        .with_status_code(StatusCode(200))
        .with_header(header("Content-Type", content_type))
        .with_header(header(
            "Cache-Control",
            "private, max-age=31536000, immutable",
        ))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn dev_reload_script() -> String {
    let Ok(token) = env::var(DEV_TOKEN_ENV) else {
        return String::new();
    };
    format!(
        r#"<script>(()=>{{let version="{token}";setInterval(async()=>{{try{{const response=await fetch('/__dev/version',{{cache:'no-store'}});if(!response.ok)return;const next=await response.text();if(next!==version)location.reload()}}catch(_error){{}}}},500)}})();</script>"#
    )
}

fn html_csp() -> Header {
    header(
        "Content-Security-Policy",
        "default-src 'none'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; img-src 'self'; media-src 'self'; connect-src 'self'; form-action 'self'; base-uri 'none'",
    )
}

fn respond_text(request: Request, status: u16, body: &str) -> Result<(), Box<dyn Error>> {
    let response = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(header("Content-Type", "text/plain; charset=utf-8"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response)?;
    Ok(())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name, value).expect("static header name and safe header value")
}

fn status_id_from_url(url: &str) -> Option<&str> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if !matches!(
        host,
        "x.com"
            | "www.x.com"
            | "mobile.x.com"
            | "twitter.com"
            | "www.twitter.com"
            | "mobile.twitter.com"
    ) {
        return None;
    }
    let (_, tail) = url.split_once("/status/")?;
    let id = tail.split(['/', '?', '#']).next()?;
    (!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())).then_some(id)
}

fn extract_download_urls(text: &str) -> Vec<String> {
    extract_supported_urls(text)
        .into_iter()
        .filter(|candidate| {
            status_id_from_url(candidate).is_some()
                || youtube_video_id(candidate).is_some()
                || snapchat_spotlight_id(candidate).is_some()
        })
        .take(50)
        .collect()
}

fn extract_supported_urls(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split_whitespace()
        .map(trim_url_punctuation)
        .filter(|candidate| {
            status_id_from_url(candidate).is_some()
                || profile_handle_from_url(candidate).is_some()
                || youtube_video_id(candidate).is_some()
                || youtube_playlist_id(candidate).is_some()
                || snapchat_spotlight_id(candidate).is_some()
        })
        .filter(|candidate| seen.insert((*candidate).to_owned()))
        .take(50)
        .map(str::to_owned)
        .collect()
}

fn youtube_playlist_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com"
    ) || parsed.path() != "/playlist"
    {
        return None;
    }
    let playlist_id = parsed
        .query_pairs()
        .find(|(key, _)| key == "list")
        .map(|(_, value)| value.into_owned())?;
    valid_youtube_playlist_id(&playlist_id).then_some(playlist_id)
}

fn valid_youtube_playlist_id(playlist_id: &str) -> bool {
    (10..=128).contains(&playlist_id.len())
        && playlist_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn youtube_id_is_valid(video_id: &str) -> bool {
    video_id.len() == 11
        && video_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn snapchat_spotlight_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !matches!(host.as_str(), "snapchat.com" | "www.snapchat.com") {
        return None;
    }
    let segments = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let spotlight_index = segments
        .iter()
        .position(|segment| *segment == "spotlight")?;
    if spotlight_index > 1 || (spotlight_index == 1 && !segments[0].starts_with('@')) {
        return None;
    }
    let id = *segments.get(spotlight_index + 1)?;
    if segments.len() != spotlight_index + 2 {
        return None;
    }
    ((20..=160).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')))
    .then(|| id.to_owned())
}

fn youtube_video_id(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let candidate = if host == "youtu.be" || host == "www.youtu.be" {
        parsed
            .path_segments()?
            .find(|segment| !segment.is_empty())?
            .to_owned()
    } else if matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com"
    ) {
        let mut segments = parsed
            .path_segments()?
            .filter(|segment| !segment.is_empty());
        match segments.next() {
            Some("shorts" | "embed" | "live") => segments.next()?.to_owned(),
            Some("watch") | None => parsed
                .query_pairs()
                .find(|(key, _)| key == "v")
                .map(|(_, value)| value.into_owned())?,
            _ => return None,
        }
    } else {
        return None;
    };
    youtube_id_is_valid(&candidate).then_some(candidate)
}

fn trim_url_punctuation(candidate: &str) -> &str {
    candidate.trim_matches(|character: char| {
        matches!(
            character,
            '<' | '>'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '"'
                | '\''
                | ','
                | ';'
                | '!'
                | '.'
                | '?'
                | ':'
        )
    })
}

fn profile_handle_from_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if !matches!(
        host,
        "x.com"
            | "www.x.com"
            | "mobile.x.com"
            | "twitter.com"
            | "www.twitter.com"
            | "mobile.twitter.com"
    ) {
        return None;
    }
    let mut segments = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty());
    let handle = segments.next()?;
    if segments.next().is_some() || !valid_x_handle(handle) {
        return None;
    }
    (!matches!(
        handle.to_ascii_lowercase().as_str(),
        "home" | "explore" | "search" | "notifications" | "messages" | "settings" | "compose"
    ))
    .then(|| handle.to_owned())
}

fn valid_x_handle(handle: &str) -> bool {
    !handle.is_empty()
        && handle.len() <= 15
        && handle
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn video_number_from_url(url: &str) -> Option<usize> {
    let (_, tail) = url.split_once("/video/")?;
    let value = tail.split(['/', '?', '#']).next()?;
    value.parse::<usize>().ok().filter(|number| *number > 0)
}

fn best_mp4_url(video: &Video) -> &str {
    video
        .variants
        .iter()
        .filter(|variant| variant.content_type == "video/mp4")
        .max_by_key(|variant| variant.bitrate)
        .map(|variant| variant.url.as_str())
        .unwrap_or(&video.url)
}

fn part_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn download(client: &Client, url: &str, output: &Path, force: bool) -> Result<(), Box<dyn Error>> {
    let mut response = client.get(url).send()?.error_for_status()?;
    let total = response.content_length();
    let temporary = part_path(output);
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }

    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut file = File::create(&temporary)?;
        let mut buffer = [0_u8; 64 * 1024];
        let mut downloaded = 0_u64;
        let mut last_progress = None;

        loop {
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            downloaded += count as u64;
            show_progress(downloaded, total, &mut last_progress)?;
        }
        if let Some(total) = total
            && downloaded != total
        {
            return Err(
                format!("incomplete download: received {downloaded} of {total} bytes").into(),
            );
        }
        file.sync_all()?;
        eprintln!();

        if force && output.exists() {
            fs::remove_file(output)?;
        }
        fs::rename(&temporary, output)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn show_progress(
    downloaded: u64,
    total: Option<u64>,
    last_progress: &mut Option<u64>,
) -> io::Result<()> {
    let progress = match total {
        Some(total) if total > 0 => {
            let percent = downloaded.saturating_mul(100) / total;
            if *last_progress != Some(percent) {
                eprint!(
                    "\r{percent:3}%  {} / {}",
                    human_bytes(downloaded),
                    human_bytes(total)
                );
            }
            percent
        }
        _ => {
            let mebibytes = downloaded / (1024 * 1024);
            if *last_progress != Some(mebibytes) {
                eprint!("\r{} downloaded", human_bytes(downloaded));
            }
            mebibytes
        }
    };
    if *last_progress != Some(progress) {
        *last_progress = Some(progress);
        io::stderr().flush()?;
    }
    Ok(())
}

fn human_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_id_and_video_number() {
        let url = "https://x.com/user/status/2091257067264733401/video/2?ref=test";
        assert_eq!(status_id_from_url(url), Some("2091257067264733401"));
        assert_eq!(video_number_from_url(url), Some(2));
    }

    #[test]
    fn rejects_non_numeric_status_id() {
        assert_eq!(status_id_from_url("https://x.com/u/status/nope"), None);
    }

    #[test]
    fn extracts_and_deduplicates_batch_links() {
        let links = extract_download_urls(
            "first https://x.com/a/status/123/video/1\n\
             duplicate https://x.com/a/status/123/video/1 and \
             https://twitter.com/b/status/456/video/2.",
        );
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], "https://x.com/a/status/123/video/1");
        assert_eq!(links[1], "https://twitter.com/b/status/456/video/2");
    }

    #[test]
    fn extracts_playlist_links_for_discovery() {
        let playlist =
            "https://youtube.com/playlist?list=PLoSF8YdZLL8bmNYuiUCst9NMkq4dI5YOq&si=test";
        let links = extract_supported_urls(&format!("music {playlist}"));
        assert_eq!(links, vec![playlist.to_owned()]);
        assert!(extract_download_urls(playlist).is_empty());
    }

    #[test]
    fn accepts_profile_links_for_discovery() {
        assert_eq!(
            profile_handle_from_url("https://x.com/AshtonLaxsma"),
            Some("AshtonLaxsma".to_owned())
        );
        assert_eq!(profile_handle_from_url("https://x.com/home"), None);
        assert_eq!(
            profile_handle_from_url("https://x.com/user/status/123"),
            None
        );
    }

    #[test]
    fn parses_youtube_shorts_and_watch_links() {
        assert_eq!(
            youtube_video_id("https://youtube.com/shorts/xw13xAOyZTw?si=test"),
            Some("xw13xAOyZTw".to_owned())
        );
        assert_eq!(
            youtube_video_id("https://www.youtube.com/watch?v=xw13xAOyZTw"),
            Some("xw13xAOyZTw".to_owned())
        );
        assert_eq!(
            youtube_video_id("https://youtu.be/xw13xAOyZTw"),
            Some("xw13xAOyZTw".to_owned())
        );
    }

    #[test]
    fn parses_youtube_playlist_links() {
        let id = "PLoSF8YdZLL8bmNYuiUCst9NMkq4dI5YOq";
        assert_eq!(
            youtube_playlist_id(&format!(
                "https://youtube.com/playlist?list={id}&si=XvucIps2QoTucO5q"
            )),
            Some(id.to_owned())
        );
        assert_eq!(
            youtube_playlist_id(&format!("https://example.com/playlist?list={id}")),
            None
        );
        assert_eq!(
            youtube_playlist_id(&format!("http://youtube.com/playlist?list={id}")),
            None
        );
    }

    #[test]
    fn extracts_all_ordered_playlist_items_from_current_youtube_page() {
        let html = r#"<script>var ytInitialData = {
          "contents":{"twoColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{
            "sectionListRenderer":{"contents":[{"itemSectionRenderer":{"contents":[
              {"lockupViewModel":{"contentId":"AAAAAAAAAAA","metadata":{"lockupMetadataViewModel":{"title":{"content":"brace } and escaped \" quote"},"metadata":{"contentMetadataViewModel":{"metadataRows":[{"metadataParts":[{"text":{"content":"Creator A"}}]}]}}}}}},
              {"lockupViewModel":{"contentId":"BBBBBBBBBBB"}},
              {"lockupViewModel":{"contentId":"AAAAAAAAAAA"}},
              {"playlistVideoRenderer":{"videoId":"CCCCCCCCCCC"}},
              {"lockupViewModel":{"contentId":"DDDDDDDDDDD"}},
              {"lockupViewModel":{"contentId":"EEEEEEEEEEE"}},
              {"lockupViewModel":{"contentId":"FFFFFFFFFFF"}},
              {"continuationItemViewModel":{"continuationCommand":{"innertubeCommand":{"continuationCommand":{"token":"next-page"}}}}}
            ]}}]}
          }}}]}}
        };</script>"#;
        let data: serde_json::Value =
            serde_json::from_str(youtube_initial_data(html).unwrap()).unwrap();
        let (entries, continuation) = initial_playlist_entries(&data).unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.video_id.as_str())
                .collect::<Vec<_>>(),
            [
                "AAAAAAAAAAA",
                "BBBBBBBBBBB",
                "CCCCCCCCCCC",
                "DDDDDDDDDDD",
                "EEEEEEEEEEE",
                "FFFFFFFFFFF"
            ]
        );
        assert_eq!(entries[0].title, "brace } and escaped \" quote");
        assert_eq!(entries[0].author, "Creator A");
        assert_eq!(continuation.as_deref(), Some("next-page"));
    }

    #[test]
    fn rejects_playlist_data_outside_the_playlist_item_section() {
        let html = r#"<script>var ytInitialData = {
          "contents":{"twoColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{
            "sectionListRenderer":{"contents":[{"otherRenderer":{"contents":[
              {"lockupViewModel":{"contentId":"AAAAAAAAAAA"}}
            ]}}]}
          }}}]}}
        };</script>"#;
        let data: serde_json::Value =
            serde_json::from_str(youtube_initial_data(html).unwrap()).unwrap();
        assert!(initial_playlist_entries(&data).unwrap().0.is_empty());
    }

    #[test]
    fn extracts_current_youtube_continuation_page() {
        let data = serde_json::json!({
            "onResponseReceivedActions": [{
                "appendContinuationItemsAction": {"continuationItems": [
                    {"lockupViewModel": {"contentId": "GGGGGGGGGGG"}},
                    {"continuationItemViewModel": {"continuationCommand": {
                        "innertubeCommand": {"continuationCommand": {"token": "more"}}
                    }}}
                ]}
            }]
        });
        let (entries, continuation) = continuation_playlist_entries(&data).unwrap();
        assert_eq!(entries[0].video_id, "GGGGGGGGGGG");
        assert_eq!(continuation.as_deref(), Some("more"));
    }

    #[test]
    fn parses_snapchat_spotlight_links() {
        let id = "W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ";
        assert_eq!(
            snapchat_spotlight_id(&format!(
                "https://www.snapchat.com/spotlight/{id}?share_id=test&locale=en-CA"
            )),
            Some(id.to_owned())
        );
        assert_eq!(
            snapchat_spotlight_id(&format!(
                "https://www.snapchat.com/@maja_karolczak/spotlight/{id}"
            )),
            Some(id.to_owned())
        );
        assert_eq!(
            snapchat_spotlight_id(&format!("https://example.com/spotlight/{id}")),
            None
        );
    }

    #[test]
    fn parses_snapchat_open_graph_metadata() {
        let id = "W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ";
        let final_url =
            Url::parse(&format!("https://www.snapchat.com/@creator/spotlight/{id}")).unwrap();
        let html = r#"<meta content="Stats | Floor Routine | Creator | Spotlight" property="og:title">
            <meta property="og:video" content="https://bolt-gcdn.sc-cdn.net/v/video?x=1&amp;y=2">
            <meta property="og:video:width" content="540">
            <meta property="og:video:height" content="960">"#;
        let candidate = snapchat_candidate_from_html(
            id,
            &format!("https://www.snapchat.com/spotlight/{id}"),
            &final_url,
            html,
        )
        .unwrap();
        assert_eq!(candidate.text, "Floor Routine");
        assert_eq!(candidate.author, "@creator · Snapchat");
        assert_eq!(candidate.resolved.quality_height, Some(540));
        assert_eq!(
            candidate.resolved.media_url,
            "https://bolt-gcdn.sc-cdn.net/v/video?x=1&y=2"
        );
    }

    #[test]
    fn x_quality_variants_are_sorted_and_labeled() {
        let video = V2Video {
            url: "https://video.example/fallback.mp4".to_owned(),
            formats: vec![
                V2Format {
                    url: "https://video.example/low.mp4".to_owned(),
                    container: Some("mp4".to_owned()),
                    bitrate: 256_000,
                },
                V2Format {
                    url: "https://video.example/high.mp4".to_owned(),
                    container: Some("mp4".to_owned()),
                    bitrate: 2_500_000,
                },
                V2Format {
                    url: "https://video.example/mid.mp4".to_owned(),
                    container: Some("mp4".to_owned()),
                    bitrate: 900_000,
                },
            ],
        };
        let qualities = quality_variants_for_x(&video, "123-1.mp4");
        assert_eq!(qualities.len(), 4);
        assert_eq!(qualities[0].media_url, "https://video.example/high.mp4");
        assert!(
            qualities[0]
                .quality_label
                .as_deref()
                .is_some_and(|label| label.starts_with("Best"))
        );
        assert!(
            qualities[2]
                .quality_label
                .as_deref()
                .is_some_and(|label| label.starts_with("Data saver"))
        );
        assert_eq!(qualities[3].filename, "123-1.m4a");
        assert_eq!(
            qualities[3].quality_label.as_deref(),
            Some("Audio only · M4A")
        );
        assert!(qualities[3].extract_audio);
    }

    #[test]
    fn old_queue_entries_default_missing_quality_fields() {
        let job: DownloadJob =
            serde_json::from_str(r#"{"phase":"Paused","downloaded":10,"total":null,"error":null}"#)
                .expect("deserialize old queue entry");
        assert_eq!(job.quality_label, None);
        assert_eq!(job.quality_height, None);
        assert!(!job.extract_audio);
    }

    #[test]
    fn deserializes_nullable_thread_results() {
        let response: V2ThreadResponse =
            serde_json::from_str(r#"{"code":200,"status":null,"thread":null,"author":null}"#)
                .expect("parse thread response");
        assert_eq!(response.code, 200);
        assert!(response.thread.is_none());
    }

    #[test]
    fn detects_same_content_under_different_video_names() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rustdl-duplicate-test-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create duplicate test directory");
        fs::write(directory.join("111-1.mp4"), b"same video bytes").expect("write first video");
        fs::write(directory.join("222-1.mp4"), b"same video bytes").expect("write second video");
        let snapshot = storage_snapshot(&directory).expect("scan storage");
        assert_eq!(snapshot.videos.len(), 2);
        assert!(snapshot.videos.iter().all(|video| video.duplicate));
        let fingerprints = load_fingerprints(&directory).expect("load cached fingerprints");
        assert_eq!(fingerprints.len(), 2);
        assert_eq!(
            fingerprints["111-1.mp4"].blake3,
            fingerprints["222-1.mp4"].blake3
        );
        fs::remove_file(directory.join("111-1.mp4")).expect("remove first video");
        fs::remove_file(directory.join("222-1.mp4")).expect("remove second video");
        fs::remove_file(fingerprints_path(&directory)).expect("remove fingerprint cache");
        fs::remove_dir(directory).expect("remove duplicate test directory");
    }

    #[test]
    fn immutable_ui_assets_are_content_hashed() {
        assert!(playback_script_path().starts_with("/__app/playback."));
        assert!(view_transition_script_path().starts_with("/__app/view-transitions."));
        assert!(index_css_path().starts_with("/__app/index."));
        assert!(player_css_path().starts_with("/__app/player."));
        assert!(index_html_template().contains(index_css_path()));
        assert!(!index_html_template().contains("<style>"));
    }

    #[test]
    fn runtime_tuning_balances_speed_and_phone_pressure() {
        let original = runtime_tuning();
        set_runtime_tuning(true, true, false, 0, 8 * 1024 * 1024 * 1024, 8);
        assert_eq!(adaptive_download_limit(), 3);
        assert_eq!(adaptive_download_buffer_bytes(), 512 * 1024);
        set_runtime_tuning(false, false, true, 4, 512 * 1024 * 1024, 8);
        assert_eq!(adaptive_download_limit(), 1);
        assert_eq!(adaptive_download_buffer_bytes(), 64 * 1024);
        set_runtime_tuning(
            original.unmetered,
            original.charging,
            original.power_save,
            original.thermal_status,
            original.free_bytes,
            original.processors,
        );
    }

    #[test]
    fn rejects_non_x_hosts() {
        assert_eq!(
            status_id_from_url("https://example.com/u/status/2091257067264733401"),
            None
        );
    }

    #[test]
    fn escapes_paths_for_result_page() {
        assert_eq!(escape_html("a&<b>'\""), "a&amp;&lt;b&gt;&#39;&quot;");
    }

    #[test]
    fn validates_generated_video_filenames() {
        assert!(valid_video_filename("2091257067264733401-1.mp4"));
        assert!(valid_video_filename("youtube-xw13xAOyZTw.mp4"));
        assert!(valid_video_filename("youtube-xw13xAOyZTw.m4a"));
        assert!(valid_video_filename(
            "snapchat-W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ.mp4"
        ));
        assert!(valid_video_filename(
            "snapchat-W7_EDlXWTBiXAEEniNoMPwAAYbXV2bWRudHNxAaAtxClKAaAtwqIbAAAAAQ.m4a"
        ));
        assert!(is_audio_filename("2091257067264733401-1.m4a"));
        assert_eq!(media_content_type("2091257067264733401-1.m4a"), "audio/mp4");
        assert!(!valid_video_filename("../secret.mp4"));
        assert!(!valid_video_filename("2091257067264733401-0.mp4"));
    }

    #[test]
    fn parses_browser_byte_ranges() {
        assert_eq!(parse_byte_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_byte_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_byte_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_byte_range("bytes=100-", 100), None);
        assert_eq!(parse_byte_range("bytes=0-1,4-5", 100), None);
    }

    #[test]
    fn gallery_cards_open_one_player_without_embedding_videos() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rustdl-gallery-test-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create gallery test directory");
        let filename = "2091257067264733401-1.mp4";
        let path = directory.join(filename);
        fs::write(&path, b"synthetic test byte").expect("create gallery test video");

        let html = render_index(&directory).expect("render gallery");
        assert!(!html.contains("<video"));
        assert!(html.contains(&format!(r#"href="/watch/{filename}""#)));
        assert!(html.contains(&format!(r#"src="/thumbnail/{filename}.jpg""#)));
        assert!(html.contains(index_css_path()));
        assert!(index_css().contains("@view-transition { navigation: auto; }"));
        assert!(html.contains("view-transition-name:video-2091257067264733401-1"));

        fs::remove_file(path).expect("remove gallery test video");
        fs::remove_dir(directory).expect("remove gallery test directory");
    }

    #[test]
    fn playback_enhancements_cover_resume_seek_speed_and_pip() {
        assert!(PLAYBACK_SCRIPT.contains("savePosition"));
        assert!(PLAYBACK_SCRIPT.contains("dblclick"));
        assert!(PLAYBACK_SCRIPT.contains("playbackRate"));
        assert!(PLAYBACK_SCRIPT.contains("enterPictureInPicture"));
        assert!(PLAYBACK_SCRIPT.contains("Continue watching"));
        assert!(PLAYER_CSS.contains("body.pip"));
    }

    #[test]
    fn modern_controls_cover_anchor_scrubbing_media_session_and_queue() {
        assert!(PLAYER_CSS.contains("position-anchor: --speed-control"));
        assert!(PLAYER_CSS.contains(".control-island"));
        assert!(PLAYER_CSS.contains("position: relative; z-index: 8"));
        assert!(
            PLAYER_CSS.contains(".player-frame:fullscreen .control-island { position: absolute")
        );
        assert!(!PLAYER_CSS.contains(".player-frame.controls-idle:not(:focus-within)"));
        assert!(PLAYER_CSS.contains(".scrub-preview"));
        assert!(PLAYER_CSS.contains(".download-boundary"));
        assert!(PLAYBACK_SCRIPT.contains("navigator.mediaSession.setActionHandler"));
        assert!(PLAYBACK_SCRIPT.contains("Waiting for download"));
        assert!(PLAYBACK_SCRIPT.contains("event.preventDefault();event.stopPropagation()"));
        assert!(PLAYBACK_SCRIPT.contains("setPointerCapture"));
        assert!(PLAYBACK_SCRIPT.contains("seekFromPointer"));
        assert!(PLAYBACK_SCRIPT.contains("card-popover"));
        assert!(PLAYBACK_SCRIPT.contains("queue-mini"));
        assert!(INDEX_HTML.contains("animation-timeline: view()"));
    }

    #[test]
    fn changelog_covers_every_version_and_marks_the_current_release() {
        assert_eq!(CHANGELOG.len(), 32);
        for (index, (version, changes)) in CHANGELOG.iter().rev().enumerate() {
            assert_eq!(*version, format!("0.1.{index}"));
            assert!(!changes.is_empty());
        }
        let html = render_changelog();
        assert!(INDEX_HTML.contains(r#"href="/changelog""#));
        assert!(html.contains("Version 0.1.0"));
        assert!(html.contains(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
        assert_eq!(html.matches(r#"class="current""#).count(), 1);
        assert_eq!(
            html.matches(r#"<option value="version-"#).count(),
            CHANGELOG.len()
        );
        assert_eq!(
            html.matches(r#"<article id="version-"#).count(),
            CHANGELOG.len()
        );
        assert!(html.contains(r#"id="version-jump""#));
        assert!(html.contains(r#"id="version-go""#));
        assert!(html.contains("scrollIntoView"));
        assert!(html.contains("history.replaceState"));
        assert_eq!(html.matches(r#"class="release-actions""#).count(), 30);
        assert!(html.contains("Go to Settings"));
        assert!(html.contains("Go to Diagnostics"));
        assert!(!html.contains(r#"href="/control"#));
        assert!(html.contains(r#"href="/peers""#));
        assert!(html.contains(r#"href="/queue""#));
        assert!(html.contains(r#"href="rustdl://mode/inspection""#));
        assert!(INDEX_HTML.contains(r#"id="downloader""#));
    }

    #[test]
    fn diagnostics_are_live_but_exclude_user_content() {
        let html = render_diagnostics_page();
        let bridge = include_str!("../android/DiagnosticsBridge.java");
        assert!(INDEX_HTML.contains(r#"href="/diagnostics""#));
        assert!(html.contains("RustDLDiagnostics"));
        assert!(html.contains("bridge.diagnostics()"));
        assert!(html.contains("response.detail"));
        assert!(html.contains("Load average restricted by Android"));
        assert!(html.contains("copySnapshot"));
        assert!(html.contains(r#"aria-busy="true""#));
        assert!(html.contains("memoryAvailableBytes"));
        assert!(html.contains("Battery sensor"));
        assert!(!html.contains("companionPid"));
        assert!(!html.contains("navigator.clipboard"));
        assert!(bridge.contains("availableSources"));
        assert!(bridge.contains("memoryState()"));
        assert!(bridge.contains("storageState()"));
        assert!(bridge.contains("batteryState()"));
        assert!(bridge.contains("copySnapshot"));
        assert!(html.contains("Privacy boundary"));
        assert!(html.contains("media filenames"));
        assert!(!html.contains("logcat"));
        assert!(!html.contains("Wi-Fi SSID"));
        assert!(!html.contains("/media/"));
        assert!(!html.contains("savedVideos"));
    }

    #[test]
    fn peer_addresses_are_restricted_to_local_ipv4() {
        assert_eq!(
            peer_base_url("192.168.50.12:37660").unwrap().as_str(),
            "http://192.168.50.12:37660/"
        );
        assert!(peer_base_url("127.0.0.1:18092").is_ok());
        assert!(peer_base_url("8.8.8.8:37660").is_err());
        assert!(peer_base_url("example.com:37660").is_err());
        assert!(peer_base_url("user@192.168.1.2:37660").is_err());
    }

    #[test]
    fn peer_pairing_keys_round_trip_hex() {
        let key = [0xabu8; 32];
        let encoded = hex_encode(&key);
        assert_eq!(encoded.len(), 64);
        assert_eq!(decode_peer_key(&encoded).unwrap(), key);
        assert!(decode_peer_key("short").is_err());
        assert!(decode_peer_key(&"z".repeat(64)).is_err());
    }

    #[test]
    fn qr_pairing_is_local_and_keeps_payload_out_of_svg() {
        let key = "ab".repeat(32);
        let payload = format!("rustdl://pair?address=192.168.50.12%3A37660&key={key}");
        let svg = render_pairing_qr(&payload).expect("render pairing QR");
        assert!(svg.starts_with(r#"<svg class="pairing-qr""#));
        assert!(svg.contains("<path d=\"M"));
        assert!(!svg.contains(&key));

        set_outbound_peer_pairing("192.168.50.12:37660", &key).expect("store valid local pairing");
        let pairing = current_outbound_peer_pairing().expect("active pairing");
        assert_eq!(pairing.address, "192.168.50.12:37660");
        assert_eq!(pairing.key, [0xab; 32]);
        assert!(set_outbound_peer_pairing("8.8.8.8:37660", &key).is_err());
    }

    #[test]
    fn pairing_refresh_keeps_geometry_and_swaps_only_stable_fields() {
        assert!(PEER_CSS.contains("min-height:344px"));
        assert!(PEER_CSS.contains("aspect-ratio:1"));
        assert!(PEER_CSS.contains("view-transition-name:pairing-code"));
        assert!(PEER_CSS.contains(":root{view-transition-name:none}"));
        assert!(PEER_CSS.contains("animation-duration:100ms"));
        assert!(!PEER_CSS.contains("opacity:.55"));
        assert!(PEER_PAIRING_SCRIPT.contains("event.preventDefault()"));
        assert!(PEER_PAIRING_SCRIPT.contains("fetch('/peers/refresh',{cache:'no-store'})"));
        assert!(PEER_PAIRING_SCRIPT.contains("response.json()"));
        assert!(PEER_PAIRING_SCRIPT.contains("document.startViewTransition(swap)"));
        assert!(PEER_PAIRING_SCRIPT.contains("outerHTML=next.qr"));
        assert!(!PEER_PAIRING_SCRIPT.contains("DOMParser"));
        assert!(!PEER_PAIRING_SCRIPT.contains("location.reload"));
    }

    #[test]
    fn playlist_membership_persists_and_renders_as_one_gallery_folder() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rustdl-playlist-folder-test-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create playlist folder test directory");
        let first = "youtube-AAAAAAAAAAA.mp4";
        let second = "youtube-BBBBBBBBBBB.mp4";
        fs::write(directory.join(first), b"first synthetic media").unwrap();
        fs::write(directory.join(second), b"second synthetic media").unwrap();
        let playlist_id = "PL1234567890_test";
        for (filename, position) in [(first, 2), (second, 1)] {
            record_playlist_membership(
                &directory,
                filename,
                PlaylistMembership {
                    playlist_id: playlist_id.to_owned(),
                    title: "Synthetic road trip".to_owned(),
                    position,
                    total: 8,
                },
            )
            .unwrap();
        }

        let root = render_index(&directory).unwrap();
        assert!(root.contains(&format!(r#"href="/gallery/playlist/{playlist_id}""#)));
        assert!(root.contains("Synthetic road trip"));
        assert!(root.contains("2 saved of 8"));
        assert!(!root.contains(&format!(r#"href="/watch/{first}""#)));

        let folder = render_index_view(&directory, Some(playlist_id)).unwrap();
        let second_position = folder.find(&format!(r#"href="/watch/{second}""#)).unwrap();
        let first_position = folder.find(&format!(r#"href="/watch/{first}""#)).unwrap();
        assert!(second_position < first_position);
        assert!(folder.contains("← All media"));

        fs::remove_file(directory.join(first)).unwrap();
        fs::remove_file(directory.join(second)).unwrap();
        fs::remove_file(playlist_memberships_path(&directory)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn reads_playlist_title_from_youtube_metadata() {
        let data = serde_json::json!({
            "metadata": {"playlistMetadataRenderer": {"title": "Rust playlist"}}
        });
        assert_eq!(
            youtube_playlist_title(&data).as_deref(),
            Some("Rust playlist")
        );
    }

    #[test]
    fn bulk_quality_options_expose_file_type_and_resolution() {
        let video = ResolvedVideo {
            filename: "youtube-AAAAAAAAAAA.mp4".to_owned(),
            media_url: "https://example.invalid/video".to_owned(),
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Balanced · 720p".to_owned()),
            quality_height: Some(720),
        };
        let audio = ResolvedVideo {
            filename: "youtube-AAAAAAAAAAA.m4a".to_owned(),
            quality_label: Some("Audio only · M4A".to_owned()),
            ..video.clone()
        };
        let video_option = render_quality_option("token", 2, 0, &video);
        let audio_option = render_quality_option("token", 2, 1, &audio);
        assert!(video_option.contains(r#"data-kind="video" data-height="720""#));
        assert!(audio_option.contains(r#"data-kind="audio""#));
        assert!(BULK_QUALITY_SCRIPT.contains("below[0]||above[0]||videos[0]"));
        assert!(BULK_QUALITY_SCRIPT.contains("value==='audio'"));
    }

    #[test]
    fn gallery_search_and_filters_are_wired_to_media_and_folders() {
        assert!(INDEX_HTML.contains(".gallery-tools"));
        assert!(PLAYBACK_SCRIPT.contains("syncGalleryFilter"));
        assert!(PLAYBACK_SCRIPT.contains("filter==='playlists'&&folder"));
        assert!(PLAYBACK_SCRIPT.contains("filter==='audio'&&audio"));
        assert!(PLAYBACK_SCRIPT.contains("filter==='downloading'&&downloading"));
        assert!(PLAYBACK_SCRIPT.contains("toLocaleLowerCase().includes(query)"));
    }

    #[test]
    fn peer_resume_metadata_resets_only_for_different_content() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rustdl-peer-test-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let filename = "youtube-AAAAAAAAAAA.mp4";
        let manifest = PeerManifest {
            filename: filename.to_owned(),
            size: 8,
            hash: "a".repeat(64),
        };
        assert_eq!(
            prepare_peer_receive(&directory, &manifest).unwrap().offset,
            0
        );
        fs::write(peer_part_path(&directory, filename), b"part").unwrap();
        assert_eq!(
            prepare_peer_receive(&directory, &manifest).unwrap().offset,
            4
        );
        let changed = PeerManifest {
            hash: "b".repeat(64),
            ..manifest
        };
        assert_eq!(
            prepare_peer_receive(&directory, &changed).unwrap().offset,
            0
        );
        remove_if_exists(&peer_part_path(&directory, filename)).unwrap();
        remove_if_exists(&peer_manifest_path(&directory, filename)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn app_state_uses_stable_download_phase_names() {
        assert_eq!(download_phase_name(DownloadPhase::Queued), "queued");
        assert_eq!(
            download_phase_name(DownloadPhase::Downloading),
            "downloading"
        );
        assert_eq!(download_phase_name(DownloadPhase::Ready), "ready");
        let job = DownloadJob {
            phase: DownloadPhase::Downloading,
            downloaded: 25,
            total: Some(100),
            error: None,
            source_url: Some("https://x.com/example/status/123".to_owned()),
            media_url: None,
            audio_url: None,
            extract_audio: false,
            quality_label: Some("Balanced · 720p".to_owned()),
            quality_height: Some(720),
        };
        let state = app_state_job("123-1.mp4", &job);
        assert_eq!(state["phase"], "downloading");
        assert_eq!(state["downloaded"], 25);
        assert_eq!(state["quality"], "Balanced · 720p");
        assert!(state.get("media_url").is_none());
    }

    #[test]
    fn view_transitions_feature_detect_and_select_one_shared_thumbnail() {
        assert!(VIEW_TRANSITION_SCRIPT.contains("document.startViewTransition"));
        assert!(VIEW_TRANSITION_SCRIPT.contains("selectSharedElement"));
        assert!(VIEW_TRANSITION_SCRIPT.contains("style.viewTransitionName='none'"));
        assert!(PLAYBACK_SCRIPT.contains("dataset.viewTransitionName"));
    }

    #[test]
    fn creates_safe_transition_names() {
        assert_eq!(
            view_transition_name("2091257067264733401-1.mp4"),
            "video-2091257067264733401-1"
        );
        assert_eq!(view_transition_name("odd name.mp4"), "video-odd-name");
    }

    #[test]
    fn reads_bytes_as_a_download_grows() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rustdl-growing-test-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create test directory");
        let filename = format!("{}-1.mp4", std::process::id());
        let output = directory.join(&filename);
        let temporary = part_path(&output);
        File::create(&temporary).expect("create partial file");
        set_download_job(
            &filename,
            DownloadJob {
                phase: DownloadPhase::Downloading,
                downloaded: 0,
                total: Some(11),
                error: None,
                source_url: None,
                media_url: None,
                audio_url: None,
                extract_audio: false,
                quality_label: None,
                quality_height: None,
            },
        );

        let writer_filename = filename.clone();
        let writer_path = temporary.clone();
        let writer = thread::spawn(move || {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(writer_path)
                .expect("open partial file for append");
            file.write_all(b"hello").expect("write first chunk");
            file.flush().expect("flush first chunk");
            update_download_progress(&writer_filename, 5, Some(11));
            thread::sleep(Duration::from_millis(100));
            file.write_all(b" world").expect("write second chunk");
            file.flush().expect("flush second chunk");
            set_download_job(
                &writer_filename,
                DownloadJob {
                    phase: DownloadPhase::Ready,
                    downloaded: 11,
                    total: Some(11),
                    error: None,
                    source_url: None,
                    media_url: None,
                    audio_url: None,
                    extract_audio: false,
                    quality_label: None,
                    quality_height: None,
                },
            );
        });

        let mut reader =
            GrowingFile::open(&directory, &filename, 0, Some(10)).expect("open growing reader");
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).expect("read growing file");
        writer.join().expect("join writer");
        assert_eq!(bytes, b"hello world");

        download_jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&filename);
        fs::remove_file(temporary).expect("remove partial file");
        fs::remove_dir(directory).expect("remove test directory");
    }
}
