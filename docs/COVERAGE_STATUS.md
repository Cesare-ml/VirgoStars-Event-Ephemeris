# Coverage / Intervalli temporali ancora da elaborare

**Ultimo aggiornamento documentale: 2026-11-17.** Questo è il punto di ingresso pubblico per verificare **cosa manca**, per corpo celeste e famiglia di eventi. **Nessuna fascia temporale dell'archivio storico è attualmente certificata come astronomicamente completa.** Il dataset originale non è distribuito nel repository e non esiste qui un inventario P0 autorizzato, firmato e scientificamente revisionato che consenta di elencare *tutte* le lacune.

## Lacune o anomalie attualmente documentate

Le date sono **UTC**. Gli intervalli di elaborazione sono scritti come `[inizio incluso, fine esclusa)`. Gli estremi delle prime due righe sono **ancore presenti nell'archivio originale**, non date ricostruite di eventi mancanti: sono documentati vuoti *tra ancore*, non assenza certificata di ogni evento al loro interno.

| Famiglia / corpo | Dall'ancora registrata (UTC) | All'ancora successiva (UTC) | Evidenza | Cosa manca / stato |
| --- | --- | --- | --- | --- |
| Sinodi / Luna | `1918-12-03T15:18:52Z` (ID 44934) | `1920-01-21T05:26:29Z` (ID 44935) | Due noviluni registrati distanti ~413,6 giorni; [analisi sorgente](MOON_SYNOD_SOURCE_GAPS.md) | Calcolo dei cicli intermedi, controllo indipendente, decisione sulla copertura; **non convalidato** |
| Sinodi / Luna | `2017-12-18T06:30:25Z` (ID 46146) | `2019-01-06T01:28:11Z` (ID 46147) | Due noviluni registrati distanti ~383,8 giorni; [analisi sorgente](MOON_SYNOD_SOURCE_GAPS.md) | Calcolo dei cicli intermedi, controllo indipendente, decisione sulla copertura; **non convalidato** |
| Ingressi / Luna | Area di indagine `2099-01-01T00:00:00Z` | `4836-01-01T00:00:00Z` | Anomalia riferita nella sorgente; **range di scansione, non un'unica lacuna dimostrata** | Segmentare l'intervallo e determinare gli **esatti** intervalli sparsi con il report P0, poi convalidare; **non auditato pubblicamente** |
| Sinodi / altri corpi | **non determinato** | **non determinato** | Nessun manifest pubblico di copertura per corpo | Audit P0 e parità scientifica, oltre a portare i calcoli planetari; **ignoto** |
| Moto / corpi supportati | **non determinato** | **non determinato** | Nessun manifest pubblico di copertura per corpo | Audit P0 per ogni corpo, dati da elaborare da identificare; **ignoto** |
| Ingressi / altri corpi | **non determinato** | **non determinato** | Nessun manifest pubblico di copertura per corpo | Audit P0 per ogni corpo, dati da elaborare da identificare; **ignoto** |
| Aspetti fra pianeti / congiunzioni 0°, sestili 60°, quadrature 90°, trigoni 120° | **non determinato, per coppia** | **non determinato, per coppia** | Candidati Rust per attraversamenti esatti e **avvisi di possibili tangenze** sperimentali; famiglia archivio **assente** | Completezza di tangenze/scansioni, verifica indipendente, formato e audit per ogni coppia/tipo; **non convalidato** |
| Estremi di velocità diretta e retrograda / ogni corpo | **non determinato** | **non determinato** | Provider già fornisce velocità, ma **non esiste un algoritmo per ricercarne i massimi** | Scrivere calcolo degli estremi, validazione e formato/reader; **non implementato** |
| Eclissi solari e lunari | **non determinato** | **non determinato** | Metodi specialistici legacy, non portati in Rust | Algoritmo astronomico specialistico, sorgente e audit; **non implementato** |

**Attenzione:** il range ampio 2099–4836 è solo l'area *proposta* per la ricerca e richiede una scansione in piccole finestre. Non va mai etichettato come intervallo completamente privo di eventi. Anche fuori dagli intervalli qui indicati la copertura è **sconosciuta**, non dichiarata completa.

## Come ottenere la lista esatta e leggibile dal proprio archivio autorizzato

1. Eseguire `vseph-fill-gaps --mode plan` per ogni famiglia/corpo/finestra, con un archivio P0 **autorizzato**, il suo SHA-256 ottenuto per via indipendente e una soglia scientificamente motivata. Su Windows, usare lo script `tools/compute-missing-intervals.ps1` descritto in [rigenerazione](REGENERATION_PIPELINE.md#windows-batch-script-for-known-gaps-and-resumable-intervals). Il comando non modifica mai il P0.
2. Convertire i JSON di staging omogenei (stesso P0, famiglia, corpo e soglia) in un unico prospetto:

   ```powershell
   py -3 tools/report_missing_intervals.py --format markdown D:\vseph-staging\prima-finestra.json D:\vseph-staging\seconda-finestra.json
   py -3 tools/report_missing_intervals.py --format json D:\vseph-staging\prima-finestra.json
   ```

   Su Linux/macOS usare `python3` con i propri percorsi. Il report mostra **date UTC leggibili**, limiti esatti `[inizio, fine)`, tipo di lacuna, ID ancore registrate, stato della generazione, finestre analizzate e **periodi non controllati tra finestre**. Periodi *fuori* dalle finestre restano esplicitamente ignoti. Per centinaia di file, costruire la lista di argomenti con il proprio script, assicurandosi di includere tutte le finestre pertinenti.
3. Ripetere l'inventario dopo qualsiasi nuova acquisizione approvata. Prima di chiamare un intervallo **convalidato**, sono necessari verifica strutturale, secondo calcolo/riscontro astronomico indipendente, revisione scientifica e registrazione esplicita nel manifest di release **legato a uno SHA-256**. Nessun report generato automaticamente costituisce da solo quella certificazione.

### Regole di interpretazione del report

| Campo/stato | Significato operativo |
| --- | --- |
| `between_recorded_anchors` | Intervallo anormalmente lungo tra due ancore originali; **candidato** a backfill |
| `leading_unconfirmed_boundary` | Prima dell'ancora successiva: potrebbe essere al di fuori della sorgente, **non** certamente mancante |
| `trailing_unconfirmed_boundary` | Dopo l'ultima ancora precedente: potrebbe essere al di fuori della sorgente |
| `no_recorded_anchors_in_window` | Nessuna ancora in quella finestra: mancanza di ancore registrate, non prova di eventi fisici assenti |
| `da_calcolare` | Solo pianificato; non esistono candidati calcolati in quel report |
| `candidati_non_convalidati` | Tentativo di calcolo eseguito; nessun dato importabile senza verifica indipendente |
| `non_verificato` | Tra finestre analizzate, nessuno scan consegnato al report |
| Nessuna lacuna segnalata | **Non** significa copertura completa: la soglia può nascondere eventi mancanti |

La soglia è un'**euristica per il corpo/famiglia**, non un criterio universale. Il report rifiuta file incongruenti, SHA/famiglie/soglie diverse e finestre sovrapposte. Non certifica la provenienza del file immesso: non fidarsi di un JSON prodotto da terzi senza il controllo dell'archivio e la revisione `vseph-review-candidates`.

## Registro di copertura da mantenere a ogni release

Il futuro manifest pubblico deve riportare **per ogni famiglia e corpo**: versione e SHA-256 del dataset effettivamente revisionato, intervallo UTC esaminato `[start_us,end_us)`, soglia e metodo di scansione, **elenco esatto** degli intervalli non coperti, evidenze e limiti della verifica scientifica, revisore e data dell'approvazione. Se manca uno di questi elementi, lo stato rimane `unknown`/`unreviewed`, mai `complete`. Conservare lo storico delle revisioni, senza sovrascrivere gli eventi/ID originali. L'eventuale pubblicazione del dataset e del manifest resta subordinata alla licenza e all'approvazione del proprietario: [provenienza](DATA_PROVENANCE.md).

Per partecipare ai calcoli: [guida ai contributi per nuovi intervalli](CONTRIBUTING_EVENT_RANGES.md).

Per la distinzione fra algoritmo **esistente**, algoritmo **da scrivere** e dati **da convalidare** per tutti i nuovi eventi richiesti: [inventario tecnico degli algoritmi](ASTRONOMICAL_EVENT_ALGORITHMS.md). Il reporter P0 attuale legge solo `synods`, `motions`, `ingresses`: **non può** inventariare le lacune delle future famiglie `aspects` o `speed_extrema` finché non esistono un formato e dati verificabili.
