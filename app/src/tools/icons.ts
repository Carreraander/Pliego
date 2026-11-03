import {
  Accessibility, ArrowLeftRight, BadgeCheck, ClipboardCheck, Columns2, Combine, Database, Droplet, Eraser, FileOutput, FilePlus2, FileType,
  Folders, FolderSearch, FormInput, Hash, House, Image, Keyboard, Languages, List, ListChecks, ListOrdered, PanelTop, PenLine, Presentation,
  Printer, Ruler, ScanSearch, SlidersHorizontal, Sparkles, Users, Wand, ExternalLink, Command,
} from "lucide-solid";
import { registerIcon } from "../ui";

/** Iconos de Lucide que usan los manifiestos de base y las pantallas de navegación (ya están en el árbol de la app). */
export function registerToolIcons(): void {
  const icons = {
    accessibility: Accessibility, "arrow-left-right": ArrowLeftRight, "badge-check": BadgeCheck, "clipboard-check": ClipboardCheck,
    "columns-2": Columns2, combine: Combine, database: Database, droplet: Droplet, eraser: Eraser, "file-output": FileOutput,
    "file-plus-2": FilePlus2, "file-type": FileType, folders: Folders, "folder-search": FolderSearch, "form-input": FormInput,
    hash: Hash, home: House, image: Image, keyboard: Keyboard, languages: Languages, list: List, "list-checks": ListChecks,
    "list-ordered": ListOrdered, "panel-top": PanelTop, "pen-line": PenLine, presentation: Presentation, printer: Printer, ruler: Ruler,
    "scan-search": ScanSearch, "sliders-horizontal": SlidersHorizontal, sparkles: Sparkles, users: Users, wand: Wand,
    "external-link": ExternalLink, command: Command,
  };
  for (const [name, component] of Object.entries(icons)) registerIcon(name, component);
}
