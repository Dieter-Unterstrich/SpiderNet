# SpiderNet

**Teile die Internetbandbreite deiner Nachbarschaft — ohne Mehrkosten.**

SpiderNet ist **Software** für ein Nachbarschaftsnetz: Häuser werden
direkt verbunden (Glasfaser, LAN, Funk — was da ist), und jeder Haushalt
profitiert von der **Summe aller Internetanschlüsse** im Viertel. Große
Downloads werden segmentiert und über alle Anschlüsse parallel geladen
— schwache Anschlüsse werden stärker, niemand zahlt mehr. Als
Nebenprodukt verteilt sich ausgehender Traffic auf viele Haushalte,
was Rückverfolgbarkeit für Außenstehende erschwert.

> **KEINE GARANTIE.** Diese Software wird "as is" ohne jegliche Garantie
> bereitgestellt (AGPL §§ 15/16). Was Nutzer:innen damit übertragen,
> liegt in ihrer Verantwortung — wir übernehmen keine Haftung.

Wir providen **nur die Werkzeuge**: Jede Nachbarschaft richtet ihre
eigene Instanz ein und entscheidet selbst über Topologie, Link-Typen,
Geschwindigkeiten und Hausregeln. Es kann beliebig viele unabhängige
SpiderNet-Instanzen geben — kein Zentralserver, keine zentrale
Datensammlung, keine Bezahlung.

## Struktur des Repos

| Ordner/Datei | Inhalt | Einstieg |
|---|---|---|
| `documentation/` | Idee, Ethos, Architektur, Rechtslage, Roadmap (Deutsch, Obsidian-freundlich mit `[[Wikilinks]]`; `documentation/Home.md` ist die Map of Content) | [README](documentation/README.md) |
| `server/` | Die Software (Rust): `sn-fetch`, `sn-node`, `sn-exit`; geplant `sn-fair`, `sn-cache`, `sn-freenet` | [README](server/README.md) |
| `scripts/` | Repo-Setup (`bootstrap-gitea.sh`) | — |
| `AGENTS.md` | Arbeitsanweisungen für Coding-Agents und Contributor:innen | [AGENTS.md](AGENTS.md) |
| `LICENSE` | Code-Lizenz: AGPL-3.0-only | [LICENSE](LICENSE) |
| `documentation/LICENSE` | Doku-Lizenz: CC-BY-SA-4.0 | [LICENSE](documentation/LICENSE) |

## Die Idee in einem Satz

Häuser werden direkt per Glasfaser verbunden; jede Anfrage ins Internet
kann über **alle** Anschlüsse des Viertels gleichzeitig laufen
(segmentiert und fair gemesht), sodass die effektive Bandbreite ~ der
Summe aller Anschlüsse ist — ohne Mehrkosten und mit besserer
Versorgung schwacher Haushalte.

## Status

Prototyp (Konzeptphase + erster Code):

- **`sn-fetch` PoC** funktioniert: segmentierter Multi-Exit-Download
  (Range-Probe, gewichteter Planner, parallele Segmente, Retry mit
  Exit-Reassignment, Reassembly + SHA-256) — verifiziert gegen einen
  echten Server (Debian-Mirror, 756 MB, Hash-Check) und gegen einen
  lokalen Proxy für Nachbar-Exits. Details: [server/README.md](server/README.md)
- **`sn-node` v0** funktioniert: Yggdrasil-Config-Generierung (`genconf`)
  + Overlay-Status (`status` via Admin-Socket)
- **`sn-exit` v0** funktioniert: kontingentierter Uplink-Proxy (Byte-Relay,
  CONNECT-Tunnel für HTTPS, SSRF-Schutz, Source-Allowlist) — End-to-End
  mit `sn-fetch` gegen einen echten Server verifiziert
- **Nächste Schritte:** Pilot mit 2–3 Haushalten (Yggdrasil-Overlay
  verkabeln, Messwerte), Fairness-Scheduler — siehe
  [Roadmap](documentation/ideen/roadmap.md)

## Doku lesen (Obsidian)

Ordner `documentation/` (oder das ganze Repo) als Obsidian-Vault öffnen
und mit `documentation/Home.md` starten. Alternativ genügen die
Markdown-Dateien in `documentation/` auch so.

## Nicht-Ziele

- Keine Bezahl-/Krypto-Infrastruktur (Kontrast: Althea)
- Keine Anonymitätsversprechen (Yggdrasil und Freenet bieten keine)
- Keine inhaltliche Kontrolle oder Zensur durch die Software selbst
- Kein zentraler Exit, kein zentraler Server

## Verwandte Projekte (Prior Art)

[Freifunk](https://freifunk.net/) · [NYC Mesh](https://www.nycmesh.net/) ·
[guifi.net](https://www.guifi.net/) ·
[Althea](https://www.althea.net/) (Kontrast: Bezahl-Modell) ·
[Yggdrasil](https://yggdrasilnetwork.github.io/) (Overlay) ·
[lancache](https://lancache.net/) (Cache-Prinzip)

## Lizenz

- Code: [AGPL-3.0-only](LICENSE)
- Dokumentation: [CC-BY-SA-4.0](documentation/LICENSE)
