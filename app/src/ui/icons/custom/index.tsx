import type { JSX } from "solid-js";

/** Iconos propios de PDF que Lucide no tiene. Rejilla 24×24, trazo con `currentColor`. */
export type IconProps = JSX.SvgSVGAttributes<SVGSVGElement> & { size?: number; strokeWidth?: number };

function base(props: IconProps, children: JSX.Element) {
  const { size = 20, strokeWidth = 1.75, ...rest } = props;
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none"
      stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round"
      aria-hidden="true" {...rest}
    >{children}</svg>
  );
}

/** Redactar: bloque macizo sobre líneas de texto. */
export const RedactIcon = (p: IconProps) => base(p, <><path d="M4 6h16" /><path d="M4 18h10" /><rect x="4" y="10" width="16" height="4" rx="1" fill="currentColor" /></>);
/** Numeración Bates: documento con almohadilla. */
export const BatesIcon = (p: IconProps) => base(p, <><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" /><path d="M14 3v5h5" /><path d="M9.5 12.5h5M9.5 16h5M11 11l-.8 6M13.8 11 13 17" /></>);
/** PDF/A: documento con sello de archivo. */
export const PdfAIcon = (p: IconProps) => base(p, <><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" /><path d="M14 3v5h5" /><path d="m9 17 2-5 2 5M9.8 15.5h2.4" /></>);
/** OCR: marco de escaneo con letra. */
export const OcrIcon = (p: IconProps) => base(p, <><path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2" /><path d="M9 16V9h4a2 2 0 0 1 0 4H9m4 0 2 3" /></>);
/** Sello de tiempo: reloj dentro de un sello. */
export const TimestampIcon = (p: IconProps) => base(p, <><circle cx="12" cy="11" r="7" /><path d="M12 7.5V11l2.5 1.5" /><path d="M8 21h8M12 18v3" /></>);
/** Firma digital (PAdES): pluma con línea y certificado. */
export const SignDigitalIcon = (p: IconProps) => base(p, <><path d="m4 17 3.5-.5L18 6a2.1 2.1 0 0 0-3-3L4.5 13.5z" /><path d="M4 21h8M16 18l1.5 1.5L21 16" /></>);

/** Logo: hoja cuadrada con la esquina superior derecha doblada. Usa tokens (nunca colores literales). */
export function Logo(props: { size?: number; mono?: boolean } & JSX.SvgSVGAttributes<SVGSVGElement>) {
  const { size = 20, mono = false, ...rest } = props;
  const sheet = mono ? "currentColor" : "var(--pl-accent)";
  const fold = mono ? "var(--pl-chrome)" : "var(--pl-accent-soft)";
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" {...rest}>
      <path d="M5 2h10l7 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z" fill={sheet} />
      <path d="M15 2v5a2 2 0 0 0 2 2h5z" fill={fold} />
      <path d="M15 2l7 7" stroke={sheet} stroke-width="0.8" fill="none" />
    </svg>
  );
}
