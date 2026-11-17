# Eventi astronomici richiesti: algoritmi, copertura e lavori mancanti

**Inventario del 2026-11-17.** Questo documento è la lista di controllo per le funzioni che la libreria *dovrà* offrire: eventi fra pianeti, ingressi nei segni, stazioni e velocità massime. **Un algoritmo che trova candidati non equivale a dati scientificamente convalidati o a un'API disponibile nel lettore offline.** Le ephemeridi sono fornite da un `LongitudeProvider` esterno in fase di sviluppo; il lettore finale consulta solo dati precalcolati e revisionati.

## Stato verificato nel codice

| Evento richiesto | Definizione di calcolo proposta | Rust `events-generator` | Archivio offline / API pubblica | Lavoro ancora necessario |
| --- | --- | --- | --- | --- |
| Congiunzione fra due pianeti | Longitudini eclittiche geocentriche coincidenti: **0°** | **Candidati Rust sperimentali** per attraversamenti e avvisi separati di possibili tangenze | **Da progettare**, non è una famiglia P0 | Verifica reale delle tangenze, completezza delle scansioni, confronto indipendente, schema/reader |
| Sestile | Separazione longitudinale **60°** | **Candidati Rust sperimentali** | **Da progettare** | Come sopra, entrambe le orientazioni e retrogradazioni |
| Quadratura | Separazione longitudinale **90°** | **Candidati Rust sperimentali** | **Da progettare** | Come sopra |
| Trigono | Separazione longitudinale **120°** | **Candidati Rust sperimentali** | **Da progettare** | Come sopra |
| Ingresso di un pianeta in un segno | Attraversamento delle soglie zodiacali ogni 30°, compreso rientro retrogrado | **Candidati implementati**: `ingress_candidates` con isolamento per bisezione | Famiglia `ingresses` esistente **solo come eventi registrati**, copertura non certificata | Parità astronomica, finestre mancanti, validazione e API stabile |
| Pianeta che si ferma e inverte la marcia | Velocità longitudinale geocentrica `dλ/dt = 0` con cambio di segno | **Candidati implementati**: `station_candidates`, `direction_after` diretta o retrograda | Famiglia `motions` registra cambi di direzione; copertura non certificata | Parità, errori di campionamento, eventuale modello di “sosta apparente” |
| Momento in cui riprende a muoversi | **Stesso istante matematico della stazione** che cambia da velocità negativa a positiva (oppure l'altro verso) | **Già rappresentabile** dal candidato `direction_after`; nessun secondo evento distinto | Lettura dello stato successivo da evento di moto registrato, non universalmente verificata | Specificare se l'utente richiede *anche* un intervallo vicino a velocità zero: serve una soglia convenzionale |
| Massima velocità in moto diretto | **Massimo locale** della velocità longitudinale positiva nell'intervallo scelto | **Da scrivere**: non esiste rilevatore di estremi della velocità | **Da progettare** come nuova tipologia | Differenze/derivata della velocità, ricerca estremi, precisione e confronto indipendente |
| Massima velocità in moto retrogrado | **Massimo locale del modulo della velocità negativa** (minimo locale della velocità con segno) | **Da scrivere** | **Da progettare** | Come sopra, chiarendo il significato di “massima” |

**Stato dei range temporali:** per tutti gli aspetti fra pianeti e per gli estremi di velocità, gli intervalli storici già elaborati sono **non determinati**; non esiste un dataset pubblico certificato da cui ricavare periodi coperti o mancanti. Per ingressi e stazioni sono presenti *registrazioni* storiche e un planner di lacune, ma i range non sono approvati scientificamente. Il [registro della copertura](COVERAGE_STATUS.md) deve continuare a mostrare `unknown` fino a una verifica per corpo/coppia di corpi, famiglia e intervallo UTC esatto `[inizio,fine)`.

## Definizioni algoritmiche da implementare

### A. Aspetti esatti fra pianeti — **primo rilevatore Rust sperimentale presente**

`crates/events-generator/src/aspects.rs` espone `exact_aspect_candidates` e il controllo aggiuntivo **euristico** `adaptive_exact_aspect_scan` (funzioni Rust, **non ancora CLI**, formato P0 o API lettore). Il secondo usa campioni intermedi e suddivisione limitata da profondità/budget per individuare anche **due attraversamenti nascosti fra estremi uguali**: questo scenario è verificato con dati sintetici.

`potential_aspect_tangencies(provider, body_a, body_b, window, tolerance_degrees)` aggiunge una **lista separata di avvisi**, cercando inversioni campionate della velocità longitudinale *relativa* e verificando quanto il punto stazionario si avvicini a 0°/60°/90°/120° (entrambi gli orientamenti). Il parametro esplicito `tolerance_degrees` è compreso fra 0 e **0,01°** ed è **solo una soglia di segnalazione**, non la precisione garantita dell'effemeride. Ogni avviso contiene il bracket temporale e il `residual_degrees` misurato: **residuo non nullo significa possibile quasi-aspetto, NON aspetto esatto**. Anche un residuo numericamente nullo richiede controllo indipendente. Gli estremi della finestra privi di informazioni su entrambi i lati e le inversioni multiple non campionate possono essere assenti dagli avvisi.

**Mancano ancora** la rilevazione sistematica e completa dei contatti tangenti, le ripetizioni invisibili anche ai campioni intermedi, una prova di completezza, la verifica scientifica indipendente e l'integrazione nell'archivio. Non interpretare un risultato vuoto come assenza verificata di aspetti.

- Usare le **longitudini eclittiche geocentriche tropicali** dei due corpi ottenute dallo **stesso provider configurato**, con ephemeris, flags, scala temporale, kernel e versione documentati. Il calcolo riguarda l'aspetto *in longitudine*, non la distanza angolare tridimensionale fra corpi osservati nel cielo.
- Trovare tutti gli istanti in cui `wrap360(λB(t) − λA(t))` raggiunge `0`, `60/300`, `90/270` o `120/240` gradi (orientazioni ± modulo 360). Ordinare le coppie per identificatore canonico, senza duplicare `A–B` e `B–A` o conteggiare due volte la congiunzione `0° = 360°`.
- Bracketing su differenza angolare **continua/disambiguata**, attraversamenti su 0°/360°, bisezione e scansione adattiva. Un campionamento grossolano può nascondere più aspetti o passaggi ripetuti durante i retrogradi: evitare false assenze, eseguire cross-check con cadenza più stretta e un **secondo provider scientificamente indipendente**. Riportare timestamp UTC con precisione esplicita; nessun evento con `source_id` inventato.
- Per la **prima versione**, distinguere `exact` dagli eventi di ingresso/uscita da un **orb**. Gli orb sono soglie di prodotto configurabili, non proprietà astronomiche fisiche; introdurre solo dopo approvazione della semantica applicativo/separativo, specialmente presso stazioni e inversioni.
- Il backend legacy ha `backend/routines/calcolo-aspetti-per-db.js` e `backend/services/astrology/aspetti-func.js::checkAspetto/getArchiAspettiFraNodi` con congiunzione, sestile, quadrato, trigono e altri aspetti; **non sono implementazioni Rust o prove di correttezza scientifica**. Vanno confrontate e, dove necessario, corrette, non copiate senza verifica.

### B. Stazioni e ripresa del moto — **candidati Rust presenti**

- Il rilevatore esistente trova uno **zero della velocità longitudinale** con cambio di segno: da positivo a negativo indica una stazione che dà inizio al moto retrogrado; da negativo a positivo indica la ripresa del moto diretto. Non vi sono *due tempi separati di fermata e ripartenza* per un moto matematicamente continuo: la velocità è zero esattamente all'inversione.
- Se si desidera visualizzare una fase di stazionarietà **percettiva**, definire separatamente una regola `|dλ/dt| <= soglia` con unità, corpo e algoritmo di entrata/uscita; **non esiste oggi** una soglia approvata. Non confondere questo intervallo convenzionale con una stazione istantanea.
- `station_candidates` e `ingress_candidates` restituiscono **brackets candidati**, senza certificazione scientifica né backfill. Il programma `vseph-fill-gaps` pianifica le lacune P0 autorizzate e calcola solo nuove candidature di sviluppo.

### C. Massimi di velocità — **algoritmo nuovo da scrivere**

- Definire la grandezza come **velocità apparente longitudinale geocentrica** `v(t) = dλ/dt`, espressa in gradi/giorno; non confonderla con velocità spaziale in km/s o velocità orbitale eliocentrica. Distinguere il massimo **positivo diretto** dal massimo in modulo **retrogrado** (velocità più negativa).
- Cercare gli estremi locali cercando i cambi di segno di `dv/dt` (accelerazione longitudinale), stimata da campioni di velocità con differenze numeriche controllate oppure fornita da un provider affidabile. Affinare l'istante e controllare classificazione/valore del massimo e confini dell'intervallo. Potrebbero esservi **più massimi locali**: non promettere un unico massimo globale fuori dalla finestra esaminata.
- Specificare prima dei dati di produzione risoluzione, tolleranze, finestre, gestione dei massimi ai confini, sovrapposizione fra cicli e criteri per separare picchi numerici da estremi reali. Confrontare risultati con una sorgente astronomica indipendente. Il solo `LongitudeProvider` esistente fornisce **già** velocità istantanea, quindi non serve un nuovo motore nel *reader*, ma **manca tutta la ricerca degli estremi**.

## Contratti di pubblicazione e verifiche necessarie

1. **Generatore:** `exact_aspect_candidates(provider, body_a, body_b, window)` è **disponibile solo come calcolatore Rust sperimentale**; `speed_extremum_candidates(provider, body, window)` è **solo proposto**. Sono ancora da aggiungere le verifiche complete dei contatti senza attraversamento, degli eventi nascosti fra campioni e la pipeline di importazione. Prevedere errori fail-closed per provider instabile, dati non finiti, passi ambigui e budget superati.
2. **Archivio:** versionare nuovi tipi di evento (`aspects`, `speed_extrema`) e metadata per coppia/tipo/verso, senza cambiare il significato dei tre tipi P0 già esistenti (`synods`, `motions`, `ingresses`). Conservare identità del dataset, SHA, fonte e copertura scientifica. Per “stazione” riusare `motions` anziché creare due righe inventate “stop” e “restart”.
3. **Lettore offline:** proporre `aspects.between/next/previous` per coppia e tipo, e `motions.speedExtremaBetween/next/previous` per corpo/tipo; mai calcolare le posizioni astronomiche a runtime. I dettagli non sono ancora API pubbliche approvate.
4. **Test obbligatori:** casi sintetici con aspettazione esatta 0°/60°/90°/120°, attraversamenti 0°/360°, moto relativo nullo, retrogradazioni e triplicazione dello stesso aspetto, timestamp coincidenti, massimi diretti/retrogradi distinti, bordi UTC `[start,end)`, provider che restituisce NaN/errore e scansioni che perdono due zeri. Golden timestamps da una seconda ephemeris indipendente, con precisione e tolleranza approvate; cross-platform Rust Desktop/Android e parità del decoder.
5. **Copertura:** inventariare **per ogni coppia e tipo di aspetto**, o per corpo/tipo di estremo, le finestre analizzate, gli intervalli mancanti, i periodi **non auditati** e il digest del dataset. Mai chiamare `complete` un intervallo solo perché una scansione non segnala candidati.

La priorità e i criteri di accettazione sono tracciati in [ROADMAP](ROADMAP.md). I contributi devono usare [CONTRIBUTING_EVENT_RANGES](CONTRIBUTING_EVENT_RANGES.md). Nessun dato reale pubblicato o importato senza licenza e approvazione esplicite.

**Issue aperte per contribuire:** [#34 — aspetti esatti tra pianeti](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/34), [#35 — estremi della velocità](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/35).
