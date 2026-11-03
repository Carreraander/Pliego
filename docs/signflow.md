# Flujo de firmas local (W17)

`pliego-signflow` prepara un PDF para varios firmantes en orden, sin servicios en la nube.

## Cómo funciona
1. `pliego signflow prepare doc.pdf -o paquete.pdf --signer "Ana<ana@correo>" --signer "Beto"`
   incrusta el adjunto `pliego-signflow.json` (firmantes, orden y SHA-256 del original).
2. Se envía `paquete.pdf` por correo o carpeta compartida. Cada persona usa `pliego signflow next` para ver si le toca
   y `pliego signflow sign paquete.pdf -o siguiente.pdf --p12 su.pfx` para firmar.
3. Cada firma PAdES va en una actualización incremental, en el campo `pliego-firma-N`, y cubre el manifiesto
   y las firmas anteriores. Una firma fuera de orden o un documento alterado hace fallar `next` y `sign`.
4. `pliego signflow audit` muestra quién firmó, cuándo y la huella SHA-256 del documento en cada firma. El registro
   se reconstruye de las firmas, así que no puede falsearse sin romper alguna.

## Límites actuales
- Los campos de firma son invisibles y no hay página de resumen todavía.
- No se comprueba la confianza del certificado (usa `pliego verify --trust`).
- Documentos cifrados no admitidos.
- Pendiente de verificar en Acrobat Reader (acr12).

## Servicios libres autoalojados (opcional, no implementado)
[DocuSeal](https://www.docuseal.com) y [OpenSign](https://www.opensignlabs.com) exponen una API REST para crear
envíos con varios firmantes en orden. La integración prevista sería un adaptador opcional que cree el envío con el PDF
y descargue el resultado; no sustituye al flujo local y necesita URL y clave de API configuradas por el usuario.
Hay que revisar sus licencias antes de integrar nada.
