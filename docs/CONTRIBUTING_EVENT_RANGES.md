# Contribute event calculations / Contribuire ai nuovi intervalli

La libreria Event Ephemeris è progettata per leggere eventi astronomici precalcolati senza un motore astronomico a runtime. **Si può contribuire estendendo il calcolo e controllando le lacune, ma un calcolo non diventa automaticamente un dato pubblico o certificato.** Prima di iniziare leggere [AGENTS.md](../AGENTS.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [stato della copertura](COVERAGE_STATUS.md), [rigenerazione](REGENERATION_PIPELINE.md) e [provenienza/licenza](DATA_PROVENANCE.md).

Per le nuove famiglie richieste (aspetti esatti **0°/60°/90°/120°**, estremi della velocità longitudinale **e latitudinale**, stazioni/ripresa in latitudine e attraversamenti del piano eclittico), partire dalla [matrice di funzionalità e algoritmi mancanti](ASTRONOMICAL_EVENT_ALGORITHMS.md): **non sono ancora supportate dal generatore di lacune P0**. Un contributore può svilupparne e testarne il motore Rust con fixture sintetiche prima che un dataset autorizzato sia disponibile.

**Issue [#38 — eventi latitudinali](https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris/issues/38):** il nuovo trait di sviluppo `LatitudeProvider` chiede latitudine geocentrica eclittica `β` e velocità `dβ/dt`; `LongitudeProvider` mantiene soltanto `λ, dλ/dt`. Sono disponibili come **candidati Rust sperimentali** `latitude_crossings::ecliptic_plane_crossing_candidates` (`β=0`, ascending/descending), `latitude_crossings::latitude_station_candidates` (`dβ/dt=0`, inversione con `direction_after`) e `latitude_extrema::latitude_speed_extremum_candidates` (massimi firmati positivi verso nord/minimi firmati negativi verso sud), con corrispondenti scansioni adattive `adaptive_ecliptic_plane_crossings_scan`, `adaptive_latitude_stations_scan` e `adaptive_latitude_speed_extrema_scan`. Tutte richiedono `SearchWindow` UTC e `&mut usize` come budget; le varianti adattive aggiungono `max_depth` 1–8, campioni ai quarti e contatori diagnostici. Zeri iniziali senza contesto, plateau, tangenze e picchi nascosti devono essere trattati secondo i contratti specifici; **neanche la scansione adattiva certifica l'assenza di eventi**. Test: `cargo test --locked -p virgostars-events-generator latitude_crossings` e `cargo test --locked -p virgostars-events-generator latitude_extrema` (33 test sintetici). **Mancano ancora** strategia di completezza scientifica, adapter astronomico, piano eclittico/versione confrontabile, verifiche scientifiche reali, CLI e lettore. Nessuna finestra UTC è certificata.

**Sperimentare i picchi di velocità senza dataset:** il crate Rust `events-generator` offre `speed_extrema::speed_extremum_candidates` (campionamento a tre punti) e `speed_extrema::adaptive_speed_extrema_scan` (quarti di passo, rischio di inversione/curvatura, profondità limitata). Entrambi richiedono un `LongitudeProvider`, un `BodyId`, una `SearchWindow` UTC e un budget condiviso di campioni (`&mut usize`); la seconda accetta anche `max_depth` da 1 a 8 e restituisce contatori diagnostici. Per verificare **numericamente** un candidato è disponibile `speed_crosscheck::crosscheck_speed_extremum` con `SpeedCrosscheckOptions`: richiede una finestra UTC più ampia del bracket, una semidistanza temporale e due tolleranze numeriche, restituisce stato supportato/contraddetto/inconcludente e residui. Usa **lo stesso provider** della candidatura, quindi non sostituisce il confronto con un secondo motore astronomico. Sono **API di sviluppo**, non modalità CLI `vseph-fill-gaps` né contenuti leggibili dalle app: confrontare sempre i risultati con un provider indipendente e segnalare tutti gli intervalli non controllati. Test sintetici: `cargo test --locked -p virgostars-events-generator speed_extrema` e `cargo test --locked -p virgostars-events-generator speed_crosscheck`.

**Confrontare due motori:** la funzione Rust `speed_provider_compare::compare_speed_extrema_providers` accetta `primary` e `reference` come due `LongitudeProvider`, lo stesso corpo e `SpeedProviderComparisonOptions` (finestra UTC, profondità, tolleranze temporale e di velocità). Passare **budget separati**; conservare nella PR sia gli abbinamenti sia gli estremi osservati da **un solo provider**. Il codice non verifica l'indipendenza scientifica delle sorgenti: documentare separatamente kernel, effemeridi, versione, sistema tropicale geocentrico, scala temporale, precisione e licenza. Due scansioni vuote, perfettamente concordi, **non convalidano gli intervalli mancanti**. Test: `cargo test --locked -p virgostars-events-generator speed_provider_compare`. Non esiste ancora una CLI di confronto per questa nuova famiglia né un archivio `speed_extrema` approvato.

## Percorso 1 — contributi senza il dataset originale

**Non è necessario accedere al database VirgoStars** per migliorare algoritmi, generare casi di prova o confrontare due motori. Un contributore può:

1. Aprire una issue per un **corpo, una famiglia e un intervallo `[UTC inizio, UTC fine)` specifici**, verificando prima i lavori aperti.
2. Creare un fork e un branch (`git clone https://github.com/Cesare-ml/VirgoStars-Event-Ephemeris.git`), predisporre Rust 1.90 e un **proprio** provider compatibile con il protocollo di longitudine/velocità documentato in [rigenerazione](REGENERATION_PIPELINE.md). Dichiarare modello di tempo, sistema di riferimento, versione del kernel, accuratezza e diritti di riutilizzo. Non caricare credenziali, effemeridi proprietarie o dati senza licenza.
3. Per **stazioni** o **ingressi**, provare `vseph-generate-candidates` senza P0. Esempio di **finestra illustrativa**, da sostituire con quella assegnata:

   ```bash
   cargo run --locked -p virgostars-events-generator --bin vseph-generate-candidates -- \
     --mode stations --body Mars --start-us 1577836800000000 --end-us 1609459200000000 \
     --step-us 21600000000 --precision-us 1000000 \
     --provider-program /path/to/your-authorized-longitude-provider
   ```

   L'output contiene **bracket temporali non verificati**, non eventi convalidati. Il comando è attualmente limitato a `stations`/`ingresses`; gli altri corpi per `synods`, aspetti ed eclissi richiedono implementazioni dedicate. Non inventare dati per colmare le funzioni non ancora portate.
4. Scrivere test sintetici e, quando le licenze lo permettono, confronti numerici indipendenti su casi reali, compresi retromoti, attraversamenti multipli e confini di finestra. Annotare anche i limiti e gli eventi che **non** è possibile escludere.
5. Aprire una **draft PR** sul codice/documentazione, con metodologia, intervalli esatti e test. Se si condividono esempi, usare fixture sintetiche o dati esplicitamente autorizzati. Non dichiarare il corpus pubblico ampliato.

## Percorso 2 — censire e calcolare le lacune su un P0 autorizzato

Solo il proprietario o un contributore **esplicitamente autorizzato** può usare l'archivio storico in privato. Non serve pubblicarlo. Il flusso è:

1. Autenticare localmente l'intero P0 con lo SHA-256 **ottenuto indipendentemente** dall'output di uno strumento; non recuperarlo dal file di staging. Scegliere famiglia (`synods`, `motions`, `ingresses`), corpo, soglia adatta e finestra UTC limitata.
2. Eseguire **prima** `vseph-fill-gaps --mode plan` oppure, su Windows, lo script di slicing in PowerShell:

   ```powershell
   .\tools\compute-missing-intervals.ps1 -Archive "D:\data\authorized.p0" `
     -TrustedSha256 "<SHA256 verificato indipendentemente>" -OutputDir "D:\vseph-staging" `
     -KnownMoonGaps -Mode plan
   ```

   `-KnownMoonGaps` copre due **anomalie note fra ancore della Luna**, non l'intero archivio. Per altri periodi usare `-Family`, `-Body`, `-FromUtc`, `-ToUtc`, `-ThresholdDays`, `-SliceYears`. Il comando registra staging in file distinti e non modifica il P0.
3. **Elencare chiaramente i periodi mancanti con date UTC**, prima di scegliere cosa calcolare:

   ```powershell
   py -3 tools/report_missing_intervals.py --format markdown "D:\vseph-staging\plan-1.json" "D:\vseph-staging\plan-2.json"
   ```

   Sostituire i nomi con i file **realmente prodotti**. L'utility elenca gli intervalli segnalati, i periodi *non* ancora esaminati fra le finestre e il fatto che tutta la copertura esterna rimane **ignota**. Non mescolare P0, soglie, corpi o famiglie in un'unica tabella. Annotare gli intervalli `[inizio incluso, fine esclusa)` in issue e draft PR.
4. Per le finestre pertinenti, passare a `--mode calculate` con provider autorizzato. Le tre famiglie di pianificazione sono interrogabili, ma il calcolo di `synods` è **solo per Luna**, oltre a moto e ingressi. Abilitare i controlli `-VerifyHalfStep`, `-Adaptive...` e, se disponibile, un secondo provider indipendente, con le relative opzioni documentate. Nessuna opzione prova da sola la completezza.
5. Eseguire `vseph-review-candidates` contro il **P0 originale autenticato**; aggiungere dove possibile confronto eventi indipendente, controlli di precisione, convenzioni astronomiche, versi retrogradi e bordi temporali. Il revisore automatico mantiene `coverage_certified:false` e `safe_to_import:false` anche quando le verifiche passano.
6. Aprire draft PR con sorgente/metodo, lista di periodi ancora mancanti e risultati dei controlli. Lasciare **fuori dal PR** file P0, CSV originali, provider proprietari, calcoli reali non autorizzati e segreti. L'import/pubblicazione è una decisione separata del manutentore, dopo revisione astronomica e della licenza.

## Informazioni obbligatorie in issue/PR per estensione degli intervalli

| Campo | Esempio o regola |
| --- | --- |
| Famiglia e corpo | `motions / Mars` oppure `synods / Moon` |
| Intervallo richiesto `[start_utc,end_utc)` | ISO 8601 UTC, fine esclusa; per esempio `[2020-01-01T00:00:00Z,2021-01-01T00:00:00Z)` |
| Intervalli segnalati / ancora scoperti | Tabella da `report_missing_intervals.py`; **esplicitare quelli non auditati** |
| Identità sorgente | SHA-256 dell'archivio P0, solo se autorizzato a condividerlo; altrimenti indicare che la prova è trattenuta privatamente |
| Metodo di calcolo | Versione, kernel, coordinate, sistema temporale, provider, licenza e risoluzione di scansione |
| Riscontri | Test sintetici, audit strutturale, confronto indipendente, errori/limiti noti |
| Stato dichiarato | `da_auditare`, `lacuna_candidata`, `calcolato_non_convalidato`, `revisionato_indipendentemente` oppure `approvato` **solo dal manutentore** |

**Definizione di “finito”:** l'intervallo è riproducibile, confrontato indipendentemente, approvato con un manifest di copertura legato ai byte realmente revisionati e con diritti di distribuzione chiariti. Finché questo non accade, lasciare la lacuna visibile nel [registro](COVERAGE_STATUS.md), non attribuire eventi a ID sorgente originali e non far comparire `complete` nelle API.

Non sono richieste e non vanno aggiunte GitHub Actions. Il codice è CPAL-1.0 con attribuzione VirgoStars; la licenza del dataset è **separata e ancora da approvare**.
