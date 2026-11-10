# VirgoStars Event Ephemeris — panoramica italiana

**Stato: soltanto documentazione e progettazione.** Il repository è autonomo dagli altri progetti VirgoStars, e la libreria finale potrà leggere file binari con eventi precalcolati senza PostgreSQL, connessione Internet o motore astronomico.

## Tre famiglie di dati

- **`sinodi`**: 70.554 righe segnalate, inclusi cicli lunari e fasi aggiuntive dei sinodi planetari.
- **`moto_pianeti`**: 266.516 righe segnalate, cambi di moto diretto/retrogrado e segno al momento della stazione.
- **`ingresso_segni`**: 43.774 righe segnalate, ingressi nei segni anche durante le retrogradazioni.

**380.844 righe in totale** al 10 novembre 2026 secondo i dati comunicati dal proprietario; la copertura temporale e la completezza non sono ancora state verificate su un export.

## Architettura

**Rust** gestisce formato binario, integrità, indici, ricerca e navigazione `sinodo.next()`/`previous()`. **JS/TS** via WebAssembly e **Python** via PyO3 riutilizzano il medesimo motore, senza riscrivere tre volte la logica. La libreria deve almeno sostituire le interrogazioni astronomiche delle tre tabelle attualmente fatte dal backend VirgoStars.

Il file `.vseph` sarà versionato e distribuito separatamente dai pacchetti software, con metadati, checksum e registri di copertura. Le regole su orari UTC e cambi di ora legale sono obbligatorie.

## Autore del dataset e licenze

L'autore dichiara che il dataset è completamente suo e generato da algoritmi propri. Questa dichiarazione è registrata, non verificata indipendentemente. **Il dataset non è ancora pubblicato e non è ancora stata selezionata la licenza dei dati.** Il **codice** usa la stessa CPAL-1.0 di VirgoStars-Ephemeris, con attribuzione “Powered by VirgoStars” e link al progetto, secondo i termini completi della licenza.

## Indicazioni per gli agenti

Leggere prima `AGENTS.md`, `docs/ROADMAP.md` e le PR aperte. Eseguire cambiamenti concreti e verificabili su branch e PR draft; aggiornare test, documenti e `docs/WORK_LOG.md`; non usare GitHub Actions per automatizzare il lavoro; non fare merge, deploy o pubblicazioni senza decisione esplicita. Non inventare dati mancanti o accessi a PostgreSQL.

Per i dettagli tecnici, le fonti e la lista dei prossimi lavori, usare la documentazione in inglese.
