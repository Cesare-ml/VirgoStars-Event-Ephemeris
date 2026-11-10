# VirgoStars backend compatibility inventory

**Reviewed:** 2026-11-10 from `Cesare-ml/virgo-stars` main (model/controller sources). This is a baseline requirements inventory; **no adapter or migration is implemented**. Do not claim production readiness from matching method names.

## Authoritative reference files

- [Synod model](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/models/tabelle/sinodo.js)
- [Motion model](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/models/tabelle/moto_pianeti.js)
- [Ingress model](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/models/tabelle/ingresso_segni.js)
- [Synod lunar controller](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/controllers/astro-calcs/sinodi.controller.js)
- [Chart/event controller](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/controllers/astro-calcs/tema-render.controller.js)
- [Calendar generator](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/routines/calcolo-calendario-eventi-astrologici.js)
- [Synodic motion repository](https://github.com/Cesare-ml/virgo-stars/blob/master/backend/infrastructure/astrology-knowledge/legacySynodicMotionRepository.js)

## Required query parity

| Legacy usage | Existing behavior | Reader target / parity test |
| --- | --- | --- |
| `TabellaSinodo.getSinodoPerDataoraUtc` | Latest event start <= instant for body | `synods.at(body,t)` |
| `TabellaSinodo.getSinodiPerDataoraUtc` | One latest event for each element at/before instant | `synods.atAll(t)` |
| `TabellaSinodo.next` | Next later start for same body | `synod.next()`, strictly > start |
| `TabellaSinodo.getMotiSinodiciPianetiPerDataoraUtc` | d1/r2/r3/d4 or four lunar subphase codes from extra dates | `synods.phaseAt(body,t)` plus optional legacy lowercase adapter |
| `TabellaSinodo.getSinodoLunaPrecCurrSucc` | Monthly lunar phase/calendar support | `synods.lunarEvents` + `cyclesOverlapping` |
| `TabellaMotoPianeti.getMotoPianetiPerData` | Station events within selected local civil day (max 1000 in legacy implementation) | `motions.onDate(date,tz)` (document pagination/limit policy) |
| `TabellaMotoPianeti.getDirezioneMotoPianetaPerData` | Latest direction before instant for one body | `motions.stateAt(body,t)` |
| `TabellaMotoPianeti.getDirezioneMotoPianetiPerData` | Latest station per body through **end** of a date in requested zone | `motions.stateAll(t)` with explicit end-of-day conversion in adapter |
| `TabellaIngressoSegni.getIngressiPerData` | Ingresses on civil day in specified timezone | `ingresses.onDate(date,tz)` |
| `TabellaIngressoSegni.getIngressiLunaPerAnnoMese` | Moon sign ingresses within UTC calendar month | `ingresses.lunarEvents(year,month,'UTC')` |
| `TabellaIngressoSegni.getById` | Fetch database row by surrogate ID for Telegram usage | Remains in VirgoStars DB adapter unless explicit ID mapping artifact is approved |
| Calendar routine | Loops through synods and phases, ingresses, days | `events.between` plus specialized lunar/synod methods |
| `legacySynodicMotionRepository.findSynodicMotionsAt` | One batched last synod per selected body + D1/R2/R3/D4 | `synods.atAll(t,bodies)` and `phaseAt` |

## Not part of the standalone reader

`interpretazione`, `messaggio_id`, `initTelegramMessage`, Telegram `event_queue` and notification jobs remain in the app; `getPianetiCase`/chart rendering and longitude calculations are not part of this dataset reader. The application can join result events to existing row IDs only with a separate stable mapping and only if approved.

## Legacy discrepancies — **do not copy bugs**

1. The legacy `getSinodoLunaPrecCurrSucc` SQL shown in `sinodo.js` lacks an `elemento = 'Moon'` filter, and maps only the first three results to `prec/curr/succ`; return all actual lunar events in the target month, with neighbors available explicitly.
2. `getMotoPianetiOggi` in `moto_pianeti.js` currently calls ingress lookup rather than motion lookup.
3. Existing chart render controller includes some **commented-out** lookup code; mark these as historical compatibility scenarios, not confirmed live callers. Its `getMotiSinodiciPianetiPerDataoraUtc`, `getSinodiPerDataoraUtc`, daily ingress and motion event calls are active in inspected excerpts.
4. Phase code convention differs between legacy lower-case and `legacySynodicMotionRepository` uppercase; translate in adapter explicitly.
5. `TIMESTAMP` (motion/ingress) is treated as UTC by much of the backend despite lacking time-zone metadata, whereas `sinodi` uses `TIMESTAMPTZ`. Test the source data assumptions.
6. `getDirezioneMotoPianetiPerData` intentionally uses **end of day**, not a generic instant; parity tests must compare equivalent instants.
7. Source generator scripts have known incomplete/legacy branches; **the 380,844 counts do not establish full temporal coverage**. Never backfill missing records by guessing.

## Parity acceptance checklist

- For every body present, compare exact source rows and requested ranges, including record ordering, null extras, sign/direction and timestamp second/microsecond precision.
- Confirm last-before, exact-boundary, first-after, no-next, before-first, after-last and gap queries.
- Compare all date-local searches across UTC and Europe/Rome at DST changes, inclusive/exclusive range ends and leap years.
- Compare monthly Moon events with months containing two new moons and cycles straddling month boundaries.
- Verify matching case/label aliases and source phase logic only where actual phase data exist.
- Separate approved behavior changes from accidental compatibility regressions, and document them in the PR.
