"use strict";
const T = window.__TAURI__;
const invoke = (c, a) => T.core.invoke(c, a);
const STORE = "clickr.config.v1";
const DEFAULT = { enabled: true, triggerKey: "F6", mode: "hold", targetCps: 100,
  outputKeys: [], multiInput: false, alwaysOnTop: false, accentColor: "#b3cbff" };
const MAX_CPS = 5000, WARN_CPS = 1000;

let cfg = load();
function load() {
  try { const s = JSON.parse(localStorage.getItem(STORE)); if (s) return { ...DEFAULT, ...s }; } catch (e) {}
  return { ...DEFAULT };
}
const $ = id => document.getElementById(id);
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

// ---- key naming ---------------------------------------------------------------
const CODE_MAP = { Space: "Space", Enter: "Enter", NumpadEnter: "Enter", Tab: "Tab", Escape: "Escape",
  Backspace: "Backspace", CapsLock: "CapsLock", ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left",
  ArrowRight: "Right", Insert: "Insert", Delete: "Delete", Home: "Home", End: "End", PageUp: "PageUp",
  PageDown: "PageDown", ShiftLeft: "Shift", ShiftRight: "Shift", ControlLeft: "Ctrl", ControlRight: "Ctrl",
  AltLeft: "Alt", AltRight: "Alt", Semicolon: "Semicolon", Equal: "Equal", Comma: "Comma", Minus: "Minus",
  Period: "Period", Slash: "Slash", Backquote: "Backquote", BracketLeft: "BracketLeft",
  Backslash: "Backslash", BracketRight: "BracketRight", Quote: "Quote" };
function codeToName(code) {
  if (CODE_MAP[code]) return CODE_MAP[code];
  let m;
  if ((m = /^Key([A-Z])$/.exec(code))) return m[1];
  if ((m = /^Digit([0-9])$/.exec(code))) return m[1];
  if ((m = /^Numpad([0-9])$/.exec(code))) return "Num" + m[1];
  if ((m = /^F([0-9]{1,2})$/.exec(code)) && +m[1] >= 1 && +m[1] <= 24) return "F" + m[1];
  return null;
}
const MOUSE_BTN = ["MouseLeft", "MouseMiddle", "MouseRight", "MouseX1", "MouseX2"]; // by event.button
const PRETTY = { MouseLeft: "left click", MouseRight: "right click", MouseMiddle: "middle click",
  MouseX1: "mouse back", MouseX2: "mouse forward", Backquote: "`", Semicolon: ";", Equal: "=", Comma: ",",
  Minus: "-", Period: ".", Slash: "/", BracketLeft: "[", BracketRight: "]", Backslash: "\\", Quote: "'" };
const pretty = n => PRETTY[n] || n;

// ---- state -> backend ---------------------------------------------------------
function save() { try { localStorage.setItem(STORE, JSON.stringify(cfg)); } catch (e) {} }
function push() {
  save();
  invoke("update_config", { config: {
    enabled: cfg.enabled, triggerKey: cfg.triggerKey, mode: cfg.mode, targetCps: cfg.targetCps,
    outputKeys: cfg.outputKeys, multiInput: cfg.multiInput } }).catch(() => {});
}
function toast(msg) {
  const t = $("toast"); t.textContent = msg; t.classList.add("show");
  clearTimeout(toast.h); toast.h = setTimeout(() => t.classList.remove("show"), 1800);
}

// ---- render -------------------------------------------------------------------
function hex2rgb(h) { const n = parseInt(h.slice(1), 16); return [n >> 16 & 255, n >> 8 & 255, n & 255]; }
function render() {
  const r = document.documentElement.style;
  r.setProperty("--accent", cfg.accentColor);
  const [R, G, B] = hex2rgb(cfg.accentColor); r.setProperty("--accent-dim", `rgba(${R},${G},${B},.45)`);
  $("accent").value = cfg.accentColor;
  for (const [id, key] of [["enabled", "enabled"], ["multi", "multiInput"], ["top", "alwaysOnTop"]])
    $(id).classList.toggle("on", !!cfg[key]);
  $("trigger").textContent = pretty(cfg.triggerKey);
  document.querySelectorAll("#mode button").forEach(b => b.classList.toggle("on", b.dataset.v === cfg.mode));
  if (document.activeElement !== $("cpsNum")) $("cpsNum").value = cfg.targetCps;
  $("cpsRange").value = cfg.targetCps;
  $("cpsRange").style.background = `linear-gradient(to right,var(--accent) ${(cfg.targetCps - 1) / (MAX_CPS - 1) * 100}%,#26262f 0)`;
  $("warn").hidden = cfg.targetCps <= WARN_CPS;
  const chips = $("chips"); chips.textContent = "";
  if (!cfg.outputKeys.length) { const e = document.createElement("div"); e.className = "empty";
    e.textContent = "nothing to press yet — add a key or button"; chips.append(e); }
  cfg.outputKeys.forEach((k, i) => {
    const c = document.createElement("span"); c.className = "chip"; c.append(pretty(k));
    const x = document.createElement("button"); x.textContent = "×"; x.title = "remove";
    x.onclick = () => { cfg.outputKeys.splice(i, 1); changed(); }; c.append(x); chips.append(c);
  });
}
function changed() { render(); push(); }

// ---- key capture ---------------------------------------------------------------
let capture = null;
function startCapture(btn, allowLeft, done) {
  stopCapture(); const old = btn.textContent; btn.textContent = "press a key…"; btn.classList.add("cap");
  const finish = name => { stopCapture(); if (name) done(name); render(); };
  const kd = e => { e.preventDefault(); e.stopPropagation();
    if (e.code === "Escape") return finish(null); const n = codeToName(e.code); if (n) finish(n); };
  const md = e => { if (e.button === 0 && !allowLeft) return; e.preventDefault(); e.stopPropagation(); finish(MOUSE_BTN[e.button] || null); };
  const cm = e => e.preventDefault();
  setTimeout(() => { window.addEventListener("keydown", kd, true); window.addEventListener("mousedown", md, true);
    window.addEventListener("contextmenu", cm, true); }, 120);
  capture = () => { window.removeEventListener("keydown", kd, true); window.removeEventListener("mousedown", md, true);
    window.removeEventListener("contextmenu", cm, true); btn.classList.remove("cap"); btn.textContent = old; };
}
function stopCapture() { if (capture) { capture(); capture = null; } }

function addOutput(name) {
  if (name === cfg.triggerKey) return toast("that's your trigger key — pick another");
  if (cfg.outputKeys.includes(name)) return toast("already added");
  cfg.outputKeys.push(name); changed();
}

// ---- wiring ---------------------------------------------------------------------
$("enabled").onclick = () => { cfg.enabled = !cfg.enabled; changed(); };
$("multi").onclick = () => { cfg.multiInput = !cfg.multiInput; changed(); };
$("top").onclick = () => { cfg.alwaysOnTop = !cfg.alwaysOnTop; applyTop(); changed(); };
$("accent").oninput = e => { cfg.accentColor = e.target.value; render(); save(); };
document.querySelectorAll("#mode button").forEach(b => b.onclick = () => { cfg.mode = b.dataset.v; changed(); });
$("trigger").onclick = () => startCapture($("trigger"), false, n => {
  if (cfg.outputKeys.includes(n)) { cfg.outputKeys = cfg.outputKeys.filter(k => k !== n); toast("removed from output keys"); }
  cfg.triggerKey = n; changed(); });
$("add").onclick = () => startCapture($("add"), false, addOutput);
document.querySelectorAll("[data-q]").forEach(b => b.onclick = () => addOutput(b.dataset.q));
$("cpsRange").oninput = e => { cfg.targetCps = +e.target.value; changed(); };
$("cpsNum").oninput = e => { const v = clamp(Math.round(+e.target.value) || 1, 1, MAX_CPS); cfg.targetCps = v; render(); push(); };
$("cpsNum").onblur = () => render();
$("discord").onclick = async e => {
  e.preventDefault();
  try { await navigator.clipboard.writeText("6a5m"); }
  catch (_) { const t = document.createElement("textarea"); t.value = "6a5m"; document.body.append(t); t.select();
    try { document.execCommand("copy"); } catch (e2) {} t.remove(); }
  toast("copied discord username: 6a5m");
};
function applyTop() { try { T.window.getCurrentWindow().setAlwaysOnTop(!!cfg.alwaysOnTop).catch(() => {}); } catch (e) {} }
window.addEventListener("contextmenu", e => e.preventDefault());

// ---- live stats -----------------------------------------------------------------
const hist = Array.from({ length: 60 }, () => ({ cps: 0, cpu: 0 }));
function draw() {
  const cv = $("chart"), dpr = window.devicePixelRatio || 1, w = cv.clientWidth, h = cv.clientHeight;
  if (cv.width !== w * dpr) { cv.width = w * dpr; cv.height = h * dpr; }
  const g = cv.getContext("2d"); g.setTransform(dpr, 0, 0, dpr, 0, 0); g.clearRect(0, 0, w, h);
  const max = Math.max(10, ...hist.map(p => p.cps)), css = getComputedStyle(document.documentElement);
  const line = (f, col, mx) => { g.beginPath(); hist.forEach((p, i) => { const x = i / (hist.length - 1) * w, y = h - 3 - f(p) / mx * (h - 6);
    i ? g.lineTo(x, y) : g.moveTo(x, y); }); g.strokeStyle = col; g.lineWidth = 2; g.stroke(); };
  line(p => p.cpu, "#ff9d7a", 100); line(p => p.cps, css.getPropertyValue("--accent").trim(), max);
}
function setRunning(on) {
  $("dot").classList.toggle("on", on); $("pill").classList.toggle("on", on); $("pill").textContent = on ? "clicking" : "idle";
}
async function init() {
  render(); applyTop(); push();
  await T.event.listen("clickr://running", e => setRunning(!!e.payload));
  await T.event.listen("clickr://stats", e => {
    if (document.hidden) return;
    hist.push({ cps: e.payload.cps, cpu: e.payload.cpu }); hist.shift();
    $("cps").textContent = Math.round(e.payload.cps).toLocaleString();
    $("cpu").textContent = e.payload.cpu.toFixed(1) + "%"; draw();
  });
  draw();
}
init();
