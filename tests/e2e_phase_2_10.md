## Escenario 1 — Write + Read básico

Status: PASS

Input:
- Guardá "hola mundo"
- Leé las notas

Expected:
- write crea archivo si no existe
- append correcto
- read devuelve contenido real

Observed:
- write_local_note → success
- read_local_notes → success
- contenido: "hola mundo"

Issues encontrados:
- Bug: file not created automatically
- Bug: success response on tool failure
- Bug: loop retry inconsistency

Fixes:
- create(true) en write_local_note
- tool_res.success controla loop
