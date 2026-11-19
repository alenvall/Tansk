// Static preview harness for the TANSK UI partials (not shipped in the game:
// the game inserts the same partials from Rust and drives them through
// src/ui/mod.rs). Serve the repo root over http and open
// crates/arena-client/ui/preview.html or title.html; ?state= picks a scene.
(async function () {
  const root = document.getElementById('tansk-ui');
  const load = async (f) => (await fetch(f)).text();
  try {
    root.innerHTML = (await load('menus.html')) + (await load('hud.html'));
  } catch (e) {
    root.innerHTML = '<p style="padding:2rem;color:#fff;pointer-events:auto">Serve this folder over http (fetch of the partials failed): ' + e + '</p>';
    return;
  }
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const roman = (n) => ['-', 'I', 'II', 'III'][n] || String(n);
  const state = new URLSearchParams(location.search).get('state') || document.body.dataset.state || 'combat';

  // --- menu wiring (the Rust bridge does the same with data-goto) ---------
  const title = $('t-title');
  root.addEventListener('click', (e) => {
    const g = e.target.closest('[data-goto]');
    if (g) title.dataset.panel = g.dataset.goto;
  });
  root.addEventListener('input', (e) => {
    const out = e.target.parentElement && e.target.parentElement.querySelector('output');
    if (out && e.target.type === 'range') out.textContent = e.target.dataset.setting === 'mouse_sens' ? (e.target.value / 100).toFixed(2) : e.target.value;
  });

  // --- the same operations as src/ui/mod.rs --------------------------------
  const ui = {
    hud() { title.hidden = true; $('t-loading').hidden = true; $('t-hud').hidden = false; },
    health(v) { $('t-hp').style.setProperty('--v', v); $('t-hp-num').textContent = Math.round(v * 100); $('t-hp').dataset.low = v < .3 ? '1' : '0'; $('t-vignette').style.setProperty('--low', Math.max(0, (0.5 - v) / 0.5).toFixed(2)); },
    armour(v) { $('t-armour').style.setProperty('--v', v); },
    boost(v) { $('t-boost').style.setProperty('--v', v); },
    track(side, st, t) { $('t-track-' + side).dataset.state = st; $('t-track-' + side + '-t').textContent = t ? t.toFixed(1) + 's' : ''; },
    scrap(i, n, tier, part) { const r = $('t-scrap-' + i); r.dataset.n = n; r.dataset.tier = tier; $('t-tier-' + i).textContent = roman(tier); $('t-part-' + i).textContent = part; },
    reticle(spread, reload) { const r = $('t-reticle'); r.style.setProperty('--spread', spread); r.style.setProperty('--reload', reload); r.dataset.ready = reload >= 1 ? '1' : '0'; },
    hit(kind) { $('t-hit').dataset.kind = kind; },
    dmg(slot, deg) { const w = $('t-dmg-' + slot); w.style.setProperty('--a', deg + 'deg'); w.dataset.on = '1'; },
    timer(s) { $('t-timer').textContent = s; },
    scores(a, b) { $('t-score-a').textContent = a; $('t-score-b').textContent = b; },
    feed(k, kt, w, v, vt, you) {
      const d = document.createElement('div'); d.className = 't-kf'; if (you) d.dataset.you = '1';
      d.innerHTML = `<span class="t-name-${kt}">${esc(k)}</span><span class="t-kf-weapon">${esc(w)}</span><span class="t-name-${vt}">${esc(v)}</span>`;
      $('t-feed').appendChild(d);
    },
    radio(t) { $('t-radio-text').textContent = t; $('t-radio').dataset.on = '1'; },
    chat(name, team, text, sys) {
      const d = document.createElement('div'); d.className = 't-chat-line'; if (sys) d.dataset.sys = '1';
      d.innerHTML = sys ? esc(text) : `<b class="t-name-${team}">${esc(name)}:</b> ${esc(text)}`;
      $('t-chat-log').appendChild(d);
    },
    branch(titleText, opts, def) {
      $('t-branch-title').textContent = titleText;
      $('t-branch-cards').innerHTML = opts.map((o, i) => `<button class="t-branch-card" data-action="branch" data-i="${i}" style="--k:var(--${o.kind})">
        <span class="t-bc-top"><span class="t-bc-key">${i + 1}</span><svg class="t-bc-ic"><use href="#i-${o.kind}"/></svg></span>
        ${i === def ? '<span class="t-bc-def">Default</span>' : ''}
        <span class="t-bc-name">${esc(o.name)}</span><span class="t-bc-blurb">${esc(o.blurb)}</span></button>`).join('');
      $('t-branch').dataset.on = '1';
    },
    board(rowsA, rowsB, title, sub) {
      const col = (team, name, score, rows) => `<div class="t-bt" data-team="${team}"><div class="t-bt-head"><span>${esc(name)}</span><b>${score}</b></div>
        <div class="t-br t-br-h"><span>Crew</span><span>K</span><span>D</span><span>Tiers</span><span>Ping</span></div>
        ${rows.map((r) => `<div class="t-br"${r.you ? ' data-you="1"' : ''}${r.dead ? ' data-dead="1"' : ''}><span class="t-br-name">${esc(r.name)}${r.bot ? '<em>BOT</em>' : ''}</span><span>${r.k}</span><span>${r.d}</span><span class="t-br-tiers">${r.t.map((t) => `<i>${t}</i>`).join('')}</span><span>${r.bot ? '-' : r.p}</span></div>`).join('')}</div>`;
      $('t-board-title').textContent = title; $('t-board-sub').textContent = sub;
      $('t-board-body').innerHTML = col('a', 'Rustbacks', 14, rowsA) + col('b', 'Dust Saints', 11, rowsB);
      $('t-board').hidden = false;
    },
    respawn(secs, by) { $('t-respawn-secs').textContent = secs; $('t-respawn-by').innerHTML = by; $('t-respawn').hidden = false; },
    end(titleText, team, rows) {
      $('t-end-title').textContent = titleText; $('t-end-title').dataset.team = team;
      $('t-end-rows').innerHTML = rows.map((r, i) => `<div class="t-ec plate" data-team="${r.team}"><div class="t-ec-rank">#${i + 1}</div><div class="t-ec-name">${esc(r.name)}</div>
        <div class="t-ec-stat"><span><b>${r.k}</b> kills</span><span><b>${r.d}</b> deaths</span></div>${r.tag ? `<span class="t-ec-tag">${esc(r.tag)}</span>` : ''}</div>`).join('');
      $('t-end').hidden = false;
    },
  };

  const A = [
    { name: 'Grit', k: 7, d: 2, t: [2, 3, 1, 1], p: 41, you: true },
    { name: 'Old Mags', k: 4, d: 3, t: [1, 2, 2, 1], p: 38 },
    { name: 'Socket', k: 2, d: 4, t: [3, 1, 1, 2], p: 64, dead: true },
    { name: 'Cinder', k: 1, d: 3, t: [1, 1, 1, 2], p: 0, bot: true },
  ];
  const B = [
    { name: 'Mirage', k: 6, d: 3, t: [1, 2, 3, 1], p: 52 },
    { name: 'Dustwife', k: 3, d: 4, t: [2, 1, 1, 3], p: 47 },
    { name: 'Halt', k: 1, d: 5, t: [1, 1, 2, 1], p: 0, bot: true },
    { name: 'Psalm', k: 1, d: 2, t: [1, 1, 1, 1], p: 88 },
  ];

  const combat = () => {
    ui.hud();
    $('t-hud').dataset.demo = '1';
    ui.health(0.27); ui.armour(0.4); ui.boost(0.62);
    ui.track('l', 'ok'); ui.track('r', 'blown', 3.4);
    ui.scrap(0, 1, 1, 'Flank plates'); ui.scrap(1, 3, 2, 'Heavy cannon'); ui.scrap(2, 2, 1, 'Sensor mast'); ui.scrap(3, 0, 1, 'Nitro tank');
    ui.reticle(0.45, 0.62); ui.hit('part'); ui.dmg(0, -65); ui.dmg(1, 160);
    ui.timer('06:42'); ui.scores(14, 11);
    ui.feed('Mirage', 'b', 'Big Bore', 'Socket', 'a');
    ui.feed('Grit', 'a', 'Heavy cannon', 'Halt', 'b', true);
    ui.feed('Old Mags', 'a', 'Rammed', 'Psalm', 'b');
    ui.radio("Big one heading north. Twin barrel, mast up. Shoot the mast, he's blind.");
    ui.chat('', '', 'Dustwife grabbed the golden crate.', true);
    ui.chat('Old Mags', 'a', 'cover me, track is gone');
    ui.chat('Mirage', 'b', 'nice hat, grit');
    ui.branch('Powder tier 2 · pick one', [
      { kind: 'powder', name: 'Big Bore', blurb: 'Huge slow shells. Splash.' },
      { kind: 'powder', name: 'Twin Barrel', blurb: 'Double blast, up close.' },
      { kind: 'powder', name: 'Long Gun', blurb: 'Flat, fast, a deeper scope.' },
    ], 0);
  };

  if (state === 'title') { /* nothing: the title is the default */ }
  else if (['host', 'join', 'settings', 'garage'].includes(state)) title.dataset.panel = state;
  else if (state === 'loading') { title.hidden = true; $('t-loading').hidden = false; $('t-loading-bar').style.setProperty('--p', .55); }
  else if (state === 'combat') combat();
  else if (state === 'hits') {
    combat(); $('t-branch').dataset.on = '0';
    // three reticles side by side are not possible in one HUD: show the kill style
    ui.hit(new URLSearchParams(location.search).get('kind') || 'kill');
  }
  else if (state === 'scoreboard') { combat(); $('t-branch').dataset.on = '0'; ui.board(A, B, 'Team Brawl', 'The Sink · 06:42 left'); }
  else if (state === 'respawn') { combat(); $('t-branch').dataset.on = '0'; $('t-reticle').hidden = true; ui.respawn(4, 'Opened up by <span class="t-name-b">Mirage</span>’s Big Bore'); }
  else if (state === 'end') {
    combat(); $('t-branch').dataset.on = '0'; $('t-radio').dataset.on = '0';
    $('t-hud').dataset.end = '1';
    ui.end('Rustbacks take the Sink', 'a', [
      { name: 'Grit', team: 'a', k: 7, d: 2, tag: 'Top gun' },
      { name: 'Mirage', team: 'b', k: 6, d: 3, tag: 'Biggest build' },
      { name: 'Old Mags', team: 'a', k: 4, d: 3 },
      { name: 'Dustwife', team: 'b', k: 3, d: 4, tag: 'Most scrap' },
      { name: 'Socket', team: 'a', k: 2, d: 4 },
    ]);
  }
  document.body.dataset.ready = '1';
})();
