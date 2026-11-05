/* Pliego · instalador. Sin dependencias. Habla con Rust por `window.ipc.postMessage` y recibe `window.__pl.*`. */
"use strict";

const STR = {
  es: {
    install: "Instalar", options: "Opciones de instalación", back: "Atrás", cancel: "Cancelar", close: "Cerrar", min: "Minimizar",
    welcome_h: "Instalar Pliego", welcome_p: "Lee, firma y organiza tus PDF. Libre, ligero y sin nube.",
    tip_where: (d) => `Para ti, en ${d}`, tip_where_machine: (d) => `Para todos los usuarios, en ${d}`,
    tip_assoc: "Abre PDF con Pliego y añade el menú contextual", tip_updates: "Busca actualizaciones firmadas automáticamente",
    legal: "Código visible · licencia BUSL-1.1", license: "Ver licencia", license_h: "Licencia de Pliego (BUSL-1.1)",
    opt_h: "Opciones de instalación", scope_user: "Solo para mí", scope_user_d: "Sin permisos de administrador",
    scope_machine: "Para todos", scope_machine_d: "Pide permiso de administrador", scope_label: "¿Para quién se instala?", change: "Cambiar…",
    o_assoc: "Abrir los PDF con Pliego", o_ctx: "Opciones de Pliego en el menú contextual del Explorador", o_desktop: "Acceso directo en el escritorio",
    o_path: "Añadir <span class='mono'>pliego</span> a la línea de órdenes", o_updates: "Buscar actualizaciones automáticamente", folder: "Carpeta de instalación",
    installing_h: "Instalando Pliego", uninstalling_h: "Desinstalando Pliego",
    st_checking: "Comprobando el equipo…", st_copying: "Copiando archivos…", st_verifying: "Comprobando la integridad del paquete…",
    st_registering: "Registrando Pliego en Windows…", st_shortcuts: "Creando accesos directos…", st_finishing: "Terminando…", st_removing: "Quitando archivos y registros…",
    done_h: "Pliego está instalado", done_p: "Ya puedes abrir tus PDF con doble clic.", done_default: "Usar Pliego como visor de PDF predeterminado",
    done_default_d: "Se abrirá Configuración de Windows para confirmarlo.", open: "Abrir Pliego",
    tip1: "Pulsa <span class='kbd'>Ctrl</span> <span class='kbd'>K</span> para encontrar cualquier herramienta", tip2: "Suelta archivos en la ventana para abrirlos o combinarlos", tip3: "Firma con tu DNIe sin instalar nada más",
    upd_done_h: "Pliego está actualizado", rep_done_h: "Pliego está reparado",
    same_h: "Pliego ya está instalado", same_p: (v) => `Versión ${v}. Puedes repararlo o desinstalarlo.`, repair: "Reparar", uninstall: "Desinstalar",
    older_h: (n) => `Actualizar a ${n}`, older_p: (v) => `Tienes la versión ${v}. Tus ajustes se conservan.`, update: (n) => `Actualizar a ${n}`,
    newer_h: "Ya tienes una versión más reciente", newer_p: (v) => `Tienes Pliego ${v}. Este instalador no instala versiones anteriores.`,
    running_h: "Pliego está abierto", running_p: "Cierra Pliego para continuar. Si tienes cambios sin guardar, Pliego te preguntará antes.", running_btn: "Cerrar Pliego por mí", running_wait: "Esperando a que Pliego se cierre…",
    un_h: "¿Desinstalar Pliego?", un_p: "Se quitarán el programa, sus accesos directos y la asociación con los PDF. Tus documentos no se tocan.", un_keep: "Conservar mis ajustes y firmas guardadas",
    uned_h: "Pliego se ha desinstalado", uned_p: "Gracias por probarlo.", uned_why: "Cuéntanos por qué (abre una página web)",
    err_h: "No se pudo completar", copy: "Copiar detalles", copied: "Copiado", log: "Registro:",
    e_no_space: (d) => `Faltan ${d.need} MB libres en ${d.drive}`, e_bad_path: "La carpeta de destino no existe o no es válida. Elige otra carpeta.", e_corrupt: "El instalador está dañado. Descárgalo de nuevo.", e_permission: "No hay permisos para escribir en esa carpeta. Elige otra o ejecuta como administrador.",
    e_old_windows: "Pliego necesita Windows 10 versión 2004 o posterior.", e_running: "No se pudo cerrar Pliego. Ciérralo y vuelve a intentarlo.", e_other: "Ocurrió un error inesperado.",
    e_cancelled: "Instalación cancelada.", retry: "Reintentar", version: (v) => `Versión ${v} · x64`, size: (m) => `${m} MB`,
  },
  en: {
    install: "Install", options: "Install options", back: "Back", cancel: "Cancel", close: "Close", min: "Minimize",
    welcome_h: "Install Pliego", welcome_p: "Read, sign and organize your PDFs. Free, light and cloud-free.",
    tip_where: (d) => `For you, in ${d}`, tip_where_machine: (d) => `For all users, in ${d}`,
    tip_assoc: "Opens PDFs with Pliego and adds the context menu", tip_updates: "Checks for signed updates automatically",
    legal: "Source-available · BUSL-1.1 license", license: "View license", license_h: "Pliego license (BUSL-1.1)",
    opt_h: "Install options", scope_user: "Just for me", scope_user_d: "No administrator rights needed",
    scope_machine: "For all users", scope_machine_d: "Asks for administrator permission", scope_label: "Who is it for?", change: "Change…",
    o_assoc: "Open PDFs with Pliego", o_ctx: "Pliego options in the Explorer context menu", o_desktop: "Desktop shortcut",
    o_path: "Add <span class='mono'>pliego</span> to the command line", o_updates: "Check for updates automatically", folder: "Install folder",
    installing_h: "Installing Pliego", uninstalling_h: "Uninstalling Pliego",
    st_checking: "Checking your PC…", st_copying: "Copying files…", st_verifying: "Checking package integrity…",
    st_registering: "Registering Pliego with Windows…", st_shortcuts: "Creating shortcuts…", st_finishing: "Finishing…", st_removing: "Removing files and registrations…",
    done_h: "Pliego is installed", done_p: "You can now open your PDFs with a double click.", done_default: "Use Pliego as the default PDF viewer",
    done_default_d: "Windows Settings will open to confirm it.", open: "Open Pliego",
    tip1: "Press <span class='kbd'>Ctrl</span> <span class='kbd'>K</span> to find any tool", tip2: "Drop files on the window to open or merge them", tip3: "Sign with your national ID card, nothing else to install",
    upd_done_h: "Pliego is up to date", rep_done_h: "Pliego is repaired",
    same_h: "Pliego is already installed", same_p: (v) => `Version ${v}. You can repair or uninstall it.`, repair: "Repair", uninstall: "Uninstall",
    older_h: (n) => `Update to ${n}`, older_p: (v) => `You have version ${v}. Your settings are kept.`, update: (n) => `Update to ${n}`,
    newer_h: "You already have a newer version", newer_p: (v) => `You have Pliego ${v}. This installer does not install older versions.`,
    running_h: "Pliego is open", running_p: "Close Pliego to continue. If you have unsaved changes, Pliego will ask first.", running_btn: "Close Pliego for me", running_wait: "Waiting for Pliego to close…",
    un_h: "Uninstall Pliego?", un_p: "The program, its shortcuts and the PDF association will be removed. Your documents are not touched.", un_keep: "Keep my settings and saved signatures",
    uned_h: "Pliego has been uninstalled", uned_p: "Thanks for trying it.", uned_why: "Tell us why (opens a web page)",
    err_h: "Could not complete", copy: "Copy details", copied: "Copied", log: "Log:",
    e_no_space: (d) => `${d.need} MB free space needed on ${d.drive}`, e_bad_path: "The destination folder does not exist or is not valid. Pick another folder.", e_corrupt: "The installer is damaged. Download it again.", e_permission: "No permission to write to that folder. Pick another or run as administrator.",
    e_old_windows: "Pliego needs Windows 10 version 2004 or later.", e_running: "Pliego could not be closed. Close it and try again.", e_other: "An unexpected error occurred.",
    e_cancelled: "Installation cancelled.", retry: "Retry", version: (v) => `Version ${v} · x64`, size: (m) => `${m} MB`,
  },
};

const I = {
  check: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
  shield: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.500-7-10V6z"/></svg>',
  kbd: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10"/></svg>',
  upload: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 16V4M7 9l5-5 5 5M5 20h14"/></svg>',
  cert: '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="9" r="5"/><path d="M9 13.500L8 21l4-2 4 2-1-7.500"/></svg>',
};

const $ = (s) => document.querySelector(s);
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

const S = { screen: "welcome", init: null, opts: null, step: "checking", pct: 0, lastError: null, doneKind: "install", keep: true, setDefault: true, closing: false };
let T = STR.es;
const send = (c, extra = {}) => window.ipc.postMessage(JSON.stringify({ c, ...extra }));
const say = (t) => { $("#live").textContent = ""; setTimeout(() => ($("#live").textContent = t), 30); };

/* ── La hoja se dobla según avanza ── */
function fold(pct) {
  const p = Math.max(0, Math.min(1, pct / 100)) * 4;
  const f = [0, 1, 2, 3].map((i) => 1 - 0.82 * Math.max(0, Math.min(1, p - i)));
  const width = 34 * f[0] * (1 + f[1] * (1 + f[2] * (1 + f[3])));
  const off = (140 - width) / 2;
  $("#p0").style.transform = `translate(${off}px,0) scale(${f[0]},1)`;
  [1, 2, 3].forEach((i) => ($(`#p${i}`).style.transform = `translate(34px,0) scale(${f[i]},1)`));
  document.querySelectorAll("#fold .pn-b").forEach((r, k) => r.classList.toggle("soft", p - (k * 2 + 1) > 0.5));
}

function chk(id, label, on) {
  return `<div class="chk"><input type="checkbox" id="${id}" ${on ? "checked" : ""}><span class="box">${I.check}</span><label for="${id}">${label}</label></div>`;
}

const tips = (items) => `<div class="tips">${items.map(([i, t]) => `<div>${i}<span>${t}</span></div>`).join("")}</div>`;
const foot = (...b) => `<div class="foot">${b.join("")}</div>`;
const btn = (id, label, cls = "", extra = "") => `<button type="button" class="btn ${cls}" id="${id}" ${extra}>${label}</button>`;
const grow = '<span class="grow"></span>';

function legal() {
  return `<div class="legal">${T.legal} · <button type="button" class="link plain" id="lic">${T.license}</button> · ${T.size(S.init.size_mb)}</div>`;
}

const SCREENS = {
  welcome() {
    const o = S.opts;
    return `<h3 tabindex="-1" id="h">${T.welcome_h}</h3><p>${T.welcome_p}</p>` +
      tips([[I.check, o.scope === "machine" ? T.tip_where_machine(esc(o.dir)) : T.tip_where(esc(o.dir))], [I.check, T.tip_assoc], [I.check, T.tip_updates]]) +
      foot(`<button type="button" class="link" id="go-opts">${T.options}</button>`, grow, btn("do-install", T.install, "pri big")) + legal();
  },
  options() {
    const o = S.opts;
    return `<h3 tabindex="-1" id="h">${T.opt_h}</h3>
      <fieldset class="scope"><legend class="sr-only">${T.scope_label}</legend>
        <label class="opt"><input type="radio" name="scope" value="user" ${o.scope === "user" ? "checked" : ""}><span class="r"></span><div>${T.scope_user}<small>${T.scope_user_d}</small></div></label>
        <label class="opt"><input type="radio" name="scope" value="machine" ${o.scope === "machine" ? "checked" : ""}><span class="r"></span><div>${T.scope_machine} ${I.shield}<small>${T.scope_machine_d}</small></div></label>
      </fieldset>
      <div class="pathrow"><div class="pathbox" id="dir" role="textbox" aria-readonly="true" aria-label="${T.folder}" tabindex="0" title="${esc(o.dir)}">${esc(o.dir)}</div>${btn("pick", T.change)}</div>
      <div class="optlist">${chk("o-assoc", T.o_assoc, o.assoc)}${chk("o-ctx", T.o_ctx, o.context_menu)}${chk("o-desktop", T.o_desktop, o.desktop_shortcut)}${chk("o-path", T.o_path, o.path)}${chk("o-updates", T.o_updates, o.updates)}</div>
      ${foot(btn("back", T.back, "ghost"), grow, btn("do-install", (o.scope === "machine" ? I.shield : "") + T.install, "pri"))}`;
  },
  installing() {
    return `<h3 tabindex="-1" id="h">${S.doneKind === "uninstall" ? T.uninstalling_h : T.installing_h}</h3><p id="step">${T["st_" + S.step] || ""}</p>
      <div class="prog" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${S.pct}" aria-label="${T.installing_h}"><i id="bar" style="width:${S.pct}%"></i></div>
      <div class="small tn" id="pct">${S.pct} %</div>
      ${foot(grow, S.doneKind === "uninstall" ? "" : btn("cancel", T.cancel, "ghost"))}`;
  },
  done() {
    const h = S.doneKind === "update" ? T.upd_done_h : S.doneKind === "repair" ? T.rep_done_h : T.done_h;
    return `<h3 tabindex="-1" id="h">${h}</h3><p>${T.done_p}</p>
      ${chk("o-default", T.done_default, S.setDefault)}<div class="small" style="margin:-8px 0 0 26px">${T.done_default_d}</div>
      ${tips([[I.kbd, T.tip1], [I.upload, T.tip2], [I.cert, T.tip3]])}
      ${foot(btn("quit", T.close, "ghost"), grow, btn("open", T.open, "pri big"))}`;
  },
  already() {
    const e = S.init.existing, n = S.init.version;
    if (e.relation === "older")
      return `<h3 tabindex="-1" id="h">${T.older_h(n)}</h3><p>${T.older_p(esc(e.version))}</p>${foot(btn("quit", T.cancel, "ghost"), grow, btn("do-update", T.update(n), "pri big"))}` + legal();
    if (e.relation === "newer")
      return `<h3 tabindex="-1" id="h">${T.newer_h}</h3><p>${T.newer_p(esc(e.version))}</p>${foot(btn("quit", T.close, "ghost"), grow, btn("open", T.open, "pri big"))}`;
    return `<h3 tabindex="-1" id="h">${T.same_h}</h3><p>${T.same_p(esc(e.version))}</p>
      ${foot(btn("quit", T.close, "ghost"), grow, btn("go-uninstall", T.uninstall, "danger"), btn("do-repair", T.repair, "pri"))}`;
  },
  running() {
    return `<h3 tabindex="-1" id="h">${T.running_h}</h3><p>${T.running_p}</p>
      <div class="proc"><svg width="20" height="20" viewBox="0 0 32 32" aria-hidden="true"><rect x="1" y="1" width="30" height="30" rx="7" fill="var(--pl-accent)"/><path d="M7.5 8l8.5 1.4v15.2L7.5 23.2z" fill="var(--pl-surface-raised)"/><path d="M16 9.4L24.5 8v15.2L16 24.6z" fill="var(--pl-accent-soft)"/></svg><span id="rstat">Pliego</span></div>
      ${foot(btn("quit", T.cancel, "ghost"), grow, btn("close-pliego", T.running_btn, "pri"))}`;
  },
  uninstall() {
    return `<h3 tabindex="-1" id="h">${T.un_h}</h3><p>${T.un_p}</p>${chk("o-keep", T.un_keep, S.keep)}
      ${foot(btn("quit", T.cancel, "ghost"), grow, btn("do-uninstall", T.uninstall, "danger"))}`;
  },
  uninstalled() {
    return `<h3 tabindex="-1" id="h">${T.uned_h}</h3><p>${T.uned_p}</p>
      <div><button type="button" class="link" id="why">${T.uned_why}</button></div>${foot(grow, btn("quit", T.close, "pri"))}`;
  },
  error() {
    const e = S.lastError;
    const msg = e.kind === "no_space" ? T.e_no_space(e) : T["e_" + e.kind] || T.e_other;
    return `<h3 tabindex="-1" id="h">${T.err_h}</h3><p>${msg}</p><div class="errbox" id="errtxt">${esc(e.detail || "")}</div>
      <div class="small">${T.log} <span class="mono" style="user-select:text">${esc(S.init.log)}</span></div>
      ${foot(btn("copy", T.copy, "ghost"), grow, e.kind === "cancelled" ? "" : btn("retry", T.retry), btn("quit", T.close, "pri"))}`;
  },
};

function show(screen, announce = true) {
  S.screen = screen;
  $("#pane").innerHTML = SCREENS[screen]();
  document.body.classList.toggle("idle-bob", screen !== "installing");
  fold(screen === "installing" ? S.pct : screen === "done" || screen === "uninstalled" ? 100 : 0);
  wire();
  const h = $("#h");
  if (h) {
    // El foco va al botón principal si lo hay (Enter actúa); el título se anuncia por separado.
    const primary = $("#pane .btn.pri") || $("#pane .btn");
    (primary || h).focus();
    if (announce) say(h.textContent);
  }
}

function readOpts() {
  const o = S.opts;
  const v = (id) => ($(id) ? $(id).checked : null);
  if ($("#o-assoc")) Object.assign(o, { assoc: v("#o-assoc"), context_menu: v("#o-ctx"), desktop_shortcut: v("#o-desktop"), path: v("#o-path"), updates: v("#o-updates") });
  return o;
}

function wire() {
  const on = (id, fn) => { const el = $(id); if (el) el.addEventListener("click", fn); };
  on("#go-opts", () => show("options"));
  on("#back", () => { readOpts(); show("welcome"); });
  on("#lic", () => showLicense());
  on("#pick", () => send("pick_folder", { current: S.opts.dir }));
  document.querySelectorAll('input[name="scope"]').forEach((r) => r.addEventListener("change", () => {
    readOpts();
    S.opts.scope = r.value;
    S.opts.dir = r.value === "machine" ? S.init.defaults.machine_dir : S.init.defaults.user_dir;
    show("options", false);
    document.querySelector(`input[name="scope"][value="${r.value}"]`).focus();
  }));
  on("#do-install", () => startInstall("install"));
  on("#do-update", () => { S.opts = Object.assign({}, S.init.existing.options || S.opts, { dir: S.init.existing.dir, scope: S.init.existing.scope }); startInstall("update"); });
  on("#do-repair", () => { S.opts = Object.assign({}, S.init.existing.options || S.opts, { dir: S.init.existing.dir, scope: S.init.existing.scope }); startInstall("repair"); });
  on("#go-uninstall", () => { S.init.mode = "uninstall"; S.init.root = S.init.existing.dir; show("uninstall"); });
  on("#do-uninstall", () => { S.keep = $("#o-keep").checked; startUninstall(); });
  on("#cancel", () => { send("cancel"); });
  on("#open", () => { send("open_pliego", { set_default: $("#o-default") ? $("#o-default").checked : false }); });
  on("#quit", () => send("quit"));
  on("#close-pliego", () => { S.closing = true; $("#rstat").textContent = T.running_wait; $("#close-pliego").disabled = true; send("close_pliego"); });
  on("#why", () => send("open_url", { url: "https://github.com/Carreraander/Pliego/discussions" }));
  on("#retry", () => { S.lastError = null; S.pct = 0; show(S.init.mode === "uninstall" ? "uninstall" : "welcome"); });
  on("#copy", () => {
    const e = S.lastError;
    const text = `Pliego ${S.init.version}\n${e.kind}: ${e.detail}\nLog: ${S.init.log}`;
    const done = () => ($("#copy").textContent = T.copied);
    (navigator.clipboard ? navigator.clipboard.writeText(text) : Promise.reject()).then(done, () => send("copy", { text }));
  });
}

function startInstall(kind) {
  readOpts();
  S.doneKind = kind; S.pct = 0; S.step = "checking";
  S.setDefault = true;
  show("installing");
  send("install", { opts: S.opts, kind });
}

function startUninstall() {
  S.doneKind = "uninstall"; S.pct = 0; S.step = "checking";
  show("installing");
  send("uninstall", { purge: !S.keep });
}

let licText = null;
function showLicense() {
  const ov = $("#overlay");
  $("#ov-title").textContent = T.license_h;
  $("#ov-close").textContent = T.close;
  ov.hidden = false;
  $("#ov-close").focus();
  const fill = (t) => ($("#ov-body").textContent = t);
  if (licText) return fill(licText);
  fetch("license.txt").then((r) => r.text()).then((t) => fill((licText = t))).catch(() => fill("https://github.com/Carreraander/Pliego/blob/main/LICENSE"));
}
function hideLicense() { $("#overlay").hidden = true; const b = $("#lic"); if (b) b.focus(); }

/* ── Eventos desde Rust ── */
window.__pl = {
  init(data) {
    S.init = data;
    T = STR[data.lang] || STR.es;
    document.documentElement.lang = data.lang;
    document.documentElement.dataset.theme = data.dark ? "dark" : "light";
    document.body.classList.toggle("mica", !!data.mica);
    $("#ver").textContent = T.version(data.version);
    $("#btn-min").setAttribute("aria-label", T.min);
    $("#btn-close").setAttribute("aria-label", T.close);
    S.opts = Object.assign({}, data.options);
    if (data.mode === "uninstall") {
      S.init.root = data.root;
      return data.running ? show("running") : show("uninstall");
    }
    if (data.begin) { S.doneKind = data.begin_kind || "install"; return startInstall(S.doneKind); }
    if (data.existing) return data.running && data.existing.relation !== "newer" ? show("running") : show("already");
    if (data.running) return show("running");
    if (data.error) { S.lastError = data.error; return show("error"); }
    show("welcome");
  },
  progress(step, percent) {
    S.step = step; S.pct = percent;
    if (S.screen !== "installing") return;
    $("#step").textContent = T["st_" + step] || "";
    $("#bar").style.width = percent + "%";
    $("#pct").textContent = percent + " %";
    $("#pane .prog").setAttribute("aria-valuenow", percent);
    fold(percent);
    if (percent % 25 === 0) say(`${$("#step").textContent} ${percent} %`);
  },
  done(kind) { S.doneKind = kind; S.pct = 100; show(kind === "uninstall" ? "uninstalled" : "done"); },
  error(e) { S.lastError = e; show("error"); },
  folder(path) { if (path) { S.opts.dir = path; } if (S.screen === "options") { readOpts(); show("options", false); $("#pick").focus(); } },
  running(isRunning) {
    if (S.screen !== "running") return;
    if (!isRunning) { S.closing = false; S.init.mode === "uninstall" ? show("uninstall") : show(S.init.existing ? "already" : "welcome"); }
    else { S.closing = false; $("#rstat").textContent = "Pliego"; $("#close-pliego").disabled = false; }
  },
};

/* ── Ventana sin marco ── */
$("#art").addEventListener("mousedown", (e) => { if (e.button === 0 && e.target.closest("[data-drag]")) send("drag"); });
$("#btn-min").addEventListener("click", () => send("minimize"));
$("#btn-close").addEventListener("click", () => send(S.screen === "installing" && S.doneKind !== "uninstall" ? "cancel" : "quit"));
$("#ov-close").addEventListener("click", hideLicense);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (!$("#overlay").hidden) return hideLicense();
    if (S.screen === "installing") return;
    send("quit");
  }
  if (e.key === "Tab" && !$("#overlay").hidden) { // el panel de licencia atrapa el foco
    const f = [$("#ov-body"), $("#ov-close")];
    const i = f.indexOf(document.activeElement);
    f[(i + (e.shiftKey ? -1 : 1) + f.length) % f.length].focus();
    e.preventDefault();
  }
});
document.addEventListener("contextmenu", (e) => e.preventDefault());
send("ready");
