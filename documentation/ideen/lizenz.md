# Lizenz — Optionen und Empfehlung

Damit wir uns festlegen können. Stand: September 2026.

## Wichtigster Rahmen

- Die Software wird **ohne jegliche Garantie** bereitgestellt — AGPL GPLv3-
 -artige Lizenzen enthalten §§ 15/16 (No Warranty, Limitation of Liability)
  bereits; permissive Lizenzen brauchen das Disclaimer-Klauseln separat
  (Apache-2.0 hat es eingebaut, MIT/BSD nur minimal).
- Ethos: Code soll **Commons bleiben** — auch Forks sollen gezwungen sein,
  offen zu bleiben, sonst kann das Nachbarschaftsprojekt abgespaltet und
  geschlossen weiterverkauft werden.

## Code-Lizenz-Optionen

| Lizenz | Copyleft | Netz-Klausel | Passt zum Ethos? | Risiko für Nachbarschafts-Projekt |
|---|---|---|---|---|
| **AGPL-3.0** | stark | **ja** (§13 Remote-Network-Interaction) | **sehr gut** | Forks bleiben offen, "as a service"-Nutzung gezwungen offenzulegen; scare-off für kommerzielle Interessenten — gewollt |
| GPL-3.0 | stark | nein | gut | fork-protect, aber wer es nur *innerhalb eines eigenen Netzes* anbietet, muss nichts freigeben |
| LGPL-3.0 | schwach (nur Library-Boundary) | nein | mittel | geschlossen Forks leicht möglich |
| MPL-2.0 | mittel (datei-level) | nein | mittel | gut für "Firmen-freundlich"-Wunsch; aber Forks können proprietär um Dateien herumgebaut werden |
| EUPL-1.2 | stark | optional (Art. 13) | gut | kompatibel zu GPL via Abs. 6; wenig verbreitet, Komplexität |
| Apache-2.0 | keiner | nein | schlecht (ethisch) | Klon kann geschlossen werden — passt nicht zu "Gemeingut" |
| MIT / BSD | keiner | nein | schlecht | identisches Risiko, minimaler Disclaimer |

### Empfehlung

**AGPL-3.0 für alle Code-Crates in `server/`** (Status quo im Repo). Gründe:

1. Spiegelbild zum Ethos: das Netz ist Commons, der Code auch.
2. **§13 verlangt Offenlegung bei Netzwerk-Nutzung** — unsere Software *ist*
   ein Netzwerkdienst; wer eine eigene SpiderNet-Node-Variante betreibt,
   muss den Quellcode anbieten.
3. `freenet-core` ist AGPL-3.0 — gleiche Lizenzfamilie macht Integration
   einfach, falls wir mal direkt gegen die Core-API bauen statt nur über
   `freenet-stdlib`/WebSocket.
4. AGPL ist nicht für Doku/Daten relevant; Code-Repo bleibt trotzdem sauber
   (Lizenz-Matching für Developer-Komfort).

## Doku-Lizenz-Optionen

Doku (dieses Verzeichnis) kann separat lizenziert werden:

| Lizenz | Charakter | Empfehlung |
|---|---|---|
| **CC-BY-SA-4.0** | teilen-als-gleiche, Namensnennung | gut, wenn Doku auch Commons bleiben soll |
| CC-BY-4.0 | Namensnennung, Weitergabe frei | ok, wer Forks der Doku nicht gleich erzwingen will |
| CC0-1.0 | public-domain-artig | im Widerspruch zum Ethos |
| gleich mit Code (AGPL) | geht auch, ist aber unüblich für Doku | n/a |

### Empfehlung

**CC-BY-SA-4.0 für `documentation/`** — sauber getrennt vom Code, passt
zum "Wissen wird geteilt wie Bandbreite"-Ethos (Ethos §7).

## Was noch offen bleibt

- [ ] Entscheidung: Code AGPL-3.0 bestätigen
- [ ] Entscheidung: Doku CC-BY-SA-4.0
- [ ] `LICENSE`-Dateien: Code-`LICENSE` (AGPL, schon da) + `documentation/LICENSE` (CC-BY-SA) und Lizenz-Hinweis in jedem Dateikopf? (AGPL: Hinweis pro Crate reicht; Dateikopf optional)
- [ ] Nebenlizenzen von Dependencies (cargo-deny, sobald Cargo-Projekt existiert)
