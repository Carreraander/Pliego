# Prueba con el Narrador del visor (Leer)

Para acr12 o «pc» (Windows con Narrador: `Win + Ctrl + Intro`). Lo automatizado (axe, roles, anuncios, foco y alto contraste) está en
`app/tests/visual/read-a11y.spec.ts`; esto comprueba lo que solo se ve con un lector de pantalla de verdad. Resultado esperado entre
paréntesis. Anotar lo que falle en `docs/handoff/flujos/w44.md`.

**Preparación:** instalar la beta o `npm run tauri build`, abrir un PDF de varias páginas con texto, un índice con enlaces internos y
una dirección web, y un PDF escaneado sin OCR. Modo **Leer**. Narrador activado, ventana de Pliego con el foco.

| # | Pasos | Esperado |
| --- | --- | --- |
| 1 | `Tab` hasta el visor | Lee «Documento: <título>, región» |
| 2 | Con el Narrador en modo exploración (`Caps + Espacio` si hace falta), flecha abajo dentro del visor | Lee «Página 1 de N», grupo, y el texto de la página por párrafos, en orden de lectura |
| 3 | `Tab` desde el visor | Llega al primer enlace; lee «Ir a la página N, vínculo» o «Abrir enlace: https://…, vínculo»; el recuadro de foco se ve |
| 4 | `Intro` en un enlace interno | Salta; tras unos instantes no lee nada raro; `Alt + ←` lee «Vuelves a la página X» y `Alt + →` «Adelante a la página Y» |
| 5 | `Intro` en una dirección web | Se abre el diálogo «¿Abrir este enlace?» y el Narrador lee la dirección y «Sitio: …»; `Esc` cancela y no abre nada |
| 6 | Desplazarse con `RePág` / rueda y parar | Tras ~0,7 s lee «Página N de M» (una vez) |
| 7 | `Mayús + →` / `Mayús + ↓` | Lee «Seleccionado: <palabra>» en cada paso |
| 8 | `Ctrl + A`, luego `Ctrl + C` | Lee «Todo el texto del documento seleccionado» y «Texto copiado»; pegar en el Bloc de notas da el texto |
| 9 | Abrir el PDF escaneado | La página lee «Sin texto que leer ni seleccionar; si es un escaneo, el OCR lo añade» |
| 10 | Tema de alto contraste de Windows (`Alt izq + Mayús izq + Impr Pant`) | La selección y el recuadro de foco de los enlaces se ven con los colores del tema; las coincidencias de la búsqueda también |

Límites conocidos: no hay cursor de texto por letra en la página (la selección es por palabra) y el Narrador no recorre el documento
con el cursor de texto de Windows, solo con su cursor virtual sobre el texto oculto de cada página.
