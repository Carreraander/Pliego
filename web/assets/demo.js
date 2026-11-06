/*
 * pdfpliego.com · interactive demo of the Pliego window.
 *
 * Progressive enhancement: without JavaScript the page shows the static window from the HTML; with it, this script
 * replaces it with a version you can use (tabs, six modes, Tools, Ctrl+K, panels, zoom and the signing wizard).
 * Everything is simulated in the browser: no file is opened or sent. No dependencies.
 *
 * Language: the page's <html lang> picks English or Spanish (`/` is English, `/es/` is Spanish).
 */
(() => {
  "use strict";
  const root = document.getElementById("demo");
  if (!root) return;

  const ES = (document.documentElement.lang || "en").toLowerCase().startsWith("es");
  /** Picks the string for the page language: L("English", "Español"). */
  const L = (en, es) => (ES ? es : en);
  const SIGNER = L("Maya Collins", "Lucía Ortega Ruiz");
  const WHEN = L("Nov 5, 2026 12:41", "05/11/2026 12:41");

  // ── Sample data ───────────────────────────────────────────────────────────────────────────────────────────
  // A line is a number (a text bar that wide, in %), a short text, or 0 (a gap).
  const DOCS = {
    contrato: {
      name: L("lease-agreement.pdf", "contrato-alquiler.pdf"),
      pages: [
        { title: L("Residential lease agreement", "Contrato de arrendamiento de vivienda"), lines: [100, 94, 97, 58, 100, 88, 70, 0, 100, 92, 45, 0, 100, 86] },
        { title: L("Terms and signature", "Cláusulas y firma"), lines: [100, 96, 91, 100, 62, 0, 100, 94, 80], sig: true },
        { title: L("Annex: inventory", "Anexo: inventario"), lines: L(["Three-seat sofa", "Dining table and 4 chairs", "Fridge", "Washing machine"], ["Sofá de tres plazas", "Mesa de comedor y 4 sillas", "Frigorífico", "Lavadora"]).concat([0, 100, 76]) },
      ],
    },
    factura: {
      name: L("invoice-0425.pdf", "factura-0425.pdf"),
      pages: [
        {
          title: L("Invoice no. 0425", "Factura n.º 0425"),
          lines: L(
            ["Lino Graphic Studio Ltd.", "Date: 4 Nov 2026", 0, "Cover design · €480.00", "Layout (96 pp.) · €720.00", "VAT 21% · €252.00", 0, "Total · €1,452.00", 0, 100, 64],
            ["Estudio Lino Gráfico, S. L.", "Fecha: 04/11/2026", 0, "Diseño de cubierta · 480,00 €", "Maquetación (96 págs.) · 720,00 €", "IVA 21 % · 252,00 €", 0, "Total · 1.452,00 €", 0, 100, 64],
          ),
        },
      ],
    },
  };
  const MODES = [
    ["leer", L("Read", "Leer")], ["comentar", L("Comment", "Comentar")], ["firmar", L("Fill and sign", "Rellenar y firmar")],
    ["organizar", L("Organize", "Organizar")], ["editar", L("Edit", "Editar")], ["proteger", L("Protect", "Proteger")],
  ];
  const TOOLS = {
    leer: [["sel", L("Select", "Seleccionar"), "cursor"], ["hand", L("Hand", "Mano"), "hand"], ["voice", L("Read aloud", "Leer en voz alta"), "volume"], ["pres", L("Presentation", "Presentación"), "screen"]],
    comentar: [["hl", L("Highlight", "Resaltar"), "hl"], ["note", L("Note", "Nota"), "comment"], ["stamp", L("Stamp", "Sello"), "stamp"]],
    firmar: [["fill", L("Fill fields", "Rellenar campos"), "form"], ["mysig", L("My signature", "Mi firma"), "sign"], ["digital", L("Digital signature…", "Firma digital…"), "cert"]],
    organizar: [["insert", L("Insert", "Insertar"), "plus"], ["rotate", L("Rotate", "Rotar"), "rotate"], ["dup", L("Duplicate", "Duplicar"), "copy"], ["del", L("Delete", "Eliminar"), "trash"]],
    editar: [["edittext", L("Edit text", "Editar texto"), "text"], ["wm", L("Watermark", "Marca de agua"), "drop"], ["hf", L("Header and footer", "Encabezado y pie"), "layers"], ["bates", "Bates", "hash"]],
    proteger: [["redact", L("Redact", "Redactar"), "redact"], ["pass", L("Password…", "Contraseña…"), "lock"], ["sanear", L("Sanitize…", "Sanear…"), "cert"]],
  };
  const DEFAULT_TOOL = { leer: "sel", comentar: "hl", firmar: "digital", organizar: null, editar: "edittext", proteger: "redact" };
  const COLORS = [["#ffd84d", L("Yellow", "Amarillo")], ["#7ed492", L("Green", "Verde")], ["#7cc3f5", L("Blue", "Azul")], ["#f59ac4", L("Pink", "Rosa")], ["#ffb35c", L("Orange", "Naranja")]];
  const TASKS = {
    comprimir: [L("Compress", "Comprimir"), L("compressed", "comprimido")],
    ocr: [L("Recognize text (OCR)", "Reconocer texto (OCR)"), "ocr"],
    pdfa: [L("Save as PDF/A", "Guardar como PDF/A"), "pdfa"],
    comparar: [L("Compare with the previous version", "Comparar con la versión anterior"), L("comparison", "comparacion")],
  };
  const G = {
    modes: L("Modes", "Modos"), goto: L("Go to", "Ir a"), recent: L("Recent", "Recientes"),
    fill: L("Fill and sign", "Rellenar y firmar"), org: L("Organize", "Organizar"), edit: L("Edit", "Editar"), prot: L("Protect", "Proteger"),
    ocr: L("Scan and OCR", "Escanear y OCR"), cmp: L("Compare and optimize", "Comparar y optimizar"), conv: L("Convert", "Convertir"),
  };

  // ── State ─────────────────────────────────────────────────────────────────────────────────────────────────
  const docState = (id) => ({
    pages: DOCS[id].pages.map((p, i) => ({ ...p, src: i, rot: 0, marks: {}, notes: [], title: p.title })),
    page: id === "contrato" ? 1 : 0, wm: false, hf: false, bates: false, signed: false, step: 3, protected: false, sel: [],
  });
  const S = {
    open: ["contrato", "factura"], doc: "contrato", home: false,
    // In the side-by-side hero the window is narrow: start with the thumbnails panel closed so the page has room.
    mode: "firmar", tool: "digital", panel: root.clientWidth >= 900 ? "thumbs" : null, zoom: 100, color: COLORS[0][0],
    docs: { contrato: docState("contrato"), factura: docState("factura") },
    palette: null, paletteSel: 0, toolsOpen: false, toast: "", task: null,
  };
  const D = () => S.docs[S.doc];

  // ── Helpers ───────────────────────────────────────────────────────────────────────────────────────────────
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const ic = (n) => `<svg aria-hidden="true"><use href="#i-${n}"/></svg>`;
  const norm = (s) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const base = () => DOCS[S.doc].name.replace(/\.pdf$/, "");
  let toastTimer = 0;
  const toast = (msg) => {
    S.toast = msg;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { S.toast = ""; softRender(); }, 3400);
  };

  // ── Commands (Ctrl+K palette and Tools) ───────────────────────────────────────────────────────────────────
  function commands() {
    const list = MODES.map(([id, label], i) => ({ g: G.modes, label, hint: `Alt+${i + 1}`, icon: "thumbs", run: () => setMode(id) }));
    list.push(
      { g: G.fill, label: L("Sign digitally", "Firmar digitalmente"), icon: "cert", run: () => setMode("firmar") },
      { g: G.org, label: L("Merge files", "Combinar archivos"), icon: "layers", run: () => { setMode("organizar"); toast(L("Drop other PDFs between the pages to merge them.", "Suelta otros PDF entre las páginas para combinarlos.")); } },
      { g: G.org, label: L("Rotate pages", "Rotar páginas"), icon: "rotate", run: () => setMode("organizar") },
      { g: G.edit, label: L("Watermark", "Marca de agua"), icon: "drop", run: () => { setMode("editar"); D().wm = true; } },
      { g: G.edit, label: L("Bates numbering", "Numeración Bates"), icon: "hash", run: () => { setMode("editar"); D().bates = true; } },
      { g: G.prot, label: L("Protect with a password", "Proteger con contraseña"), icon: "lock", run: () => { setMode("proteger"); S.tool = "pass"; } },
      { g: G.prot, label: L("Redact personal data", "Redactar datos personales"), icon: "redact", run: () => setMode("proteger") },
      { g: G.ocr, label: L("Recognize text (OCR)", "Reconocer texto (OCR)"), icon: "scan", run: () => runTask("ocr") },
      { g: G.cmp, label: L("Compress", "Comprimir"), icon: "zip", run: () => runTask("comprimir") },
      { g: G.cmp, label: L("Compare with the previous version", "Comparar con la versión anterior"), icon: "compare", run: () => runTask("comparar") },
      { g: G.conv, label: L("Save as PDF/A", "Guardar como PDF/A"), icon: "access", run: () => runTask("pdfa") },
      { g: G.conv, label: L("Export to Word", "Exportar a Word"), icon: "swap", run: () => toast(L("Export to Word: choose the folder and format in the task panel.", "Exportar a Word: elige carpeta y formato en el panel de la tarea.")) },
    );
    if (!S.home) D().pages.forEach((_, i) => list.push({ g: G.goto, label: `${L("Page", "Página")} ${i + 1}`, icon: "bookmark", run: () => { D().page = i; } }));
    Object.keys(DOCS).forEach((id) => list.push({ g: G.recent, label: DOCS[id].name, icon: "form", run: () => openDoc(id) }));
    return list;
  }

  // ── Actions ───────────────────────────────────────────────────────────────────────────────────────────────
  function setMode(m) {
    if (S.home) openDoc(S.doc in S.docs ? S.doc : "contrato");
    S.mode = m; S.tool = DEFAULT_TOOL[m]; S.toolsOpen = false; S.palette = null;
  }
  function openDoc(id) {
    if (!S.open.includes(id)) S.open.push(id);
    if (!S.docs[id]) S.docs[id] = docState(id);
    S.doc = id; S.home = false;
  }
  function closeDoc(id) {
    S.open = S.open.filter((d) => d !== id);
    delete S.docs[id];
    if (S.doc === id) { if (S.open.length) S.doc = S.open[S.open.length - 1]; else S.home = true; }
  }
  function runTask(key) {
    if (S.home) openDoc("contrato");
    if (S.task) return toast(L("A task is already running. You can keep working while it finishes.", "Ya hay una tarea en curso. Puedes seguir trabajando mientras termina."));
    const [label, suffix] = TASKS[key];
    const name = base();
    S.task = { label, pct: 0 }; S.palette = null; S.toolsOpen = false;
    const t = setInterval(() => {
      S.task.pct = Math.min(100, S.task.pct + 9);
      if (S.task.pct >= 100) {
        clearInterval(t); S.task = null;
        toast(key === "comparar"
          ? L("Comparison ready: 3 text changes on page 1.", "Comparación lista: 3 cambios de texto en la página 1.")
          : L(`Copy saved: ${name}-${suffix}.pdf. The original is untouched.`, `Copia guardada: ${name}-${suffix}.pdf. El original no se ha tocado.`));
      }
      softRender();
    }, 160);
  }
  function orgAction(a) {
    const d = D();
    const sel = d.sel.length ? d.sel : [d.page];
    if (a === "rotate") sel.forEach((i) => { d.pages[i].rot = (d.pages[i].rot + 90) % 360; });
    if (a === "dup") { const last = Math.max(...sel); d.pages.splice(last + 1, 0, ...sel.map((i) => ({ ...d.pages[i], marks: { ...d.pages[i].marks }, notes: [...d.pages[i].notes] }))); d.sel = []; }
    if (a === "insert") { d.pages.splice(Math.max(...sel) + 1, 0, { title: L("Blank page", "Página en blanco"), lines: [], src: -1, rot: 0, marks: {}, notes: [] }); d.sel = []; }
    if (a === "del") {
      if (sel.length >= d.pages.length) return toast(L("A PDF needs at least one page.", "Un PDF necesita al menos una página."));
      d.pages = d.pages.filter((_, i) => !sel.includes(i)); d.sel = [];
    }
    d.page = Math.min(d.page, d.pages.length - 1);
  }

  function act(a, v, ev) {
    const d = S.home ? null : D();
    switch (a) {
      case "tab": openDoc(v); break;
      case "close": closeDoc(v); break;
      case "newtab": S.home = true; break;
      case "reopen": openDoc(v); break;
      case "quick": {
        openDoc("contrato");
        if (v === "comprimir" || v === "ocr") runTask(v); else setMode(v);
        break;
      }
      case "mode": setMode(v); break;
      case "tools": S.toolsOpen = !S.toolsOpen; S.palette = null; break;
      case "cmdk": S.palette = S.palette === null ? "" : null; S.paletteSel = 0; S.toolsOpen = false; break;
      case "cmd": { const c = filtered()[+v]; S.palette = null; S.toolsOpen = false; c && c.run(); break; }
      case "tcmd": { const c = commands()[+v]; S.toolsOpen = false; c && c.run(); break; }
      case "tool":
        if (S.mode === "organizar") { orgAction(v); break; }
        if (S.mode === "editar" && v !== "edittext") { d[v] = !d[v]; break; }
        S.tool = v;
        if (S.mode === "leer" && v === "pres") toast(L("Presentation: in the app it fills the whole screen (F11).", "Presentación: en la app ocupa toda la pantalla (F11)."));
        if (S.mode === "leer" && v === "voice") toast(L("Read aloud uses the Windows voices.", "Leer en voz alta usa las voces de Windows."));
        break;
      case "rail": S.panel = S.panel === v ? null : v; break;
      case "page": d.page = +v; break;
      case "zoom": S.zoom = Math.max(50, Math.min(200, S.zoom + (v === "+" ? 25 : -25))); break;
      case "line": {
        const p = d.pages[d.page], k = +v;
        if (S.mode === "comentar") { p.marks[k] = p.marks[k] && p.marks[k] === S.color ? undefined : S.color; }
        if (S.mode === "proteger" && p.marks[k] !== "done") { p.marks[k] = p.marks[k] === "mark" ? undefined : "mark"; }
        break;
      }
      case "pageclick": {
        if (S.mode !== "comentar" || (S.tool !== "note" && S.tool !== "stamp")) return;
        const page = root.querySelector(".dpage");
        const b = page.getBoundingClientRect();
        D().pages[d.page].notes.push({ kind: S.tool, x: ((ev.clientX - b.left) / b.width) * 100, y: ((ev.clientY - b.top) / b.height) * 100 });
        break;
      }
      case "color": S.color = v; break;
      case "next": d.step = Math.min(4, d.step + 1); break;
      case "back": d.step = Math.max(1, d.step - 1); break;
      case "sign":
        d.signed = true; d.step = 5;
        d.page = d.pages.findIndex((p) => p.sig) >= 0 ? d.pages.findIndex((p) => p.sig) : d.page;
        toast(L(`Signed. Copy saved: ${base()}-signed.pdf`, `Firmado. Copia guardada: ${base()}-firmado.pdf`));
        break;
      case "resign": d.signed = false; d.step = 1; break;
      case "applyredact":
        d.pages.forEach((p) => Object.keys(p.marks).forEach((k) => { if (p.marks[k] === "mark") p.marks[k] = "done"; }));
        toast(L("Redaction applied: the marked text is removed from the file, not just covered.", "Redacción aplicada: el texto marcado se ha eliminado del archivo, no solo tapado."));
        break;
      case "protect": d.protected = !d.protected; if (d.protected) toast(L("Password protected (AES-256).", "Protegido con contraseña (AES-256).")); break;
      case "osel": {
        const i = +v;
        if (ev.ctrlKey || ev.metaKey || ev.shiftKey) d.sel = d.sel.includes(i) ? d.sel.filter((x) => x !== i) : [...d.sel, i];
        else d.sel = d.sel.length === 1 && d.sel[0] === i ? [] : [i];
        d.page = i;
        break;
      }
      default: return;
    }
    render();
  }

  // ── Rendering ─────────────────────────────────────────────────────────────────────────────────────────────
  const pageLabel = (i) => `${L("Page", "Página")} ${i + 1}`;

  function mini(p, i, on, attrs = "") {
    const bars = (p.lines.filter((l) => l !== 0).slice(0, 7)).map((l, k) =>
      `<i style="width:${typeof l === "number" ? l : 60 + ((k * 17) % 35)}%"></i>`).join("");
    return `<button type="button" class="mini-wrap${on ? " on" : ""}" ${attrs}>
      <span class="mini" style="transform:rotate(${p.rot}deg)"><i class="mt"></i>${bars}</span><span class="num">${i + 1}</span></button>`;
  }

  function lineHtml(p, l, k) {
    if (l === 0) return `<span class="gap"></span>`;
    const m = p.marks[k];
    const live = S.mode === "comentar" && S.tool === "hl" || S.mode === "proteger" && S.tool === "redact";
    const cls = m === "mark" ? " mark" : m === "done" ? " done" : m ? " hl" : "";
    const style = (typeof l === "number" ? `width:${l}%;` : "") + (m && m !== "mark" && m !== "done" ? `--hl:${m};` : "");
    const tag = live && m !== "done" ? `button type="button" data-a="line" data-v="${k}"` : "span";
    const close = tag.startsWith("button") ? "button" : "span";
    const label = typeof l === "number" ? "" : esc(l);
    return `<${tag} class="${typeof l === "number" ? "ln" : "tx"}${cls}" style="${style}"${live ? ` aria-label="${L("Line", "Línea")} ${k + 1}"` : ""}>${label}</${close}>`;
  }

  function pageHtml() {
    const d = D(), p = d.pages[d.page];
    const editable = S.mode === "editar" && S.tool === "edittext";
    const clickable = S.mode === "comentar" && (S.tool === "note" || S.tool === "stamp");
    const sig = p.sig ? (d.signed
      ? `<div class="sig in"><strong>✓ ${L("Digitally signed", "Firmado digitalmente")}</strong><span>${SIGNER} · ${WHEN}</span><span>${L("PAdES signature · valid certificate", "Firma PAdES · certificado válido")}</span></div>`
      : `<div class="sig-field${S.mode === "firmar" ? " live" : ""}"><span>${L("Signature field", "Campo de firma")}</span><small>${SIGNER}</small></div>`) : "";
    const notes = p.notes.map((n) => n.kind === "note"
      ? `<span class="pin" style="left:${n.x}%;top:${n.y}%">${ic("comment")}</span>`
      : `<span class="stampmark" style="left:${n.x}%;top:${n.y}%">${L("APPROVED", "APROBADO")}</span>`).join("");
    return `<div class="dpage${clickable ? " clickable" : ""}" ${clickable ? 'data-a="pageclick"' : ""}
        style="width:calc(min(80%, 420px) * ${S.zoom / 100});transform:rotate(${p.rot}deg)">
      ${d.hf ? `<span class="hf top">${esc(DOCS[S.doc].name)} · ${L("p.", "pág.")} ${d.page + 1}</span>` : ""}
      <b class="ptitle${editable ? " editable" : ""}" ${editable ? 'contenteditable="true" spellcheck="false" data-k="title"' : ""}>${esc(p.title)}</b>
      ${p.lines.map((l, k) => lineHtml(p, l, k)).join("")}
      ${sig}${notes}
      ${d.wm ? `<span class="wm">${L("DRAFT", "BORRADOR")}</span>` : ""}
      ${d.bates ? `<span class="hf bottom">PLG${String(d.page + 1).padStart(6, "0")}</span>` : ""}
      ${d.hf ? `<span class="hf bottom left">${L(`Page ${d.page + 1} of ${d.pages.length}`, `Página ${d.page + 1} de ${d.pages.length}`)}</span>` : ""}
    </div>`;
  }

  function orgHtml() {
    const d = D();
    return `<div class="orggrid">${d.pages.map((p, i) => mini(p, i, d.sel.includes(i), `data-a="osel" data-v="${i}" aria-pressed="${d.sel.includes(i)}" aria-label="${pageLabel(i)}"`)).join("")}</div>
      <p class="hint-floating">${L("Click to pick pages (Ctrl for several), then use Rotate, Duplicate or Delete above.", "Haz clic para elegir páginas (Ctrl para varias) y usa Rotar, Duplicar o Eliminar arriba.")}</p>`;
  }

  function homeHtml() {
    const q = [["firmar", L("Sign", "Firmar"), "cert"], ["organizar", L("Merge files", "Combinar archivos"), "layers"], ["ocr", L("Recognize text (OCR)", "Reconocer texto (OCR)"), "scan"],
      ["proteger", L("Protect", "Proteger"), "lock"], ["comprimir", L("Compress", "Comprimir"), "zip"], ["editar", L("Edit", "Editar"), "text"]];
    return `<div class="home">
      <div class="drop">${ic("down")}<b>${L("Drop a PDF here or click to open", "Suelta aquí un PDF o haz clic para abrir")}</b><small>${L("In this demo, open one of the recent files.", "En esta demo, abre uno de los recientes.")}</small></div>
      <div class="quick">${q.map(([v, l, i]) => `<button type="button" data-a="quick" data-v="${v}">${ic(i)}${l}</button>`).join("")}</div>
      <div class="recent"><span class="lbl">${G.recent}</span>${Object.keys(DOCS).map((id) => `<button type="button" data-a="reopen" data-v="${id}">${ic("form")}<span>${DOCS[id].name}</span><small>${L("Documents", "Documentos")}</small></button>`).join("")}</div>
    </div>`;
  }

  function sideHtml() {
    const d = D();
    if (S.panel === "thumbs") return `<div class="side thumbs-p">${d.pages.map((p, i) => mini(p, i, i === d.page, `data-a="page" data-v="${i}" aria-label="${pageLabel(i)}"`)).join("")}</div>`;
    if (S.panel === "bookmarks") return `<div class="side list-p"><span class="lbl">${L("Bookmarks", "Marcadores")}</span>${d.pages.map((p, i) => `<button type="button" data-a="page" data-v="${i}" class="${i === d.page ? "on" : ""}">${esc(p.title)}</button>`).join("")}</div>`;
    if (S.panel === "comments") {
      const items = [];
      d.pages.forEach((p, i) => {
        Object.values(p.marks).forEach((m) => { if (m && m !== "mark" && m !== "done") items.push(`<button type="button" data-a="page" data-v="${i}"><i class="dot" style="background:${m}"></i>${L("Highlight", "Resaltado")} · ${L("p.", "pág.")} ${i + 1}</button>`); });
        p.notes.forEach((n) => items.push(`<button type="button" data-a="page" data-v="${i}">${ic(n.kind === "note" ? "comment" : "stamp")}${n.kind === "note" ? L("Note", "Nota") : L("Stamp", "Sello")} · ${L("p.", "pág.")} ${i + 1}</button>`));
      });
      return `<div class="side list-p"><span class="lbl">${L("Comments", "Comentarios")}</span>${items.join("") || `<p class="empty">${L("No comments yet. Try Comment mode.", "Sin comentarios. Prueba el modo Comentar.")}</p>`}</div>`;
    }
    if (S.panel === "sigs") return `<div class="side list-p"><span class="lbl">${L("Signatures", "Firmas")}</span>${d.signed
      ? `<button type="button" data-a="page" data-v="${Math.max(0, d.pages.findIndex((p) => p.sig))}" class="sigrow">${ic("check")}<span>${SIGNER}<small>${L("Valid signature · PAdES", "Firma válida · PAdES")}</small></span></button>`
      : `<p class="empty">${L("This document has no signatures.", "Este documento no tiene firmas.")}</p>`}</div>`;
    return "";
  }

  function inspectorHtml() {
    const d = D();
    if (S.mode === "leer") return "";
    if (S.mode === "comentar") {
      const n = d.pages.reduce((a, p) => a + Object.values(p.marks).filter((m) => m && m !== "mark" && m !== "done").length + p.notes.length, 0);
      const hint = {
        hl: L("Click a line on the page to highlight it. Click again to remove it.", "Haz clic en una línea de la página para resaltarla. Otro clic la quita."),
        note: L("Click anywhere on the page to leave a note.", "Haz clic en cualquier punto de la página para dejar una nota."),
        stamp: L("Click the page to place the stamp.", "Haz clic en la página para poner el sello."),
      }[S.tool];
      const title = { hl: L("Highlight", "Resaltar"), note: L("Note", "Nota"), stamp: L("Stamp", "Sello") }[S.tool];
      return `<h4>${title}</h4>
        ${S.tool === "hl" ? `<div class="field">${L("Colour", "Color")}<div class="swatches">${COLORS.map(([c, l]) => `<button type="button" data-a="color" data-v="${c}" style="--c:${c}" class="${c === S.color ? "on" : ""}" aria-label="${l}" aria-pressed="${c === S.color}"></button>`).join("")}</div></div>` : ""}
        <p class="hint">${hint}</p><p class="hint num">${L(`${n} ${n === 1 ? "comment" : "comments"} in the document`, `${n} ${n === 1 ? "comentario" : "comentarios"} en el documento`)}</p>`;
    }
    if (S.mode === "firmar") {
      const h = `<h4>${L("Digital signature", "Firma digital")}</h4>`;
      if (!d.pages.some((p) => p.sig)) return `${h}<p class="hint">${L("This document has no signature field. In the app, you draw the box where you want to sign.", "Este documento no tiene campo de firma. En la app, dibujas el recuadro donde quieras firmar.")}</p><button type="button" class="ibtn" data-a="tab" data-v="contrato">${L("Open the lease", "Abrir el contrato")}</button>`;
      if (d.signed) return `${h}<div class="okbox">${ic("check")}<span><b>${L("Document signed", "Documento firmado")}</b><small>${L("Valid PAdES signature. A signed copy was saved.", "Firma PAdES válida. Se guardó una copia firmada.")}</small></span></div>
        <button type="button" class="ibtn quiet" data-a="rail" data-v="sigs">${L("View signatures", "Ver firmas")}</button><button type="button" class="ibtn quiet" data-a="resign">${L("Run the demo again", "Repetir la demo")}</button>`;
      const steps = [L("Certificate", "Certificado"), L("Where to sign", "Dónde firmar"), L("Look and reason", "Aspecto y motivo"), L("Sign and save", "Firmar y guardar")];
      const body = [
        `<div class="field">${L("Certificate", "Certificado")}<div>${ic("cert")}${SIGNER}</div></div><p class="hint">${L("From the Windows certificate store or a .p12 / .pfx file.", "Del almacén de Windows o de un archivo .p12 / .pfx.")}</p>`,
        `<div class="field">${L("Field", "Campo")}<div>${ic("sign")}${L("Page 2 · signature field", "Página 2 · campo de firma")}</div></div>`,
        `<div class="field">${L("Reason", "Motivo")}<div>${L("I agree to this lease", "Conforme con el contrato")}</div></div><div class="field">${L("Level", "Nivel")}<div>${L("PAdES B-LT (with timestamp)", "PAdES B-LT (con sello de tiempo)")}</div></div>`,
        `<p class="hint">${L("A signed copy will be saved. The original does not change.", "Se guardará una copia firmada. El original no cambia.")}</p>`,
      ][d.step - 1];
      return `${h}
        <ol class="steps">${steps.map((s, i) => `<li class="${i + 1 < d.step ? "done" : i + 1 === d.step ? "on" : ""}" data-n="${i + 1}">${s}</li>`).join("")}</ol>
        ${body}
        <div class="ibtns">${d.step > 1 ? `<button type="button" class="ibtn quiet" data-a="back">${L("Back", "Atrás")}</button>` : ""}
        ${d.step < 4 ? `<button type="button" class="ibtn" data-a="next">${L("Next", "Siguiente")}</button>` : `<button type="button" class="ibtn" data-a="sign">${L("Sign and save", "Firmar y guardar")}</button>`}</div>`;
    }
    if (S.mode === "organizar") {
      return `<h4>${L("Organize", "Organizar")}</h4><p class="hint num">${L(`${d.pages.length} pages · ${d.sel.length || 0} selected`, `${d.pages.length} páginas · ${d.sel.length || 0} seleccionadas`)}</p>
        <p class="hint">${L("Rotate, duplicate and delete act on the pages you pick, or on the current page if you pick none.", "Rotar, duplicar o eliminar actúan sobre las páginas elegidas. Si no eliges ninguna, sobre la actual.")}</p>`;
    }
    if (S.mode === "editar") {
      const tg = (k, l) => `<button type="button" class="toggle-row" data-a="tool" data-v="${k}" aria-pressed="${d[k]}"><span class="tg${d[k] ? " on" : ""}"></span>${l}</button>`;
      return `<h4>${L("Edit", "Editar")}</h4><p class="hint">${L("Click the page title and type.", "Haz clic en el título de la página y escribe.")}</p>
        <div class="field">${L("Font", "Fuente")}<div>Segoe UI · 18 pt · Semibold</div></div>
        ${tg("wm", L("Watermark", "Marca de agua"))}${tg("hf", L("Header and footer", "Encabezado y pie"))}${tg("bates", L("Bates numbering", "Numeración Bates"))}`;
    }
    if (S.mode === "proteger") {
      const n = d.pages.reduce((a, p) => a + Object.values(p.marks).filter((m) => m === "mark").length, 0);
      if (S.tool === "pass") return `<h4>${L("Password", "Contraseña")}</h4><div class="field">${L("Password to open", "Contraseña para abrir")}<div class="dots">••••••••••</div></div>
        <div class="field">${L("Encryption", "Cifrado")}<div>AES-256</div></div><button type="button" class="ibtn" data-a="protect">${d.protected ? L("Remove protection", "Quitar protección") : L("Protect", "Proteger")}</button>`;
      if (S.tool === "sanear") return `<h4>${L("Sanitize document", "Sanear documento")}</h4><p class="hint">${L("Removes metadata, JavaScript, attachments and hidden layers. Saves a copy.", "Quita metadatos, JavaScript, archivos adjuntos y capas ocultas. Guarda una copia.")}</p><button type="button" class="ibtn" data-a="tool" data-v="redact">${L("Back to Redact", "Volver a Redactar")}</button>`;
      return `<h4>${L("Redact", "Redactar")}</h4><p class="hint">${L("Click the lines that contain personal data to mark them.", "Haz clic en las líneas que contengan datos personales para marcarlas.")}</p>
        <button type="button" class="ibtn danger" data-a="applyredact" ${n ? "" : "disabled"}>${L("Apply redaction", "Aplicar redacción")}${n ? ` (${n})` : ""}</button>
        <p class="hint">${L("Applying removes the text from the file, not just covers it.", "Aplicar elimina el texto del archivo, no solo lo tapa.")}</p>`;
    }
    return "";
  }

  function filtered() {
    const q = norm(S.palette || "");
    return commands().filter((c) => !q || norm(c.label + " " + c.g).includes(q)).slice(0, 8);
  }

  function paletteHtml() {
    if (S.palette === null) return "";
    const list = filtered();
    S.paletteSel = Math.min(S.paletteSel, Math.max(0, list.length - 1));
    const lbl = L("Search or run", "Buscar o ejecutar");
    return `<div class="dpalette" role="dialog" aria-label="${lbl}">
      <div class="palette-q">${ic("search")}<input data-k="palette" type="text" value="${esc(S.palette)}" placeholder="${L("Find a tool, page or document", "Busca una herramienta, página o documento")}" aria-label="${lbl}" autocomplete="off"></div>
      <div class="dpal-list">${list.map((c, i) => `<button type="button" class="palette-row${i === S.paletteSel ? " on" : ""}" data-a="cmd" data-v="${i}">${ic(c.icon)}<div><b>${esc(c.label)}</b><small>${esc(c.g)}</small></div>${c.hint ? `<span class="kbd">${c.hint}</span>` : ""}</button>`).join("") || `<p class="empty">${L(`Nothing matches “${esc(S.palette)}”.`, `Nada con «${esc(S.palette)}».`)}</p>`}</div>
    </div>`;
  }

  function toolsHtml() {
    if (!S.toolsOpen) return "";
    const groups = {};
    commands().forEach((c, i) => { if (c.g !== G.modes && c.g !== G.goto && c.g !== G.recent) (groups[c.g] ||= []).push([c, i]); });
    return `<div class="dtools" role="dialog" aria-label="${L("All tools", "Todas las herramientas")}">${Object.entries(groups).map(([g, items]) =>
      `<div><span class="lbl">${g}</span>${items.map(([c, i]) => `<button type="button" data-a="tcmd" data-v="${i}">${ic(c.icon)}${esc(c.label)}</button>`).join("")}</div>`).join("")}</div>`;
  }

  const toastHtml = () => (S.toast ? `<span>${esc(S.toast)}</span>` : "");
  function statusHtml() {
    const d = S.home ? null : D();
    const pageInfo = d ? L(`Page ${d.page + 1} of ${d.pages.length}`, `Pág. ${d.page + 1} de ${d.pages.length}`) : L("Home", "Inicio");
    const state = !d ? "" : d.signed ? `<span class="ok">${ic("check")}${L("Signed · valid", "Firmado · válido")}</span>`
      : d.protected ? `<span>${ic("lock")}${L("Protected", "Protegido")}</span>` : `<span class="muted">${L("Not signed", "Sin firmar")}</span>`;
    return `<div class="win-status">
        <span class="num">${pageInfo}</span>${d ? `<span class="num hide-s">A4 · 210 × 297 mm</span>` : ""}${state}
        ${S.task ? `<span class="task">${ic("refresh")}<span class="hide-s">${esc(S.task.label)}</span><i class="bar"><i style="width:${S.task.pct}%"></i></i><span class="num">${S.task.pct} %</span></span>` : ""}
        <span class="push num hide-s">${d ? S.zoom + " %" : ""}</span>
      </div>`;
  }

  function render() {
    const active = document.activeElement;
    const focusKey = active && root.contains(active) ? (active.dataset.k || (active.dataset.a ? `${active.dataset.a}|${active.dataset.v || ""}` : "")) : "";
    const d = S.home ? null : D();
    const hasInspector = !S.home && S.mode !== "leer";
    const hasSide = !S.home && S.panel;
    const narrow = root.clientWidth < 760;
    const cols = S.home ? "minmax(0,1fr)" : `48px ${hasSide ? (narrow ? "120px " : "150px ") : ""}minmax(0,1fr)${hasInspector ? (narrow ? " 220px" : " 248px") : ""}`;
    const tools = S.home ? [] : TOOLS[S.mode];

    root.innerHTML = `
      <div class="win-title">
        <svg class="logo" aria-hidden="true"><use href="#logo"/></svg>
        <div class="tabs" role="tablist" aria-label="${L("Open documents", "Documentos abiertos")}">
          ${S.open.map((id) => `<span class="tab${!S.home && id === S.doc ? " on" : ""}"><button type="button" role="tab" aria-selected="${!S.home && id === S.doc}" data-a="tab" data-v="${id}">${DOCS[id].name}</button><button type="button" class="x" data-a="close" data-v="${id}" aria-label="${L("Close", "Cerrar")} ${DOCS[id].name}">${ic("x")}</button></span>`).join("")}
          <button type="button" class="tab-new${S.home ? " on" : ""}" data-a="newtab" aria-label="${L("New tab", "Pestaña nueva")}">${ic("plus")}</button>
        </div>
        <button type="button" class="cmdk" data-a="cmdk" aria-label="${L("Search or run (Ctrl+K)", "Buscar o ejecutar (Ctrl+K)")}">${ic("search")}<span class="t">${L("Search or run…", "Buscar o ejecutar…")}</span><kbd>Ctrl K</kbd></button>
        <span class="winbtns" aria-hidden="true"><span>${ic("min")}</span><span>${ic("max")}</span><span>${ic("x")}</span></span>
      </div>
      <div class="win-modes">
        <div class="modes-scroll" role="toolbar" aria-label="${L("Modes", "Modos")}">
          ${MODES.map(([id, l]) => `<button type="button" class="mode${!S.home && S.mode === id ? " on" : ""}" data-a="mode" data-v="${id}" aria-pressed="${!S.home && S.mode === id}">${l}</button>`).join("")}
          <span class="mode-sep"></span>
          <button type="button" class="mode${S.toolsOpen ? " on" : ""}" data-a="tools" aria-expanded="${S.toolsOpen}">${L("Tools", "Herramientas")} ${ic("chev")}</button>
        </div>
        <div class="mode-tools" role="toolbar" aria-label="${L("Mode tools", "Herramientas del modo")}">
          ${tools.map(([id, l, i]) => {
            const on = S.mode === "editar" && id !== "edittext" ? d[id] : S.tool === id;
            return `<button type="button" class="${on ? "on" : ""}" data-a="tool" data-v="${id}" title="${l}" aria-pressed="${!!on}">${ic(i)}<span>${l}</span></button>`;
          }).join("")}
        </div>
      </div>
      <div class="win-body" style="grid-template-columns:${cols}">
        ${S.home ? "" : `<div class="rail" role="toolbar" aria-label="${L("Panels", "Paneles")}">
          ${[["thumbs", L("Thumbnails", "Miniaturas"), "thumbs"], ["bookmarks", L("Bookmarks", "Marcadores"), "bookmark"], ["comments", L("Comments", "Comentarios"), "comment"], ["sigs", L("Signatures", "Firmas"), "cert"]].map(([id, l, i]) => `<button type="button" class="${S.panel === id ? "on" : ""}" data-a="rail" data-v="${id}" aria-label="${l}" aria-pressed="${S.panel === id}" title="${l}">${ic(i)}</button>`).join("")}
        </div>${sideHtml()}`}
        <div class="canvas${S.mode === "organizar" && !S.home ? " org" : ""}">
          ${S.home ? homeHtml() : S.mode === "organizar" ? orgHtml() : pageHtml()}
          ${!S.home && S.mode !== "organizar" ? `<div class="zoom"><button type="button" data-a="zoom" data-v="-" aria-label="${L("Zoom out", "Alejar")}">−</button><span class="num">${S.zoom} %</span><button type="button" data-a="zoom" data-v="+" aria-label="${L("Zoom in", "Acercar")}">+</button></div>` : ""}
          <div class="dtoast" role="status" aria-live="polite">${toastHtml()}</div>
        </div>
        ${hasInspector ? `<div class="inspector">${inspectorHtml()}</div>` : ""}
      </div>
      ${statusHtml()}
      ${paletteHtml()}${toolsHtml()}`;

    // Restore focus after repainting.
    if (S.palette !== null) {
      const inp = root.querySelector('[data-k="palette"]');
      inp.focus({ preventScroll: true }); inp.setSelectionRange(inp.value.length, inp.value.length);
    } else if (focusKey) {
      const [a, v] = focusKey.split("|");
      const el = root.querySelector(v !== undefined ? `[data-a="${a}"][data-v="${v}"]` : `[data-k="${focusKey}"]`) || root.querySelector(`[data-a="${a}"]`);
      if (el && focusKey !== "title") el.focus({ preventScroll: true });
    }
  }

  // While typing in the page title, do not repaint everything (the caret would be lost): only status and toast.
  function softRender() {
    const a = document.activeElement;
    if (!(a && a.dataset && a.dataset.k === "title")) return render();
    root.querySelector(".win-status").outerHTML = statusHtml();
    root.querySelector(".dtoast").innerHTML = toastHtml();
  }

  // ── Events ────────────────────────────────────────────────────────────────────────────────────────────────
  root.addEventListener("click", (e) => {
    const b = e.target.closest("[data-a]");
    if (!b || !root.contains(b) || b.disabled) {
      if ((S.palette !== null || S.toolsOpen) && !e.target.closest(".dpalette, .dtools")) { S.palette = null; S.toolsOpen = false; render(); }
      return;
    }
    if (b.dataset.a === "pageclick" && e.target.closest(".ptitle")) return;
    act(b.dataset.a, b.dataset.v, e);
  });
  root.addEventListener("input", (e) => {
    if (e.target.dataset.k === "palette") { S.palette = e.target.value; S.paletteSel = 0; render(); }
    if (e.target.dataset.k === "title") { D().pages[D().page].title = e.target.textContent.slice(0, 80); }
  });
  root.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") { e.preventDefault(); act("cmdk"); return; }
    if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "a") { e.preventDefault(); act("tools"); return; }
    if (e.altKey && /^[1-6]$/.test(e.key)) { e.preventDefault(); setMode(MODES[+e.key - 1][0]); render(); return; }
    if (e.key === "Escape" && (S.palette !== null || S.toolsOpen)) { S.palette = null; S.toolsOpen = false; render(); root.querySelector(".cmdk").focus(); return; }
    if (e.target.dataset.k === "title" && e.key === "Enter") { e.preventDefault(); e.target.blur(); }
    if (S.palette !== null && e.target.dataset.k === "palette") {
      const n = filtered().length;
      if (e.key === "ArrowDown") { e.preventDefault(); S.paletteSel = (S.paletteSel + 1) % Math.max(1, n); render(); }
      if (e.key === "ArrowUp") { e.preventDefault(); S.paletteSel = (S.paletteSel - 1 + n) % Math.max(1, n); render(); }
      if (e.key === "Enter") { e.preventDefault(); act("cmd", String(S.paletteSel)); }
    }
  });

  // Repaint when the window crosses the narrow breakpoint (the hero switches between side-by-side and stacked).
  let wasNarrow = root.clientWidth < 760;
  if ("ResizeObserver" in window) {
    new ResizeObserver(() => {
      const n = root.clientWidth < 760;
      if (n !== wasNarrow) { wasNarrow = n; render(); }
    }).observe(root);
  }

  root.classList.add("live");
  root.setAttribute("aria-label", L("Interactive Pliego demo", "Demostración interactiva de Pliego"));
  root.setAttribute("role", "region");
  render();
  const hint = document.getElementById("demo-hint");
  if (hint) hint.hidden = false;
})();
