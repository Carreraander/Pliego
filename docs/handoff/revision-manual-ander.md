# Revisión manual pendiente de Ander

Solo lo que necesita ojos humanos. Lo demás se valida con renders PNG y pruebas automáticas.

## W44: imprimir

Con un PDF abierto, pulsa **Ctrl+P** (o busca «Imprimir» en Ctrl+K):

1. Se abre el panel «Imprimir» en la columna derecha (no se imprime la interfaz de la app ni sale el menú de Edge).
2. La **vista previa de la hoja** cambia al elegir «Tamaño real», un porcentaje o «Horizontal», y la página apaisada se gira sola en una hoja vertical.
3. Imprime en «Microsoft Print to PDF» (o en una impresora de papel): las páginas elegidas salen, centradas y sin cortes; con 2 copias y «Intercalar» el orden es 1, 2 · 1, 2.
4. Con comentarios y campos rellenados: salen los campos con lo escrito y, con «Incluir comentarios» desmarcado, sin notas ni resaltados.

## W43: edición tipo Acrobat (PR #211)

Con un PDF de Word de varias páginas, en modo Editar:

1. **Texto que se recoloca.** Alarga un párrafo (con «Recolocar lo de debajo» marcado). Comprueba que lo de debajo baja, que lo que no cabe pasa a la página siguiente y que el pie de página no se pisa. Deshaz (Ctrl+Z): debe volver todo de una vez.
2. **Imagen.** Con la herramienta Imagen, mueve y redimensiona una imagen, recórtala, sustitúyela y bórrala. Comprueba que no se deforma sin querer.
3. **Vector.** Con la herramienta Objeto, mueve un rectángulo o una línea del PDF.
4. **Enlace.** Crea un enlace (a página y a URL), cambia su destino y bórralo. Ábrelo luego en Reader y comprueba que lleva donde debe.
5. Abre el resultado en Reader y comprueba que el texto movido sigue seleccionable y que no hay huecos extraños.
