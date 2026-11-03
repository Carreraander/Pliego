import { splitProps, type JSX } from "solid-js";
import { Icon } from "./Icon";
import "./Button.css";

export type ButtonVariant = "primary" | "normal" | "ghost" | "danger";

export interface ButtonProps extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  variant?: ButtonVariant;
  /** Nombre de icono a la izquierda del texto. */
  icon?: string;
  /** Muestra un indicador de actividad y desactiva el clic (sin perder el foco). */
  busy?: boolean;
  children?: JSX.Element;
}

export function Button(props: ButtonProps) {
  const [own, rest] = splitProps(props, ["variant", "icon", "busy", "children", "class", "onClick", "type"]);
  return (
    <button
      type={own.type ?? "button"}
      class={`pl-btn pl-btn--${own.variant ?? "normal"} ${own.class ?? ""}`}
      aria-busy={own.busy || undefined}
      onClick={(e) => { if (!own.busy) (own.onClick as ((e: MouseEvent) => void) | undefined)?.(e); }}
      {...rest}
    >
      {own.icon && <Icon name={own.icon} size={16} />}
      {own.children}
    </button>
  );
}

export interface IconButtonProps extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  icon: string;
  /** Nombre accesible obligatorio (también es el texto del tooltip nativo). */
  label: string;
  size?: "sm" | "md";
  /** Estado de botón conmutable (aria-pressed). */
  pressed?: boolean;
  variant?: "ghost" | "danger";
}

export function IconButton(props: IconButtonProps) {
  const [own, rest] = splitProps(props, ["icon", "label", "size", "pressed", "variant", "class"]);
  return (
    <button
      type="button"
      class={`pl-iconbtn pl-iconbtn--${own.size ?? "md"} ${own.variant === "danger" ? "pl-iconbtn--danger" : ""} ${own.class ?? ""}`}
      aria-label={own.label}
      aria-pressed={own.pressed}
      {...rest}
    >
      <Icon name={own.icon} size={own.size === "sm" ? 16 : 20} />
    </button>
  );
}
