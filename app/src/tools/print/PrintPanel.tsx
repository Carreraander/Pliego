import { createEffect, createMemo, createResource, createSignal, on, Show } from "solid-js";
import { tileUrl } from "../../api";
import { currentPageIndex, activeDoc } from "../../state/documents";
import { Checkbox, Field, Notice, Radio, RadioGroup, Select, TaskPanel, TextInput } from "../../ui";
import { errorText, fileName, runToolJob } from "../run";
import { printApi, type Placement, type PrintOptions, type SheetMetrics } from "./api";
import {
  defaultOptions, landscapeSheet, loadRemembered, pagesExpression, paperSizeMm, rangeError, remember, selectedPages, thumbZoom,
  type PageChoice,
} from "./options";
import { pt } from "./strings";
import "./print.css";

const A4: SheetMetrics = { dpiX: 72, dpiY: 72, physW: 595, physH: 842, offX: 0, offY: 0, areaW: 595, areaH: 842 };

/**
 * Imprimir: impresora, páginas, copias, ajuste a la hoja, color y comentarios, con una vista previa de cómo cae la página
 * en la hoja (las mismas cuentas que la impresión real, que se piden al motor). El envío es un trabajo en segundo plano:
 * `pliego printer send` en un proceso hijo, con su progreso y su «Cancelar» en el centro de tareas.
 */
export default function PrintPanel(p: { onClose: () => void }) {
  const doc = activeDoc;
  const [options, setOptions] = createSignal<PrintOptions>({ ...defaultOptions(), ...loadRemembered() });
  const patch = (changes: Partial<PrintOptions>) => setOptions((o) => ({ ...o, ...changes }));
  const [choice, setChoice] = createSignal<PageChoice>("all");
  const [range, setRange] = createSignal("");
  const pageCount = () => doc()?.sizes.length ?? 0;
  const currentPage = () => Math.min(currentPageIndex(), Math.max(pageCount() - 1, 0)) + 1;

  // Impresoras y la elegida: la de la última vez si sigue ahí; si no, la predeterminada.
  const [printers] = createResource(() => printApi.printers().catch((e) => { throw new Error(errorText(e)); }));
  const printerName = createMemo(() => {
    const list = printers();
    if (!list || list.length === 0) return null;
    const wanted = options().printer;
    return list.find((x) => x.name === wanted)?.name ?? list.find((x) => x.isDefault)?.name ?? list[0]?.name ?? null;
  });
  const [caps] = createResource(printerName, (name) => printApi.caps(name).catch(() => null));
  // Si el papel recordado ya no existe en esta impresora, vale el de la impresora.
  createEffect(on(caps, (c) => {
    const paper = options().paper;
    if (c && paper !== null && !c.papers.some((x) => x.id === paper)) patch({ paper: null });
  }));

  const expression = () => pagesExpression(choice(), currentPage(), range());
  const rangeProblem = () => (choice() === "range" ? rangeError(range(), pageCount()) : null);
  const pages = createMemo(() => (rangeProblem() ? [] : selectedPages(expression(), options().subset, options().reverse, pageCount())));
  const first = () => doc()?.sizes[pages()[0] ?? 0];
  const landscape = () => landscapeSheet(options().orientation, first());

  // Hoja de la impresora para el papel y la orientación elegidos (si no responde, un A4 sin márgenes).
  const [sheet] = createResource(
    () => (printerName() ? { name: printerName()!, paper: options().paper, landscape: landscape() } : null),
    (k) => printApi.sheet(k.name, k.paper, k.landscape).catch(() => null),
  );
  const sheetMetrics = (): SheetMetrics => {
    const s = sheet();
    if (s) return s;
    const [w, h] = paperSizeMm(210, 297, landscape());
    return { ...A4, physW: (w / 25.4) * 72, physH: (h / 25.4) * 72, areaW: (w / 25.4) * 72, areaH: (h / 25.4) * 72 };
  };

  const [placement] = createResource(
    () => {
      const f = first();
      return f && pages().length > 0 ? { w: f.width, h: f.height, sheet: sheetMetrics(), options: { ...options(), printer: printerName() } } : null;
    },
    (k) => printApi.layout(k.w, k.h, k.sheet, k.options).catch(() => null),
  );

  const summary = () => pt("print.summary", { pages: pages().length, total: pageCount(), copies: options().copies });
  const canPrint = () => !!doc() && !!printerName() && pages().length > 0 && !rangeProblem() && options().copies >= 1 && options().copies <= 999;

  const apply = async () => {
    const d = doc();
    if (!d || !canPrint()) return;
    const request = { path: d.path, options: { ...options(), printer: printerName(), pages: expression() } };
    remember(request.options);
    const id = await runToolJob({
      command: "print_document",
      args: { request },
      title: pt("print.job", { name: fileName(d.path) }),
      started: pt("print.started"),
    });
    if (id !== null) p.onClose();
  };

  const rangeText = () => {
    const e = rangeProblem();
    return e ? pt(`print.err.${e}`, { n: pageCount() }) : undefined;
  };
  const num = (v: string, min: number, max: number, fallback: number) => {
    const n = Math.round(Number(v));
    return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
  };

  return (
    <TaskPanel
      title={pt("print.title")}
      onClose={p.onClose}
      estimate={doc() ? summary() : undefined}
      primary={{ label: pt("print.action"), icon: "printer", onClick: () => void apply(), disabled: !canPrint() }}
      advanced={
        <>
          <Field label={pt("print.paper")}>
            <Select
              value={options().paper === null ? "" : String(options().paper)}
              onChange={(e) => patch({ paper: e.currentTarget.value === "" ? null : Number(e.currentTarget.value) })}
              options={[
                { value: "", label: pt("print.paperDefault") },
                ...(caps()?.papers ?? []).map((x) => ({ value: String(x.id), label: `${x.name} (${Math.round(x.widthMm)} × ${Math.round(x.heightMm)} mm)` })),
              ]}
            />
          </Field>
          <Field label={pt("print.orientation")}>
            <Select
              value={options().orientation}
              onChange={(e) => patch({ orientation: e.currentTarget.value as PrintOptions["orientation"] })}
              options={[
                { value: "auto", label: pt("print.orientAuto") },
                { value: "portrait", label: pt("print.orientPortrait") },
                { value: "landscape", label: pt("print.orientLandscape") },
              ]}
            />
          </Field>
          <Checkbox label={pt("print.autoRotate")} checked={options().autoRotate} onChange={(e) => patch({ autoRotate: e.currentTarget.checked })} />
          <Field label={pt("print.duplex")} hint={caps() && !caps()!.duplex ? pt("print.noDuplex") : undefined}>
            <Select
              value={options().duplex}
              disabled={caps() ? !caps()!.duplex : false}
              onChange={(e) => patch({ duplex: e.currentTarget.value as PrintOptions["duplex"] })}
              options={[
                { value: "off", label: pt("print.duplexOff") },
                { value: "longEdge", label: pt("print.duplexLong") },
                { value: "shortEdge", label: pt("print.duplexShort") },
              ]}
            />
          </Field>
          <Field label={pt("print.subset")}>
            <Select
              value={options().subset}
              onChange={(e) => patch({ subset: e.currentTarget.value as PrintOptions["subset"] })}
              options={[
                { value: "all", label: pt("print.subsetAll") },
                { value: "odd", label: pt("print.subsetOdd") },
                { value: "even", label: pt("print.subsetEven") },
              ]}
            />
          </Field>
          <Checkbox label={pt("print.reverse")} checked={options().reverse} onChange={(e) => patch({ reverse: e.currentTarget.checked })} />
        </>
      }
    >
      <Show when={!doc()}><Notice tone="warn" inline>{pt("print.noDoc")}</Notice></Show>
      <Show when={printers.error}>
        <Notice tone="warn" inline>{pt("print.unavailable", { error: (printers.error as Error).message })}</Notice>
      </Show>
      <Show when={printers() && printers()!.length === 0}><Notice tone="warn" inline>{pt("print.none")}</Notice></Show>

      <Preview
        sheet={sheetMetrics()} placement={placement() ?? null} label={pt("print.preview")}
        src={doc() && pages().length > 0 && first() ? tileUrl(doc()!.id, pages()[0] ?? 0, thumbZoom(first()!.width, first()!.height), 0, 0, doc()!.revision) : null}
        alt={pt("print.previewAlt", { n: (pages()[0] ?? 0) + 1 })}
      />

      <Field label={pt("print.printer")}>
        <Select
          value={printerName() ?? ""}
          disabled={!printers() || printers()!.length === 0}
          onChange={(e) => patch({ printer: e.currentTarget.value })}
          options={(printers() ?? []).map((x) => ({ value: x.name, label: x.isDefault ? pt("print.default", { name: x.name }) : x.name }))}
        />
      </Field>

      <RadioGroup legend={pt("print.pages")} value={choice()} onChange={(v) => setChoice(v as PageChoice)}>
        <Radio value="all" label={pt("print.pagesAll", { n: pageCount() })} />
        <Radio value="current" label={pt("print.pagesCurrent", { n: currentPage() })} />
        <Radio value="range" label={pt("print.pagesRange")} />
      </RadioGroup>
      <Show when={choice() === "range"}>
        <Field label={pt("print.rangeLabel")} error={range() === "" ? undefined : rangeText()}>
          <TextInput
            value={range()} placeholder={pt("print.rangePlaceholder")} autofocus
            onInput={(e) => setRange(e.currentTarget.value)}
          />
        </Field>
      </Show>
      <Show when={pages().length === 0 && !rangeProblem() && doc()}><Notice tone="warn" inline>{pt("print.noPages")}</Notice></Show>

      <div class="print-copies">
        <Field label={pt("print.copies")}>
          <TextInput
            type="number" min={1} max={999} value={options().copies}
            onChange={(e) => patch({ copies: num(e.currentTarget.value, 1, 999, 1) })}
          />
        </Field>
      </div>
      <Show when={options().copies > 1}>
        <Checkbox label={pt("print.collate")} checked={options().collate} onChange={(e) => patch({ collate: e.currentTarget.checked })} />
      </Show>

      <Field label={pt("print.scale")}>
        <Select
          value={options().scale}
          onChange={(e) => patch({ scale: e.currentTarget.value as PrintOptions["scale"] })}
          options={[
            { value: "fit", label: pt("print.scaleFit") },
            { value: "shrink", label: pt("print.scaleShrink") },
            { value: "actual", label: pt("print.scaleActual") },
            { value: "custom", label: pt("print.scaleCustom") },
          ]}
        />
      </Field>
      <Show when={options().scale === "custom"}>
        <Field label={pt("print.percent")}>
          <TextInput
            type="number" min={10} max={400} value={options().scalePercent}
            onChange={(e) => patch({ scalePercent: num(e.currentTarget.value, 10, 400, 100) })}
          />
        </Field>
      </Show>

      <Checkbox label={pt("print.gray")} hint={pt("print.grayHint")} checked={options().grayscale} onChange={(e) => patch({ grayscale: e.currentTarget.checked })} />
      <Checkbox label={pt("print.comments")} hint={pt("print.commentsHint")} checked={options().comments} onChange={(e) => patch({ comments: e.currentTarget.checked })} />
    </TaskPanel>
  );
}

/** La hoja en miniatura con la página donde caerá; lo que sobresale de la zona imprimible se recorta, como en el papel. */
function Preview(p: { sheet: SheetMetrics; placement: Placement | null; src: string | null; alt: string; label: string }) {
  const pct = (v: number, of: number) => `${(v / of) * 100}%`;
  const area = () => ({
    left: pct(p.sheet.offX, p.sheet.physW), top: pct(p.sheet.offY, p.sheet.physH),
    width: pct(p.sheet.areaW, p.sheet.physW), height: pct(p.sheet.areaH, p.sheet.physH),
  });
  const rect = () => {
    const pl = p.placement;
    if (!pl) return null;
    return {
      left: pct(pl.x, p.sheet.areaW), top: pct(pl.y, p.sheet.areaH), width: pct(pl.w, p.sheet.areaW), height: pct(pl.h, p.sheet.areaH),
    };
  };
  const turned = () => (p.placement?.rotate ?? 0) === 1;
  return (
    <figure class="print-preview" aria-label={p.label}>
      <div class="print-preview__sheet" style={{ "aspect-ratio": `${p.sheet.physW} / ${p.sheet.physH}` }}>
        <div class="print-preview__area" style={area()}>
          <Show when={p.src && rect()}>
            <div class="print-preview__page" style={rect()!}>
              <img
                src={p.src!} alt={p.alt} draggable={false}
                classList={{ "print-preview__img--turned": turned() }}
                style={turned() ? { width: `${((p.placement!.h) / p.placement!.w) * 100}%`, height: `${((p.placement!.w) / p.placement!.h) * 100}%` } : undefined}
              />
            </div>
          </Show>
        </div>
      </div>
    </figure>
  );
}

