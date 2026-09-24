# SpiderNet — Startseite (Obsidian)

> Für Obsidian: Ordner `documentation/` (oder das ganze Repo) als Vault
> öffnen; diese Notiz ist der Einstieg. Alle Verbindungen laufen über
> `[[Wikilinks]]`.

## Projekt

**SpiderNet** ist **nur die Software** für ein Nachbarschaftsnetz: Häuser
werden per Glasfaser (neben den bestehenden Internetanschlüssen) direkt
verbunden — welcher Link-Typ und welche Geschwindigkeit, entscheidet jede
Nachbarschaft selbst (10 Gbit, 1 Gbit, 1,6 Tbit, Funk — alles möglich).
Die Bandbreite aller Anschlüsse wird gepoolt, sodass jeder Haushalt vom
gesamten Netz profitiert. **Wir providen nur die Software**, jede
Nachbarschaft betreibt ihre eigene Instanz nach eigenen Regeln.

## Navigation

| Notiz | Worum es geht |
|---|---|
| [[Ethos]] | Werte: Gemeingut, Fairness, Leechen erlaubt, legale Basis |
| [[Idee]] | Kernidee + ehrlicher Realitätscheck |
| [[Architektur]] | Schichten: Physik → Routing → Pooling → Freenet-Inhalte |
| [[Overlay]] | Yggdrasil als Overlay: Auswahl, Deployment, Grenzen |
| [[Recht]] | Deutsche Rechtslage + Pflichtregeln für die Software |
| [[Splitting]] | Was sich splitten lässt: Ranges, Flow-Verteilung, Realtime |
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
- [[Idee]] (Realitätscheck) ↔ [[Architektur]] (link-agnostische Physik, `sn-fetch`)
- [[Splitting]] ↔ [[Architektur]] (Schicht 2) — was der Exit-Pool tatsächlich nutzen kann
- [[Overlay]] ↔ [[Architektur]] (Schicht 1) — Routing-Entscheidung und deren Grenzen
- [[Overlay]] ↔ [[Server-README]] — `sn-node` generiert die Yggdrasil-Config, `sn-fetch` nutzt `Egress::Proxy`
- [[Lizenz]] ↔ [[Ethos]] §6 (Transparenz) und `freenet-core` (AGPL)

## Außerhalb der Doku

- [[Server-README]] — Software-Konzept in `server/`
- `AGENTS.md` — Arbeitsregeln für Coding-Agents
- `scripts/bootstrap-gitea.sh` — Repo-Setup
