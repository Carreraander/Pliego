// Modelo de objetos de JavaScript de formularios (campos, event, app, util, doc, color y funciones AF*).
// Escrito para Pliego a partir de la documentación pública del comportamiento; la lógica pesada (números, fechas,
// printf, máscaras) vive en Rust y se llama con `__h(op, ...)`.
"use strict";
(function (G) {
  var H = G.__h;
  delete G.__h;
  function def(o, k, v) {
    Object.defineProperty(o, k, { value: v, writable: true, configurable: true, enumerable: false });
  }
  function msg(key) {
    return H.apply(null, ["msg"].concat(Array.prototype.slice.call(arguments)));
  }

  // --- Colores -------------------------------------------------------------------------------------------------
  var color = {
    transparent: ["T"],
    black: ["G", 0],
    white: ["G", 1],
    red: ["RGB", 1, 0, 0],
    green: ["RGB", 0, 1, 0],
    blue: ["RGB", 0, 0, 1],
    cyan: ["CMYK", 1, 0, 0, 0],
    magenta: ["CMYK", 0, 1, 0, 0],
    yellow: ["CMYK", 0, 0, 1, 0],
    dkGray: ["G", 0.25],
    gray: ["G", 0.5],
    ltGray: ["G", 0.75],
  };
  function toRGB(c) {
    switch (c[0]) {
      case "G": return [c[1], c[1], c[1]];
      case "RGB": return [c[1], c[2], c[3]];
      case "CMYK": return [1 - Math.min(1, c[1] + c[4]), 1 - Math.min(1, c[2] + c[4]), 1 - Math.min(1, c[3] + c[4])];
      default: return [0, 0, 0];
    }
  }
  def(color, "convert", function (c, space) {
    if (!c || c[0] === space) return c;
    if (space === "T" || c[0] === "T") return space === "T" ? ["T"] : c;
    var rgb = toRGB(c);
    if (space === "RGB") return ["RGB", rgb[0], rgb[1], rgb[2]];
    if (space === "G") return ["G", 0.3 * rgb[0] + 0.59 * rgb[1] + 0.11 * rgb[2]];
    if (space === "CMYK") {
      var k = 1 - Math.max(rgb[0], rgb[1], rgb[2]);
      if (k >= 1) return ["CMYK", 0, 0, 0, 1];
      return ["CMYK", (1 - rgb[0] - k) / (1 - k), (1 - rgb[1] - k) / (1 - k), (1 - rgb[2] - k) / (1 - k), k];
    }
    return c;
  });
  def(color, "equal", function (a, b) {
    if (!a || !b) return false;
    if (a[0] !== b[0]) b = color.convert(b, a[0]);
    if (a.length !== b.length) return false;
    for (var i = 1; i < a.length; i++) if (Math.abs(a[i] - b[i]) > 1e-6) return false;
    return true;
  });
  def(G, "color", color);
  def(G, "display", { visible: 0, hidden: 1, noPrint: 2, noView: 3 });
  def(G, "border", { b: "beveled", d: "dashed", i: "inset", s: "solid", u: "underline" });
  def(G, "highlight", { n: "none", i: "invert", p: "push", o: "outline" });
  def(G, "position", { textOnly: 0, iconOnly: 1, iconTextV: 2, textIconV: 3, iconTextH: 4, textIconH: 5, overlay: 6 });
  def(G, "scaleHow", { proportional: 0, anamorphic: 1 });
  def(G, "scaleWhen", { always: 0, never: 1, tooBig: 2, tooSmall: 3 });
  def(G, "style", { ch: "check", ci: "circle", cr: "cross", di: "diamond", sq: "square", st: "star" });
  def(G, "font", { Times: "Times-Roman", TimesB: "Times-Bold", TimesI: "Times-Italic", TimesBI: "Times-BoldItalic",
    Helv: "Helvetica", HelvB: "Helvetica-Bold", HelvI: "Helvetica-Oblique", HelvBI: "Helvetica-BoldOblique",
    Cour: "Courier", CourB: "Courier-Bold", CourI: "Courier-Oblique", CourBI: "Courier-BoldOblique",
    Symbol: "Symbol", ZapfD: "ZapfDingbats" });
  def(G, "zoomType", { none: "NoVary", fitP: "FitPage", fitW: "FitWidth", fitH: "FitHeight", fitV: "FitVisibleWidth",
    pref: "Preferred", refW: "ReflowWidth" });

  // --- Campos ----------------------------------------------------------------------------------------------------
  function Field(name) {
    def(this, "_n", name);
  }
  var RO = ["name", "type", "valueAsString", "numItems", "page", "doc", "source", "calcOrderIndex"];
  var RW = ["value", "defaultValue", "readonly", "required", "display", "hidden", "textColor", "fillColor",
    "strokeColor", "bgColor", "fgColor", "textSize", "textFont", "alignment", "multiline", "password", "fileSelect",
    "doNotSpellCheck", "doNotScroll", "comb", "richText", "commitOnSelChange", "charLimit", "userName", "rect",
    "borderStyle", "lineWidth", "currentValueIndices", "exportValues", "editable", "multipleSelection", "sort",
    "buttonAlignX", "buttonAlignY", "buttonFitBounds", "buttonPosition", "buttonScaleHow", "buttonScaleWhen",
    "delay", "print", "submitName", "highlight", "radiosInUnison", "noExport", "defaultStyle", "rotation",
    "style", "comments"];
  RO.forEach(function (p) {
    Object.defineProperty(Field.prototype, p, {
      get: function () { return H("get", this._n, p); },
      enumerable: true, configurable: true,
    });
  });
  RW.forEach(function (p) {
    Object.defineProperty(Field.prototype, p, {
      get: function () { return H("get", this._n, p); },
      set: function (v) { H("set", this._n, p, v); },
      enumerable: true, configurable: true,
    });
  });
  var methods = {
    isBoxChecked: function (i) { return H("box", this._n, i | 0, "checked"); },
    isDefaultChecked: function (i) { return H("box", this._n, i | 0, "default"); },
    checkThisBox: function (i, on) { H("check", this._n, i | 0, on !== false, false); },
    defaultIsChecked: function (i, on) { H("check", this._n, i | 0, on !== false, true); },
    clearItems: function () { H("items", this._n, "clear"); },
    insertItemAt: function (label, exp, i) {
      H("items", this._n, "insert", String(label), exp === undefined ? String(label) : String(exp), i === undefined ? -1 : i | 0);
    },
    deleteItemAt: function (i) { H("items", this._n, "delete", i === undefined ? -1 : i | 0); },
    getItemAt: function (i, wantExport) { return H("items", this._n, "get", i | 0, wantExport !== false); },
    setItems: function (list) { H("items", this._n, "set", list); },
    setFocus: function () {},
    buttonSetCaption: function (s) { H("set", this._n, "buttonCaption", String(s)); },
    buttonGetCaption: function () { return H("get", this._n, "buttonCaption"); },
    buttonImportIcon: function () { return 0; },
    buttonSetIcon: function () {},
    setAction: function (trigger, script) { H("setaction", this._n, String(trigger), String(script)); },
    signatureValidate: function () { return 0; },
    signatureInfo: function () { return {}; },
    browseForFileToSubmit: function () { H("blocked", "browseForFileToSubmit", this._n); },
    getArray: function () {
      var names = H("children", this._n);
      return names.length ? names.map(getField) : [this];
    },
    getLock: function () { return null; },
    setLock: function () {},
  };
  Object.keys(methods).forEach(function (k) { def(Field.prototype, k, methods[k]); });
  def(Field.prototype, "toString", function () { return "[object Field " + this._n + "]"; });
  def(Field.prototype, "valueOf", function () { return this.value; });

  var cache = {};
  function getField(name) {
    name = String(name);
    if (!H("has", name)) return null;
    return cache[name] || (cache[name] = new Field(name));
  }

  // --- event -------------------------------------------------------------------------------------------------------
  var EVNAMES = { K: "Keystroke", F: "Format", V: "Validate", C: "Calculate", E: "MouseEnter", X: "MouseExit",
    D: "MouseDown", U: "MouseUp", A: "MouseUp", Fo: "Focus", Bl: "Blur", Open: "Open", WillClose: "WillClose",
    WillSave: "WillSave", DidSave: "DidSave", WillPrint: "WillPrint", DidPrint: "DidPrint", PageOpen: "Open", PageClose: "Close" };
  def(G, "__run", function (code, name, trigger, value, willCommit, change, selStart, selEnd, fieldFull) {
    var target = name == null ? null : getField(name);
    var ev = {
      name: EVNAMES[trigger] || trigger,
      type: name == null ? "Doc" : "Field",
      source: target, target: target, targetName: name == null ? "" : name,
      value: (trigger === "F" || trigger === "C") && target ? target.value : value,
      rc: true, willCommit: !!willCommit, change: change == null ? "" : change, changeEx: change == null ? "" : change,
      selStart: selStart | 0, selEnd: selEnd | 0, fieldFull: !!fieldFull, shift: false, modifier: false,
      commitKey: 0, keyDown: false, richValue: undefined, richChange: undefined, targetName2: undefined,
    };
    var saved = G.event;
    G.event = ev;
    var err = "";
    try {
      (new Function(code)).call(G);
    } catch (e) {
      err = e && e.message !== undefined ? String(e.name ? e.name + ": " + e.message : e.message) : String(e);
    }
    G.event = saved;
    return [ev.rc !== false, ev.value, ev.change, err];
  });
  def(G, "__global_eval", function (code) {
    try { (0, eval)(code); return ""; } catch (e) { return e && e.message !== undefined ? String(e.name + ": " + e.message) : String(e); }
  });
  G.event = { rc: true, value: "", change: "", willCommit: false, name: "", type: "" };

  // --- app ---------------------------------------------------------------------------------------------------------
  var timers = 0;
  var app = {
    viewerType: "Reader", viewerVariation: "Reader", viewerVersion: 11, platform: "WIN", language: H("lang"),
    formsVersion: 11, calculate: true, focusRect: true, fullscreen: false, runtimeHighlight: false,
    alert: function (msg, icon, type, title) {
      if (msg !== null && typeof msg === "object" && !Array.isArray(msg)) {
        title = msg.cTitle; type = msg.nType; icon = msg.nIcon; msg = msg.cMsg;
      }
      return H("alert", String(msg), icon | 0, type | 0, title == null ? "" : String(title));
    },
    beep: function () {},
    response: function () { return null; },
    setTimeOut: function (code) { timers++; return { _id: timers, _code: String(code) }; },
    setInterval: function (code) { timers++; return { _id: timers, _code: String(code) }; },
    clearTimeOut: function () {},
    clearInterval: function () {},
    execMenuItem: function (item) { H("blocked", "execMenuItem", String(item)); },
    launchURL: function (url) { H("blocked", "launchURL", String(url)); },
    mailMsg: function () { H("blocked", "mailMsg", ""); },
    openDoc: function (p) { H("blocked", "openDoc", typeof p === "string" ? p : JSON.stringify(p)); return null; },
    newDoc: function () { H("blocked", "newDoc", ""); return null; },
    popUpMenu: function () { return null; },
    addMenuItem: function () {}, addSubMenu: function () {}, addToolButton: function () {}, removeToolButton: function () {},
    trustedFunction: function (f) { return f; }, trustPropagatorFunction: function (f) { return f; },
    beginPriv: function () {}, endPriv: function () {},
    activeDocs: [], plugIns: [], constants: { align: { left: 0, center: 1, right: 2 } },
  };
  def(G, "app", app);

  // --- util --------------------------------------------------------------------------------------------------------
  function dateParts(d) {
    return [d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds()];
  }
  var PRINTD = { 0: "D:yyyymmddHHMMss", 1: "yyyy.mm.dd HH:MM:ss", 2: "m/d/yy h:MM:ss tt", 3: "mm/dd/yy" };
  var util = {
    printf: function (fmt) {
      return H("printf", String(fmt), Array.prototype.slice.call(arguments, 1));
    },
    printd: function (fmt, d, bXFAPicture) {
      if (!(d instanceof Date)) d = new Date(d);
      if (isNaN(d.getTime())) return "";
      if (typeof fmt === "number") fmt = PRINTD[fmt] || PRINTD[0];
      var p = dateParts(d);
      return H("fmt_date", String(fmt), p[0], p[1], p[2], p[3], p[4], p[5]);
    },
    scand: function (fmt, str) {
      var p = H("parse_date", String(str), String(fmt));
      return p ? new Date(p[0], p[1], p[2], p[3], p[4], p[5]) : null;
    },
    printx: function (fmt, str) {
      str = String(str); fmt = String(fmt);
      var out = "", j = 0, up = null;
      for (var i = 0; i < fmt.length; i++) {
        var f = fmt.charAt(i), c = str.charAt(j);
        var put = function (ch) { out += up === ">" ? ch.toUpperCase() : up === "<" ? ch.toLowerCase() : ch; };
        if (f === "\\") { out += fmt.charAt(++i); }
        else if (f === ">" || f === "<") up = f;
        else if (f === "=") up = null;
        else if (f === "?") { if (j < str.length) { put(c); j++; } }
        else if (f === "X") { while (j < str.length && !/[A-Za-z0-9]/.test(str.charAt(j))) j++; if (j < str.length) { put(str.charAt(j)); j++; } }
        else if (f === "A") { while (j < str.length && !/[A-Za-z]/.test(str.charAt(j))) j++; if (j < str.length) { put(str.charAt(j)); j++; } }
        else if (f === "9") { while (j < str.length && !/[0-9]/.test(str.charAt(j))) j++; if (j < str.length) { out += str.charAt(j); j++; } }
        else if (f === "*") { put(str.substring(j)); j = str.length; }
        else out += f;
      }
      return out;
    },
    byteToChar: function (n) { return String.fromCharCode(n & 255); },
    streamFromString: function (s) { return String(s); },
    stringFromStream: function (s) { return String(s); },
    crackURL: function (u) { return { cScheme: String(u).split(":")[0] }; },
    iconStreamFromIcon: function () { return null; },
    spansToXML: function (s) { return String(s); },
    xmlToSpans: function (s) { return []; },
  };
  def(G, "util", util);

  // --- console y global --------------------------------------------------------------------------------------------
  def(G, "console", {
    println: function (s) { H("console", String(s)); },
    show: function () {}, hide: function () {}, clear: function () {},
    log: function () { H("console", Array.prototype.slice.call(arguments).join(" ")); },
  });
  var persistent = {};
  def(G, "global", {
    setPersistent: function (n, v) { persistent[n] = v; },
    subscribe: function () {},
  });
  def(G, "Collab", { addStateModel: function () {}, removeStateModel: function () {} });
  def(G, "identity", { name: "", loginName: "", corporation: "", email: "" });
  def(G, "security", { handlers: [] });

  // --- Documento (this) --------------------------------------------------------------------------------------------
  var docApi = {
    getField: getField,
    getNthFieldName: function (i) { var n = H("names"); return n[i | 0] === undefined ? null : n[i | 0]; },
    resetForm: function (names) {
      H("reset", names === undefined ? null : [].concat(names).map(String));
    },
    calculateNow: function () { H("calcnow"); },
    getAnnots: function () { return []; },
    getAnnot: function () { return null; },
    getPageNumWords: function () { return 0; },
    getPageNthWord: function () { return ""; },
    gotoNamedDest: function () {}, getNamedDest: function () { return null; },
    getOCGs: function () { return []; }, setOCGOrder: function () {},
    submitForm: function (a) { H("blocked", "submitForm", typeof a === "string" ? a : JSON.stringify(a)); },
    mailDoc: function () { H("blocked", "mailDoc", ""); },
    mailForm: function () { H("blocked", "mailForm", ""); },
    print: function () { H("blocked", "print", ""); },
    saveAs: function () { H("blocked", "saveAs", ""); },
    exportAsText: function () { H("blocked", "exportAsText", ""); },
    exportAsFDF: function () { H("blocked", "exportAsFDF", ""); },
    exportAsXFDF: function () { H("blocked", "exportAsXFDF", ""); },
    importAnFDF: function () { H("blocked", "importAnFDF", ""); },
    importTextData: function () { H("blocked", "importTextData", ""); },
    importDataObject: function () { H("blocked", "importDataObject", ""); },
    getDataObject: function () { return null; },
    addScript: function (n, s) { (0, eval)(String(s)); },
    removeScript: function () {},
    addField: function () { return null; }, removeField: function () {},
    flattenPages: function () {}, deletePages: function () {}, insertPages: function () {}, extractPages: function () {},
    syncAnnotScan: function () {}, setPageLabels: function () {}, closeDoc: function () {},
    dirty: false, hidden: false, external: false, disclosed: true,
  };
  Object.keys(docApi).forEach(function (k) { def(G, k, docApi[k]); });
  [["numFields", function () { return H("names").length; }],
   ["numPages", function () { return H("info", "pages"); }],
   ["pageNum", function () { return 0; }],
   ["path", function () { return H("info", "file"); }],
   ["documentFileName", function () { return H("info", "file"); }],
   ["filesize", function () { return 0; }],
   ["URL", function () { return ""; }],
   ["info", function () { return { Title: H("info", "title"), title: H("info", "title") }; }],
   ["calculate", function () { return true; }],
  ].forEach(function (kv) {
    Object.defineProperty(G, kv[0], { get: kv[1], enumerable: false, configurable: true });
  });
  def(G, "fileName", H("info", "file"));
  def(G, "this_doc", G);

  // --- Funciones AF* (AForm) ---------------------------------------------------------------------------------------
  function isEmpty(v) { return v === undefined || v === null || v === ""; }
  function fail(text) { event.rc = false; app.alert(text, 0); }

  function AFMakeNumber(s) {
    if (typeof s === "number") return isNaN(s) ? null : s;
    return H("make_number", String(s));
  }
  function AFExtractNums(s) {
    var m = String(s).replace(/,/g, ".").match(/[+-]?\d+(\.\d+)?|[+-]?\.\d+/g);
    return m && m.length ? m : null;
  }
  function AFMergeChange(ev) {
    var v = String(ev.value), s = ev.selStart | 0, e = ev.selEnd | 0;
    if (s > e) { var t = s; s = e; e = t; }
    return v.substring(0, s) + String(ev.change) + v.substring(e);
  }

  function AFNumber_Format(nDec, sepStyle, negStyle, currStyle, strCurrency, bCurrencyPrepend) {
    var v = event.value;
    if (isEmpty(v)) { event.value = ""; return; }
    var n = AFMakeNumber(v);
    if (n === null) { event.value = ""; return; }
    nDec = Math.max(0, Math.min(12, Math.floor(nDec) || 0));
    var body = H("fmt_number", n, nDec, sepStyle | 0);
    var neg = body.charAt(0) === "-";
    if (neg) body = body.substring(1);
    var cur = strCurrency == null ? "" : String(strCurrency);
    var text = bCurrencyPrepend ? cur + body : body + cur;
    if (neg) text = (negStyle === 2 || negStyle === 3) ? "(" + text + ")" : "-" + text;
    if (negStyle === 1 || negStyle === 3) {
      var tgt = event.target;
      if (tgt) tgt.textColor = neg ? color.red : color.black;
    }
    event.value = text;
  }
  function AFNumber_Keystroke(nDec, sepStyle, negStyle, currStyle, strCurrency, bCurrencyPrepend) {
    var cur = strCurrency == null ? "" : String(strCurrency);
    if (event.willCommit) {
      if (isEmpty(event.value)) return;
      if (!H("is_numeric", String(event.value), cur)) fail(msg("num_invalid"));
    } else {
      var c = String(event.change);
      if (cur) c = c.split(cur).join("");
      if (c !== "" && !/^[0-9+\-.,'() % ]*$/.test(c)) event.rc = false;
    }
  }
  function AFPercent_Format(nDec, sepStyle) {
    var v = event.value;
    if (isEmpty(v)) { event.value = ""; return; }
    var n = AFMakeNumber(v);
    if (n === null) { event.value = ""; return; }
    var body = H("fmt_number", n * 100, Math.max(0, Math.min(12, Math.floor(nDec) || 0)), sepStyle | 0);
    event.value = body + "%";
  }
  function AFPercent_Keystroke(nDec, sepStyle) { AFNumber_Keystroke(nDec, sepStyle, 0, 0, "", true); }

  var DATE_FORMATS = ["m/d", "m/d/yy", "mm/dd/yy", "mm/yy", "d-mmm", "d-mmm-yy", "dd-mmm-yy", "yy-mm-dd", "mmm-yy",
    "mmmm-yy", "mmm d, yyyy", "mmmm d, yyyy", "m/d/yy h:MM tt", "m/d/yy HH:MM"];
  var TIME_FORMATS = ["HH:MM", "h:MM tt", "HH:MM:ss", "h:MM:ss tt"];
  function AFParseDateEx(s, fmt) { return util.scand(fmt, s); }
  function formatWith(fmt) {
    var v = event.value;
    if (isEmpty(v)) { event.value = ""; return; }
    var d = AFParseDateEx(v, fmt);
    event.value = d ? util.printd(fmt, d) : "";
  }
  function keystrokeWith(fmt, key) {
    if (!event.willCommit || isEmpty(event.value)) return;
    if (!AFParseDateEx(event.value, fmt)) {
      fail(msg(key, event.targetName, fmt));
    }
  }
  function AFDate_FormatEx(fmt) { formatWith(String(fmt)); }
  function AFDate_Format(i) { formatWith(DATE_FORMATS[i | 0] || DATE_FORMATS[0]); }
  function AFDate_KeystrokeEx(fmt) { keystrokeWith(String(fmt), "date_invalid"); }
  function AFDate_Keystroke(i) { keystrokeWith(DATE_FORMATS[i | 0] || DATE_FORMATS[0], "date_invalid"); }
  function AFTime_FormatEx(fmt) { formatWith(String(fmt)); }
  function AFTime_Format(i) { formatWith(TIME_FORMATS[i | 0] || TIME_FORMATS[0]); }
  function AFTime_KeystrokeEx(fmt) { keystrokeWith(String(fmt), "time_invalid"); }
  function AFTime_Keystroke(i) { keystrokeWith(TIME_FORMATS[i | 0] || TIME_FORMATS[0], "time_invalid"); }

  var SPECIAL_NAMES = ["99999", "99999-9999", "(999) 999-9999", "999-99-9999"];
  function AFSpecial_Format(psf) {
    var v = event.value;
    if (isEmpty(v)) { event.value = ""; return; }
    event.value = H("special", psf | 0, String(v));
  }
  function AFSpecial_Keystroke(psf) {
    psf = psf | 0;
    if (!event.willCommit) {
      if (!/^[0-9 ()+\-]*$/.test(String(event.change))) event.rc = false;
      return;
    }
    if (isEmpty(event.value)) return;
    var digits = String(event.value).replace(/\D/g, "");
    var ok = psf === 0 ? /^\d{5}$/.test(String(event.value)) :
      psf === 1 ? digits.length === 9 :
      psf === 2 ? (digits.length === 7 || digits.length === 10 || /^\+/.test(String(event.value))) :
      psf === 3 ? digits.length === 9 : true;
    if (!ok) fail(msg("special_invalid", SPECIAL_NAMES[psf] || ""));
  }
  function AFSpecial_KeystrokeEx(mask) {
    mask = String(mask);
    if (event.willCommit) {
      if (isEmpty(event.value)) return;
      if (!H("mask", mask, String(event.value), false)) fail(msg("special_invalid", mask));
    } else if (!H("mask", mask, AFMergeChange(event), true)) {
      event.rc = false;
    }
  }

  function AFRange_Validate(bGreaterThan, nGreaterThan, bLessThan, nLessThan) {
    if (isEmpty(event.value)) return;
    var n = AFMakeNumber(event.value);
    var bad = n === null ||
      (bGreaterThan && n < nGreaterThan) || (bLessThan && n > nLessThan);
    if (!bad) return;
    var a = H("num_text", nGreaterThan), b = H("num_text", nLessThan);
    fail(bGreaterThan && bLessThan ? msg("range_both", a, b) :
         bGreaterThan ? msg("range_min", a) : msg("range_max", b));
  }

  function AFSimple_Calculate(fn, fields) {
    if (typeof fields === "string") fields = fields.split(/\s*,\s*/);
    var vals = [];
    for (var i = 0; i < fields.length; i++) {
      var f = getField(String(fields[i]));
      if (!f) continue;
      var list = f.getArray();
      for (var k = 0; k < list.length; k++) {
        var raw = list[k].value;
        var n = (typeof raw === "number") ? raw : (isEmpty(raw) ? null : AFMakeNumber(raw));
        if (n !== null && !isNaN(n)) vals.push(n);
      }
    }
    var r = 0, j;
    switch (String(fn)) {
      case "SUM": for (j = 0; j < vals.length; j++) r += vals[j]; break;
      case "PRD": if (vals.length) { r = 1; for (j = 0; j < vals.length; j++) r *= vals[j]; } break;
      case "AVG": if (vals.length) { for (j = 0; j < vals.length; j++) r += vals[j]; r /= vals.length; } break;
      case "MIN": if (vals.length) { r = vals[0]; for (j = 1; j < vals.length; j++) if (vals[j] < r) r = vals[j]; } break;
      case "MAX": if (vals.length) { r = vals[0]; for (j = 1; j < vals.length; j++) if (vals[j] > r) r = vals[j]; } break;
      default: break;
    }
    event.value = r;
  }
  function AFSimple(fn, a, b) {
    switch (String(fn)) {
      case "AVG": return (a + b) / 2;
      case "SUM": return a + b;
      case "PRD": return a * b;
      case "MIN": return Math.min(a, b);
      case "MAX": return Math.max(a, b);
      default: return 0;
    }
  }
  function AFPercent_Calculate() {}
  function AFTrim(s) { return String(s).replace(/^\s+|\s+$/g, ""); }

  [AFMakeNumber, AFExtractNums, AFMergeChange, AFNumber_Format, AFNumber_Keystroke, AFPercent_Format,
   AFPercent_Keystroke, AFParseDateEx, AFDate_FormatEx, AFDate_Format, AFDate_KeystrokeEx, AFDate_Keystroke,
   AFTime_FormatEx, AFTime_Format, AFTime_KeystrokeEx, AFTime_Keystroke, AFSpecial_Format, AFSpecial_Keystroke,
   AFSpecial_KeystrokeEx, AFRange_Validate, AFSimple_Calculate, AFSimple, AFPercent_Calculate, AFTrim,
  ].forEach(function (f) { def(G, f.name, f); });
  // Nombres antiguos que Acrobat acepta.
  def(G, "AFNumber_FormatEx", AFNumber_Format);
  def(G, "AFDate_FormatExEx", AFDate_FormatEx);
})(globalThis);
