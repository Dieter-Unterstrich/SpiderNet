# SpiderNet — Startseite (Obsidian)

> Für Obsidian: Ordner `documentation/` (oder das ganze Repo) als Vault
> öffnen; diese Notiz ist der Einstieg. Alle Verbindungen laufen über
> `[[Wikilinks]]`.

## Projekt

**SpiderNet** — ein Nachbarschaftsnetz: Häuser werden per Glasfaser
(neben den bestehenden Internetanschlüssen, Ziel 10 Gbit/s) direkt
verbunden. Die Bandbreite aller Anschlüsse wird gepoolt, sodass jeder
Haushalt vom gesamten Netz profitiert.

## Navigation

| Notiz | Worum es geht |
|---|---|
| [[Ethos]] | Werte: Gemeingut, Fairness, Leechen erlaubt, legale Basis |
| [[Idee]] | Kernidee + ehrlicher Realitätscheck |
| [[Architektur]] | Schichten: Physik → Routing → Pooling → Freenet-Inhalte |
| [[Recht]] | Deutsche Rechtslage + Pflichtregeln für die Software |
| [[Lizenz]] | Lizenz-Optionen (Code & Doku) |
| [[Roadmap]] | Phasen: Pilot → MVP → Rollout → Freenet → Backbone |
| [[Server-README|Server-Konzept]] | Software-Komponenten, Code-Standards |

## Lesereihenfolge zum Einstieg

1. [[Idee]] — was wollen wir eigentlich
2. [[Ethos]] — welche Regeln geben uns vor
3. [[Architektur]] — wie bauen wir es technisch
4. [[Recht]] — was können wir (nicht) tun
5. [[Roadmap]] — wie kommen wir dahin

## Querverbindungen zwischen den Notizen

- [[Ethos]] §2 (Fairness/Leechen) ↔ [[Architektur]] (Leech-Modus, `sn-fair`)
- [[Recht]] §5 (Pflichtregeln) ↔ [[Server-README]] (Haftungsausschluss)
- [[Idee]] (Realitätscheck) ↔ [[Architektur]] (10G-Faser, `sn-fetch`)
- [[Lizenz]] ↔ [[Ethos]] §6 (Transparenz) und `freenet-core` (AGPL)

## Außerhalb der Doku

- [[Server-README]] — Software-Konzept in `server/`
- `AGENTS.md` — Arbeitsregeln für Coding-Agents
- `scripts/bootstrap-gitea.sh` — Repo-Setup
