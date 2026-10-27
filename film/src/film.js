/* LeakBench — film engine.
 *
 * window.seek(t) is a pure function of t: it reads timeline.json and writes
 * styles. No timers, no randomness at draw time, no CSS transitions.
 *
 * Layers: scene (2) < exit bars (6) < mascot (7) < wipe (80), grain at 1.
 */
(() => {
  'use strict';

  const TL = window.__TIMELINE__;
  const META = TL.meta;
  const BEAT = META.beat;
  const BAR = META.bar;
  const FPS = META.fps;
  const DUR = META.duration;
  const HIT = 0.5 / FPS; // a beat counts as reached at this tolerance

  // ---------------------------------------------------------------- math
  const clamp = (x, a = 0, b = 1) => (x < a ? a : x > b ? b : x);
  const lerp = (a, b, k) => a + (b - a) * k;
  const out3 = (k) => 1 - Math.pow(1 - k, 3);
  const in3 = (k) => k * k * k;
  const inout = (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
  const back = (k, s = 1.7) => 1 + (s + 1) * Math.pow(k - 1, 3) + s * Math.pow(k - 1, 2);
  const prog = (t, start, dur) => clamp((t - start) / dur);
  const pulse = (t, c, h) => Math.max(0, 1 - Math.abs(t - c) / h);
  const reached = (t, at) => at <= t + HIT;

  // ---------------------------------------------------------------- dom
  const stage = document.getElementById('stage');
  const sceneLayer = document.getElementById('scenes');
  const exitLayer = document.getElementById('exit');
  const wipeLayer = document.getElementById('wipe');
  const mascotLayer = document.getElementById('mascot');

  function mk(tag, cls, parent, text) {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text !== undefined) n.textContent = text;
    (parent || sceneLayer).appendChild(n);
    return n;
  }

  function css(node, styles) {
    for (const k in styles) node.style[k] = styles[k];
    return node;
  }

  // ---------------------------------------------------------------- events
  const byScene = new Map();
  for (const e of TL.events) {
    const key = e.scene || '';
    if (!byScene.has(key)) byScene.set(key, []);
    byScene.get(key).push(e);
  }
  function evs(sceneId, kind) {
    const list = byScene.get(sceneId) || [];
    return kind ? list.filter((e) => e.kind === kind).sort((a, b) => a.t - b.t) : list;
  }

  // ---------------------------------------------------------------- text
  function textBlock(parent, { text, cls = 'words', size = 64, left = 160, top = 0, color, width }) {
    const n = mk('div', 't ' + cls, parent, text);
    n.style.fontSize = size + 'px';
    n.style.left = left + 'px';
    n.style.top = top + 'px';
    if (color) n.style.color = color;
    if (width) n.style.width = width + 'px';
    return n;
  }

  /** TEXT IN — 0.2 s slam. */
  function slamIn(node, t, t0, dur = 0.2) {
    const k = prog(t, t0, dur);
    node.style.opacity = String(clamp(5 * k));
    node.style.transform = `translateY(${lerp(10, 0, out3(k))}px) scale(${lerp(1.08, 1, back(k))})`;
  }

  /** Measure the rendered line boxes of an element (one per <br>-split line). */
  function lineBoxes(node) {
    const boxes = [];
    const range = document.createRange();
    const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
    let current = null;
    let textNode;
    while ((textNode = walker.nextNode())) {
      range.selectNodeContents(textNode);
      const rects = range.getClientRects();
      for (const r of rects) {
        if (!current || Math.abs(r.top - current.top) > 2) {
          if (current) boxes.push(current);
          current = { left: r.left, right: r.right, top: r.top, height: r.height };
        } else {
          current.left = Math.min(current.left, r.left);
          current.right = Math.max(current.right, r.right);
          current.height = Math.max(current.height, r.height);
        }
      }
    }
    if (current) boxes.push(current);
    return boxes;
  }

  /**
   * TEXT OUT — one bar per line, 72% of the line, 5% below its middle, 10 px
   * past each end. Line i starts at t0+0.03i, grows 0.11 (out3), holds, then
   * retracts to the right from +0.19 over 0.13 (in3).
   */
  function makeRedactor(parent, node, t0) {
    const bars = [];
    for (const box of lineBoxes(node)) {
      const h = box.height * 0.72;
      const bar = mk('div', 'bar', parent);
      css(bar, {
        left: box.left - 10 + 'px',
        top: box.top + box.height / 2 + box.height * 0.05 - h / 2 + 'px',
        width: box.right - box.left + 20 + 'px',
        height: h + 'px',
      });
      bars.push(bar);
    }
    return function draw(t) {
      let allGrown = bars.length > 0;
      bars.forEach((bar, i) => {
        const start = t0 + 0.03 * i;
        const grow = prog(t, start, 0.11);
        const retract = prog(t, start + 0.19, 0.13);
        let sx = out3(grow);
        let origin = '0 50%';
        if (retract > 0) {
          sx = 1 - in3(retract);
          origin = '100% 50%';
        }
        bar.style.transformOrigin = origin;
        bar.style.transform = `scaleX(${grow <= 0 ? 0 : sx})`;
        if (grow < 1) allGrown = false;
      });
      node.style.opacity = allGrown && t > t0 + 0.19 ? '0' : '1';
    };
  }

  // ---------------------------------------------------------------- sheets
  function sheet(parent, { x, y, w, h, dark }) {
    const el = mk('div', 'sheet' + (dark ? ' dark' : ''), parent);
    css(el, { left: x + 'px', top: y + 'px', width: w + 'px', height: h + 'px' });
    return el;
  }

  /** SHEET IN — rises 660–760 px, tilt settles, over 0.42 (out3). */
  function sheetIn(el, t, t0, { from = 700, dx = 0, tilt = 4, screen = false } = {}) {
    const k = out3(prog(t, t0, 0.42));
    const rot = screen ? lerp(2.2 * tilt, 0.5 * tilt, k) : lerp(tilt, 0, k);
    el.style.transform = `translate(${lerp(dx * 1.0, 0, k)}px, ${lerp(from, 0, k)}px) rotate(${rot}deg)`;
    el.style.opacity = '1';
  }

  /** SHEET OUT — in3 over 0.32, 900 px down, -5°. */
  function sheetOut(el, t, t0) {
    if (!t0) return;
    const k = in3(prog(t, t0, 0.32));
    if (k <= 0) return;
    el.style.transform = `translateY(${900 * k}px) rotate(${-5 * k}deg)`;
  }

  // ---------------------------------------------------------------- stamp
  function stamp(parent, cfg) {
    const el = mk('div', 'stamp ' + (cfg.ok ? 'yes' : 'no'), parent);
    el.innerHTML = `<div class="word">${cfg.text}</div>`;
    el.style.fontSize = (cfg.size || 88) + 'px';
    return el;
  }
  function drawStamp(el, t, t0, rot) {
    const k = prog(t, t0, 0.11);
    el.style.opacity = String(clamp(6 * k));
    el.style.transform = `rotate(${rot}deg) scale(${lerp(1.9, 1, out3(k))})`;
  }

  // ---------------------------------------------------------------- camera
  function cameraBox(a, b, k) {
    return { x: lerp(a.x, b.x, k), y: lerp(a.y, b.y, k), w: lerp(a.w, b.w, k) };
  }

  // =======================================================================
  //  templates
  // =======================================================================
  const BUILD = {};

  BUILD.title = (s) => {
    const el = mk('div', 'scene');
    const eyebrow = textBlock(el, { text: s.eyebrow, cls: 'eyebrow', size: 19, top: 170 });
    const lines = s.lines.map((l) =>
      textBlock(el, { text: l.text, cls: 'display', size: l.size, top: l.top }),
    );
    const sub = s.sub
      ? textBlock(el, {
          text: s.sub.text,
          cls: s.sub.mono ? 'data' : 'words',
          size: s.sub.mono ? 30 : 40,
          top: s.sub.top,
          color: 'var(--muted)',
        })
      : null;
    const drawers = lines.map((node) => ({ node, fn: null }));
    return {
      el,
      layout() {},
      draw(t) {
        slamIn(eyebrow, t, s.eyebrow_t ?? s.start);
        lines.forEach((node, i) => slamIn(node, t, s.lines[i].t));
        if (sub) slamIn(sub, t, s.sub.t);
        if (s.out) {
          // TEXT OUT: each line gets its own bar set, lazily, at t0 + 0.03 i.
          drawers.forEach((d, i) => {
            if (!d.fn) d.fn = makeRedactor(el, d.node, s.out + 0.03 * i);
            d.fn(t);
          });
        }
      },
    };
  };

  BUILD.chapter = (s) => {
    const el = mk('div', 'scene');
    const word = textBlock(el, { text: s.word, cls: 'display', size: 300, left: 150, top: 330 });
    word.style.transformOrigin = '0 0';
    word.style.lineHeight = '0.82';

    const card = sheet(el, { x: 830, y: 286, w: 940, h: 330 });
    const rows = s.rows.map((row, i) => {
      const r = mk('div', '', card);
      css(r, { position: 'absolute', left: '36px', right: '36px', top: 52 + i * 84 + 'px', height: '56px' });
      const left = mk('div', 'data', r, row.left);
      css(left, { position: 'absolute', left: 0, top: '14px', fontSize: '25px', color: 'var(--muted)' });
      const right = mk('div', 'data500', r, row.right);
      css(right, { position: 'absolute', right: 0, top: '14px', fontSize: '30px' });
      const rule = mk('div', 'rule-line', r);
      css(rule, { left: 0, right: 0, bottom: 0 });
      return { row: r, left, right, bars: [] };
    });

    const lines = s.lines.map((l) => textBlock(el, { text: l.text, cls: 'words', size: 42, top: l.top }));

    return {
      el,
      layout() {
        // REDACTING DATA — two bars per row, 0.05 apart, 0.09 out3 from left.
        rows.forEach((r) => {
          for (const node of [r.left, r.right]) {
            const b = node.getBoundingClientRect();
            const bar = mk('div', 'bar', exitLayer);
            bar.dataset.scene = s.id;
            css(bar, {
              left: b.left - 6 + 'px',
              top: b.top + b.height * 0.5 - b.height * 0.36 + 'px',
              width: b.width + 12 + 'px',
              height: b.height * 0.72 + 'px',
            });
            r.bars.push(bar);
          }
        });
      },
      draw(t) {
        const c = s.cues;
        slamIn(word, t, c.land);
        const move = inout(prog(t, c.move, 0.42));
        if (move > 0) {
          word.style.transform = `translate(${10 * move}px, ${-262 * move}px) scale(${lerp(1, 0.42, move)})`;
          word.style.fontSize = '300px';
        }
        sheetIn(card, t, c.sheet, { from: 700, tilt: 3.2 });
        rows.forEach((r, i) => {
          const t0 = c.sheet + i * 0.3125;
          const k = prog(t, t0, 0.2);
          // As the bars lift off, the row goes with them: the redaction is not
          // peeled back to reveal the data it was covering.
          const vanish = 1 - in3(prog(t, (c.lift ?? 1e9) + i * 0.3125, 0.2));
          r.row.style.opacity = String(clamp(5 * k) * vanish);
          r.row.style.transform = `translateY(${lerp(10, 0, out3(k))}px)`;
          r.bars.forEach((bar, bi) => {
            const start = c.redact + i * 0.3125 + bi * 0.05;
            const grow = out3(prog(t, start, 0.09));
            // BARS LIFTING OFF ROWS — 26 px up, scaleY to 0, from the top edge.
            const lift = in3(prog(t, (c.lift ?? 1e9) + i * 0.3125, 0.2));
            if (lift > 0) {
              bar.style.transformOrigin = '50% 0%';
              bar.style.transform = `translateY(${-26 * lift}px) scaleY(${1 - lift})`;
            } else {
              bar.style.transformOrigin = '0 50%';
              bar.style.transform = `scaleX(${grow})`;
            }
          });
        });
        lines.forEach((n, i) => slamIn(n, t, s.lines[i].t));
      },
    };
  };

  BUILD.counter = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'display', size: 300, left: 150, top: 250 });
    heading.style.lineHeight = '0.8';
    const count = s.count;
    const c = s.cues;

    const dots = [];
    for (let i = 0; i < count.to; i++) {
      const row = Math.floor(i / count.rows);
      const col = i % count.rows;
      const d = mk('div', 'dot', el);
      css(d, { left: 830 + col * 30 + 'px', top: 560 + row * 30 + 'px', opacity: '0' });
      dots.push(d);
    }
    const tiles = [0, 1, 2].map((i) => {
      const tile = mk('div', 'tile', el);
      css(tile, { left: 830 + 70 * i + 'px', top: 560 + 'px' });
      return tile;
    });

    const lines = s.lines.map((l) => textBlock(el, { text: l.text, cls: 'words', size: 36, top: l.top }));

    return {
      el,
      layout() {},
      draw(t) {
        slamIn(heading, t, c.heading);
        heading.style.opacity = '1';
        heading.style.transform = 'none';

        const k = inout(prog(t, c.count_start, c.count_end - c.count_start));
        const n = Math.round(lerp(count.from, count.to, k));
        heading.textContent = String(n);
        heading.style.fontSize = '400px';
        heading.style.top = '130px';

        const label = el.querySelector('.counter-label');
        if (!label) {
          const l = mk('div', 't words counter-label', el, 'findings');
          css(l, { left: '160px', top: '612px', fontSize: '72px' });
        }

        const shrink = inout(prog(t, c.count_start, 0.32));
        tiles.forEach((tile, i) => {
          const tx = lerp(830 + 70 * i, 830 + 30 * i, shrink);
          const ty = lerp(560, 560, shrink);
          const sc = lerp(1, 22 / 64, shrink);
          tile.style.transform = `translate(${tx - (830 + 70 * i)}px, ${ty - 560}px) scale(${sc})`;
          tile.style.opacity = shrink >= 1 ? '0' : '1';
        });

        const dotProgress = inout(prog(t, c.count_start + 0.3, c.count_end - c.count_start - 0.3));
        const shown = Math.round(lerp(0, count.to, dotProgress));
        dots.forEach((d, i) => {
          d.style.opacity = i < shown ? '1' : '0';
        });

        lines.forEach((n2, i) => slamIn(n2, t, s.lines[i].t));
      },
    };
  };

  BUILD.screen = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'words', size: 64, top: 170 });
    const box = sheet(el, { x: s.sheet.x, y: s.sheet.y, w: s.sheet.w, h: 560 });
    const clip = mk('div', 'clip', box);
    css(clip, { left: '10px', top: '10px', right: '10px', bottom: '10px', borderRadius: '6px' });
    const img = mk('img', '', clip);
    img.src = s.image;
    img.decoding = 'sync';
    img.style.width = '2880px';
    img.style.height = 'auto';

    const st = stamp(box, s.stamp);
    st.style.right = '34px';
    st.style.bottom = '34px';

    const lines = s.lines.map((l) => textBlock(el, { text: l.text, cls: 'words', size: 40, top: l.top }));

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);
        sheetIn(box, t, c.sheet, { from: 700, dx: 200, tilt: 1.4, screen: true });

        // CAMERA — drift to the second box 0.3 after landing, over 0.9 (inout).
        const k = inout(prog(t, c.sheet + 1.2, 0.9));
        const b = cameraBox(s.boxes[0], s.boxes[1], k);
        const scale = (s.sheet.w - 0) / b.w;
        img.style.transform = `translate(${-b.x * scale}px, ${-b.y * scale}px) scale(${scale})`;

        const jump = pulse(t, c.stamp + 0.03, 0.1);
        box.style.marginLeft = `${5 * jump}px`;
        box.style.marginTop = `${3 * jump}px`;

        drawStamp(st, t, c.stamp, -5);
        lines.forEach((n, i) => slamIn(n, t, s.lines[i].t));
        sheetOut(box, t, s.end - 0.4);
      },
    };
  };

  BUILD.compare = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'display', size: 190, top: 210 });
    const cards = s.sheets.map((cfg, i) => {
      const x = 830 + i * 540;
      const card = sheet(el, { x, y: 400, w: 500, h: 350 });
      const label = mk('div', 'data', card, cfg.label);
      css(label, { position: 'absolute', left: '30px', top: '26px', fontSize: '21px', color: 'var(--muted)', letterSpacing: '.04em' });
      const score = mk('div', 'display', card, cfg.score);
      css(score, { position: 'absolute', left: '26px', top: '66px', fontSize: '160px', lineHeight: '0.9' });
      const grade = mk('div', 'data500', card, 'grade ' + cfg.grade);
      css(grade, { position: 'absolute', right: '30px', top: '30px', fontSize: '22px', color: 'var(--muted)' });
      const st = stamp(card, { ...cfg.stamp, size: 64 });
      st.style.right = '20px';
      st.style.bottom = '20px';
      return { card, st, cfg };
    });
    const line = textBlock(el, { text: s.lines[0].text, cls: 'words', size: 40, top: s.lines[0].top });

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);
        cards.forEach(({ card, st, cfg }, i) => {
          const t0 = i === 0 ? c.sheetA : c.sheetB;
          sheetIn(card, t, t0, { from: 680 + i * 40, tilt: i === 0 ? -3 : 3 });
          const hit = i === 0 ? c.stampA : c.stampB;
          const jump = pulse(t, hit + 0.03, 0.1);
          card.style.marginLeft = `${5 * jump}px`;
          card.style.marginTop = `${3 * jump}px`;
          drawStamp(st, t, hit, cfg.stamp.ok ? -7 : -5);
        });
        slamIn(line, t, s.lines[0].t);
      },
    };
  };

  BUILD.receipt = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'words', size: 64, top: 170 });
    const r = s.receipt;
    const x = 1810 - r.width;
    const slotY = 806;

    const slot = mk('div', '', el);
    css(slot, {
      position: 'absolute', left: x - 36 + 'px', top: slotY + 'px',
      width: r.width + 72 + 'px', height: '36px', background: '#000', borderRadius: '18px',
    });

    const wrapper = mk('div', '', el);
    css(wrapper, {
      position: 'absolute', left: x + 'px', bottom: `${1080 - slotY - 8}px`,
      width: r.width + 'px', filter: 'drop-shadow(0 18px 28px rgba(20,20,20,.28))',
    });

    const paper = mk('div', '', wrapper);
    css(paper, {
      position: 'absolute', left: 0, bottom: '0px', width: r.width + 'px',
      background: '#fff', border: '1.5px solid #DAD9D3', borderBottom: 'none',
      height: '70px', overflow: 'hidden',
    });
    // torn top: 24 teeth, 12 px deep
    const pts = [];
    const teeth = 24;
    for (let i = 0; i <= teeth; i++) {
      pts.push(`${(i / teeth) * 100}% ${i % 2 === 0 ? 0 : 12}px`);
    }
    paper.style.clipPath = `polygon(0% 12px, ${pts.join(', ')}, 100% 100%, 0% 100%)`;

    const inner = mk('div', '', paper);
    css(inner, { position: 'absolute', left: 0, right: 0, bottom: 0, padding: '0 34px 24px' });

    const rows = r.lines.map((ln, i) => {
      const row = mk('div', '', inner);
      css(row, { position: 'relative', height: r.pitch + 'px', opacity: '0' });
      const left = mk('div', 'data', row, ln.left);
      css(left, { position: 'absolute', left: 0, top: '8px', fontSize: '24px', color: 'var(--muted)' });
      const right = mk('div', 'data500', row, ln.right);
      css(right, { position: 'absolute', right: 0, top: '8px', fontSize: '24px' });
      return row;
    });
    const foot = mk('div', 'words', inner, r.foot);
    css(foot, { position: 'relative', fontSize: '30px', paddingTop: '16px', opacity: '0' });

    const line = textBlock(el, { text: s.lines[0].text, cls: 'words', size: 36, top: s.lines[0].top });

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);
        slot.style.opacity = '1';

        const tearK = in3(prog(t, c.tear, 0.42));
        paper.style.transform = `translate(${300 * tearK}px, ${-1250 * tearK}px) rotate(${10 * tearK}deg)`;
        slot.style.transform = `translateY(${170 * tearK}px)`;

        let fed = 0;
        rows.forEach((row, i) => {
          const k = prog(t, c.print[i], 0.09);
          if (k > 0) fed = i + out3(k);
          row.style.opacity = k > 0 ? '1' : '0';
          row.style.transform = `translateY(${lerp(8, 0, out3(k))}px)`;
        });
        foot.style.opacity = fed >= rows.length ? '1' : '0';
        const h = 48 + fed * r.pitch + 22 + (fed >= rows.length ? 54 : 0);
        paper.style.height = h + 'px';
        inner.style.height = h + 'px';

        slamIn(line, t, s.lines[0].t);
      },
    };
  };

  BUILD.terminal = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'words', size: 64, top: 170 });
    const win = sheet(el, { x: 830, y: 240, w: 900, h: 520, dark: true });
    css(win, { borderRadius: '10px' });

    const bar = mk('div', '', win);
    css(bar, { position: 'absolute', left: 0, right: 0, top: 0, height: '44px', borderBottom: '1px solid #2a2a2a', borderRadius: '10px 10px 0 0' });
    const title = mk('div', 'data', bar, 'leakbench — zsh');
    css(title, { position: 'absolute', left: '18px', top: '12px', fontSize: '16px', color: '#8b8b86' });
    ['#ff5f56', '#ffbd2e', '#27c93f'].forEach((color, i) => {
      const dot = mk('div', '', bar);
      css(dot, { position: 'absolute', right: 20 + i * 20 + 'px', top: '17px', width: '10px', height: '10px', borderRadius: '50%', background: color, opacity: '.5' });
    });

    const body = mk('div', '', win);
    css(body, { position: 'absolute', left: '28px', top: '70px', right: '28px' });

    const prompt = mk('div', '', body);
    css(prompt, { position: 'relative', height: '34px' });
    const ps = mk('span', 'data500', prompt, '› ');
    css(ps, { color: '#8b8b86', fontSize: '24px' });
    const typedEl = mk('span', 'data', prompt, '');
    css(typedEl, { fontSize: '24px', color: '#F3F3F0' });
    const caret = mk('span', '', prompt);
    css(caret, { display: 'inline-block', width: '12px', height: '26px', background: '#F3F3F0', verticalAlign: '-3px', marginLeft: '2px' });

    const outs = s.outputs.map((text, i) => {
      const row = mk('div', `${i === s.ok_index ? 'data500' : 'data'}`, body, text);
      css(row, { position: 'relative', fontSize: '24px', lineHeight: '34px', marginTop: '2px', color: i === s.ok_index ? 'var(--green)' : '#C9C9C4', opacity: '0' });
      return row;
    });

    const line = textBlock(el, { text: s.lines[0].text, cls: 'words', size: 36, top: s.lines[0].top });
    const keys = evs(s.id, 'typekey').filter((e) => e.char !== undefined);

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);
        const k = out3(prog(t, c.window, 0.45));
        win.style.transform = `translate(${lerp(260, 0, k)}px, ${lerp(40, 0, k)}px) rotate(${lerp(2, 0, k)}deg)`;
        win.style.opacity = '1';

        let n = 0;
        for (const key of keys) if (reached(t, key.t)) n = key.index + 1;
        typedEl.textContent = s.command.slice(0, n);
        const typing = t < keys[keys.length - 1].t + 0.12;
        const blink = typing ? true : Math.floor(t * 2) % 2 === 0;
        caret.style.opacity = blink ? '1' : '0';

        outs.forEach((row, i) => {
          const local = prog(t, c.outputs[i], 0.18);
          row.style.opacity = String(clamp(4 * local));
          row.style.transform = `translateX(${lerp(-6, 0, out3(local))}px)`;
        });
        slamIn(line, t, s.lines[0].t);
      },
    };
  };

  BUILD.slips = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'words', size: 76, top: 190 });
    const spots = [
      { x: 900, y: 380 }, { x: 1180, y: 330 }, { x: 1460, y: 410 },
      { x: 1020, y: 560 }, { x: 1330, y: 560 },
    ];
    const slips = s.slips.map((slip, i) => {
      const spot = spots[i % spots.length];
      const card = mk('div', 'sheet', el);
      css(card, { left: spot.x + 'px', top: spot.y + 'px', width: '210px', height: '118px' });
      const pin = mk('div', 'pin', el);
      css(pin, { left: spot.x + 105 - 13 + 'px', top: spot.y - 13 + 'px' });
      const value = mk('div', 'data500', card, slip.value);
      css(value, { position: 'absolute', left: '20px', top: '30px', fontSize: '30px' });
      const unit = mk('div', 'data', card, 'ZEC');
      css(unit, { position: 'absolute', left: '20px', bottom: '18px', fontSize: '16px', color: 'var(--muted)' });
      return { card, pin, slip, spot };
    });
    const note = textBlock(el, { text: s.note, cls: 'data', size: 26, left: 160, top: 700, color: 'var(--muted)' });
    const lines = s.lines.map((l) => textBlock(el, { text: l.text, cls: 'words', size: 40, top: l.top }));

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);
        slips.forEach(({ card, pin, slip, spot }, i) => {
          const t0 = c.slips[i];
          const k = out3(prog(t, t0, 0.18));
          const scale = lerp(1.12, 1, k);
          const y = lerp(-80, 0, k);
          const rot = lerp(slip.tilt * 1.6, slip.tilt, k);
          card.style.transform = `translateY(${y}px) rotate(${rot}deg) scale(${scale})`;
          card.style.opacity = String(clamp(4 * k));
          pin.style.opacity = card.style.opacity;
          pin.style.transform = `translate(${spot.x + 105 - 13 - (spot.x + 105 - 13)}px, ${y}px)`;
        });
        slamIn(note, t, c.lines[0] - 0.2);
        lines.forEach((n, i) => slamIn(n, t, s.lines[i].t));
      },
    };
  };

  BUILD.refusal = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'display', size: 190, top: 250 });

    const key = mk('div', '', el);
    css(key, { position: 'absolute', left: '980px', top: '540px', width: '300px', height: '70px' });
    const ring = mk('div', '', key);
    css(ring, { position: 'absolute', left: 0, top: '0px', width: '58px', height: '58px', borderRadius: '50%', border: '17px solid #000' });
    const shaft = mk('div', '', key);
    css(shaft, { position: 'absolute', left: '52px', top: '24px', width: '200px', height: '20px', background: '#000' });
    [0, 1].forEach((i) => {
      const tooth = mk('div', '', key);
      css(tooth, { position: 'absolute', left: 170 + i * 40 + 'px', top: '40px', width: '18px', height: '28px', background: '#000' });
    });

    const st = stamp(el, { ...s.stamp, size: 96 });
    st.style.left = '1010px';
    st.style.top = '330px';

    const line = textBlock(el, { text: s.lines[0].text, cls: 'words', size: 36, top: s.lines[0].top });

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);

        const k = out3(prog(t, c.key - 0.5, 0.5));
        const turn = inout(prog(t, c.key + 0.5, 0.22));
        const jiggle = pulse(t, c.jiggle, 0.5);
        const jx = 10 * Math.sin(6 * Math.PI * jiggle) * (1 - jiggle) * 6;
        key.style.transform = `translate(${lerp(-420, 0, k) + (turn > 0 ? jx : 0)}px, 0) scaleY(${lerp(1, 0.3, turn)})`;
        key.style.opacity = String(clamp(4 * k));

        drawStamp(st, t, c.stamp, -5);
        slamIn(line, t, s.lines[0].t);
      },
    };
  };

  BUILD.phone = (s) => {
    const el = mk('div', 'scene');
    const heading = textBlock(el, { text: s.heading, cls: 'words', size: 64, top: 170 });

    const device = mk('div', '', el);
    css(device, {
      position: 'absolute', left: '1340px', top: '190px', width: '380px', height: '800px',
      background: '#000', borderRadius: '54px', padding: '12px',
    });
    const screen = mk('div', '', device);
    css(screen, { position: 'relative', width: '356px', height: '776px', borderRadius: '44px', overflow: 'hidden', background: '#F3F3F0' });
    const img = mk('img', '', screen);
    img.src = s.image;
    img.decoding = 'sync';
    css(img, { position: 'absolute', left: 0, top: 0, width: '780px', height: 'auto', transformOrigin: '0 0' });

    const scan = mk('div', '', screen);
    css(scan, { position: 'absolute', left: 0, right: 0, height: '3px', background: 'var(--green)', opacity: '0', boxShadow: '0 0 18px rgba(11,107,58,.6)' });

    const tick = mk('div', '', screen);
    css(tick, {
      position: 'absolute', left: '50%', top: '50%', width: '96px', height: '96px', marginLeft: '-48px',
      marginTop: '-48px', borderRadius: '50%', background: 'var(--green)', opacity: '0',
    });
    const tickMark = mk('div', '', tick);
    css(tickMark, {
      position: 'absolute', left: '26px', top: '30px', width: '42px', height: '22px',
      borderLeft: '7px solid #F3F3F0', borderBottom: '7px solid #F3F3F0', transform: 'rotate(-45deg)',
    });

    const line = textBlock(el, { text: s.lines[0].text, cls: 'data', size: 30, top: s.lines[0].top });
    const slip = mk('div', 'sheet', el);
    css(slip, { width: '190px', height: '86px', opacity: '0' });
    const slipText = mk('div', 'data', slip, 'peers.json');
    css(slipText, { position: 'absolute', left: '14px', top: '18px', fontSize: '15px' });
    const slipText2 = mk('div', 'data500', slip, '3,412 peers');
    css(slipText2, { position: 'absolute', left: '14px', top: '42px', fontSize: '19px' });

    return {
      el,
      layout() {},
      draw(t) {
        const c = s.cues;
        slamIn(heading, t, c.heading);

        const k = out3(prog(t, c.phone, 0.5));
        device.style.transform = `translateY(${lerp(900, 0, k)}px) rotate(${lerp(4, 0, k)}deg)`;

        // page scroll inside the phone
        const scroll = inout(prog(t, c.scroll, 0.55));
        img.style.transform = `translateY(${-scroll * 520}px)`;

        // scan line sweeps back and forth every 0.9
        const phase = (t - c.scan[0]) / 0.9;
        let scanY = null;
        if (phase >= 0 && phase < 2) {
          const kk = inout(phase % 1);
          scanY = 210 + (phase < 1 ? kk : 1 - kk) * 330;
        }
        scan.style.opacity = scanY === null ? '0' : '1';
        if (scanY !== null) scan.style.top = scanY + 'px';

        const tickK = prog(t, c.tick, 0.22);
        tick.style.opacity = String(clamp(4 * tickK));
        tick.style.transform = `scale(${lerp(0.4, 1, back(tickK))})`;

        // HAND-OVER: a slip flies from the phone to the words.
        const hk = inout(prog(t, c.handover, 0.5));
        const fromX = 1400, fromY = 620, toX = 160, toY = s.lines[0].top - 16;
        const x = lerp(fromX, toX, hk);
        const y = lerp(fromY, toY, hk) - 120 * Math.sin(Math.PI * hk);
        slip.style.left = x + 'px';
        slip.style.top = y + 'px';
        slip.style.transform = `rotate(${lerp(-8, -2, hk)}deg) scale(${lerp(0.8, 1, hk)})`;
        slip.style.opacity = String(clamp(4 * prog(t, c.handover, 0.1)));

        slamIn(line, t, c.handover + 0.12);
      },
    };
  };

  BUILD.name = (s) => {
    const el = mk('div', 'scene');
    const word = textBlock(el, { text: s.word, cls: 'display', size: 210, left: 160, top: 400 });
    word.style.letterSpacing = '-0.02em';
    const sub = textBlock(el, { text: s.sub, cls: 'data', size: 30, left: 165, top: 660, color: 'var(--muted)' });

    const bars = [];
    return {
      el,
      layout() {
        const box = word.getBoundingClientRect();
        for (let i = 0; i < 3; i++) {
          const bar = mk('div', 'bar', el);
          css(bar, {
            left: box.left - 20 + 'px',
            top: box.top - 30 + i * 104 + 'px',
            width: box.width + 40 + 'px',
            height: '92px',
          });
          bars.push(bar);
        }
      },
      draw(t) {
        const c = s.cues;
        bars.forEach((bar, i) => {
          const grow = prog(t, c.bars[i], 0.24);
          const retract = prog(t, c.reveal + i * 0.05, 0.3);
          let sx = out3(grow);
          let origin = '0 50%';
          if (retract > 0) {
            sx = 1 - in3(retract);
            origin = '100% 50%';
          }
          bar.style.transformOrigin = origin;
          bar.style.transform = `scaleX(${sx})`;
        });
        slamIn(sub, t, c.sub);
        word.style.opacity = '1';
      },
    };
  };

  // ---------------------------------------------------------------- build
  const scenes = TL.scenes.map((s) => {
    const built = BUILD[s.template](s);
    built.id = s.id;
    built.start = s.start;
    built.end = s.end;
    built.data = s;
    sceneLayer.appendChild(built.el);
    return built;
  });

  // wipe bars
  const wipeBars = [];
  for (let i = 0; i < 9; i++) {
    const bar = mk('div', 'bar wipebar', wipeLayer);
    css(bar, { left: '-30px', top: 6 + i * 120 + 'px', width: '1980px', height: '108px', opacity: '0' });
    wipeBars.push(bar);
  }

  // mascot
  const mascot = mk('div', '', mascotLayer);
  css(mascot, { position: 'absolute', left: 0, top: 0, transformOrigin: '50% 100%' });
  const body = mk('div', '', mascot);
  css(body, { position: 'absolute', left: 0, top: 0, background: '#000', willChange: 'transform' });
  const eyes = [0, 1].map((i) => {
    const eye = mk('div', '', body);
    css(eye, { position: 'absolute', background: '#fff', borderRadius: '50%', overflow: 'hidden' });
    const pupil = mk('div', '', eye);
    css(pupil, { position: 'absolute', background: '#000', borderRadius: '50%' });
    return { eye, pupil };
  });

  function drawMascot(t, scene) {
    const cfg = scene && scene.data.mascot;
    if (!cfg) {
      mascotLayer.style.opacity = '0';
      return;
    }
    mascotLayer.style.opacity = '1';
    const { x, y, w, h } = cfg;
    const enterStart = cfg.enter - 0.42;
    const enter = out3(prog(t, enterStart, 0.42));
    const xNow = lerp(2010, x, enter);
    const land = pulse(t, cfg.enter, 0.1);
    let hopY = 0;
    let hopStretch = 1;
    if (cfg.hop) {
      const hk = prog(t, cfg.hop, 0.34);
      hopY = -56 * Math.sin(Math.PI * hk);
      hopStretch = 1 + 0.06 * Math.sin(Math.PI * hk);
    }
    css(mascot, { left: xNow + 'px', top: y + 'px', width: w + 'px', height: h + 'px' });
    mascot.style.transform = `translateY(${hopY}px) scale(${1 + 0.1 * land}, ${(1 - 0.18 * land) * hopStretch})`;
    body.style.width = w + 'px';
    body.style.height = h + 'px';
    body.style.borderRadius = h * 0.2 + 'px';

    // eyes: diameter 0.4h, outer one ending 0.3h from the right end, 0.42 apart
    const eyeD = h * 0.4;
    const cy = h / 2;
    const cx2 = w - h * 0.5;
    const cx1 = cx2 - eyeD * 1.42;
    const openK = prog(t, cfg.enter + 0.05, 0.12);
    let eyeScale = lerp(0.1, 1, out3(openK));
    for (let c = cfg.enter + 0.9; c < DUR; c += 1.9) {
      eyeScale = Math.min(eyeScale, 1 - 0.9 * pulse(t, c, 0.075));
    }
    if (cfg.wink) eyeScale = Math.min(eyeScale, 1 - 0.9 * pulse(t, cfg.wink, 0.16));

    eyes.forEach(({ eye, pupil }, i) => {
      const cx = i === 0 ? cx1 : cx2;
      css(eye, {
        left: cx - eyeD / 2 + 'px',
        top: cy - eyeD / 2 + 'px',
        width: eyeD + 'px',
        height: eyeD + 'px',
      });
      eye.style.transform = `scaleY(${i === 1 && cfg.wink ? eyeScale : eyeScale})`;
      const pupilD = eyeD * 0.46;
      const spare = eyeD - pupilD;
      const look = Math.sin(t * 0.9 + i * 0.4) * 0.5;
      css(pupil, {
        left: spare / 2 + look * 0.42 * spare + 'px',
        top: spare / 2 + Math.cos(t * 0.7) * 0.18 * spare + 'px',
        width: pupilD + 'px',
        height: pupilD + 'px',
      });
    });
  }

  function drawWipe(t) {
    const cuts = TL.scenes.slice(1).map((s) => s.start);
    let cut = null;
    for (const c of cuts) {
      if (t > c - 0.5 && t < c + 0.45) {
        cut = c;
        break;
      }
    }
    wipeBars.forEach((bar, i) => {
      if (cut === null) {
        bar.style.opacity = '0';
        return;
      }
      const lag = 0.014 * i + 0.03 * (i % 2);
      const grow = out3(prog(t, cut - 0.42 + lag, 0.26));
      const retract = in3(prog(t, cut + 0.04 + lag, 0.26));
      let sx = grow;
      if (retract > 0) sx = 1 - retract;
      bar.style.opacity = sx > 0.0001 ? '1' : '0';
      bar.style.transformOrigin = retract > 0 ? '100% 50%' : '0 50%';
      bar.style.transform = `scaleX(${sx})`;
    });
  }

  // ---------------------------------------------------------------- seek
  function sceneAt(t) {
    for (const s of scenes) if (t >= s.start && t < s.end) return s;
    return t >= DUR ? scenes[scenes.length - 1] : scenes[0];
  }

  function seek(t) {
    const clamped = Math.max(0, Math.min(DUR - 1e-6, t));
    const active = sceneAt(clamped);
    for (const s of scenes) {
      s.el.style.display = s === active ? 'block' : 'none';
    }
    for (const node of exitLayer.children) {
      node.style.display = node.dataset.scene === active.id ? 'block' : 'none';
    }
    active.draw(clamped);
    drawWipe(clamped);
    drawMascot(clamped, active);
  }

  // ---------------------------------------------------------------- boot
  async function ready() {
    await document.fonts.ready;
    const imgs = [...document.images];
    await Promise.all(imgs.map((i) => (i.decode ? i.decode().catch(() => {}) : Promise.resolve())));
    // measure with every scene visible, then hide again
    scenes.forEach((s) => (s.el.style.display = 'block'));
    scenes.forEach((s) => s.layout && s.layout());
    scenes.forEach((s) => (s.el.style.display = 'none'));
    seek(0);
    window.__READY = true;
  }

  window.seek = seek;
  window.FRAMES = Math.round(DUR * FPS);
  window.READY = ready();

  // a self-test hook for render.mjs --check
  window.__probe = (t) => {
    seek(t);
    return [...stage.querySelectorAll('*')].length;
  };
})();
