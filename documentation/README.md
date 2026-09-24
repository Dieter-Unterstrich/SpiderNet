# SpiderNet — Dokumentation

Ein Nachbarschaftsnetz aus Glasfaser, in dem sich alle Haushalte ihre
Internetbandbreite teilen: **vom Internetanschluss zum Gemeingut.**

SpiderNet ist **nur die Software** — jedes Viertel richtet seine eigene
Instanz ein und entscheidet selbst, wie es sie betreibt (Topologie,
Link-Typen und -Geschwindigkeiten, Regeln). Es kann beliebig viele
unabhängige SpiderNet-Instanzen geben; wir providen nur die Werkzeuge.

## Inhaltsverzeichnis (Obsidian-Map of Content)

- [[Ethos]] — Werte, Prinzipien und "Hausregeln" des Netzes
- [[Idee]] — Die Kernidee und ein ehrlicher Realitätscheck
- [[Architektur]] — Technische Architektur (Physik → Routing → Pooling → Inhalt)
- [[Recht]] — Rechtslage in Deutschland (Störerhaftung, Abmahnrisiko, Verkabelung)
- [[Splitting]] — Was sich über mehrere Exits splitten/verteilen lässt (und was nicht)
- [[Lizenz]] — Lizenz-Optionen und Empfehlung
- [[Roadmap]] — Phasenplan vom Pilot bis zum Nachbarschafts-Backbone
- [[Pilot]] — Pilot-Handbuch: Schritt-für-Schritt + Messprotokoll (Phase 1)
- [[Server-README|Server-Konzept]] — geplante Software (`sn-node`, `sn-fetch`, …)

## Die Idee in einem Satz

Häuser werden direkt per Glasfaser verbunden; jede Anfrage ins Internet kann
über **alle** Anschlüsse des Viertels gleichzeitig laufen (segmentiert und
fair gemesht), sodass die effektive Bandbreite ~ der Summe aller Anschlüsse
ist — ohne Mehrkosten und mit besserer Versorgung schwacher Haushalte.

## Status

Prototypphase (siehe [[Roadmap]]): `sn-fetch` (segmentierter Multi-Exit-
Download mit Retry + Egress), `sn-node` v0 (Yggdrasil-Config + Status),
`sn-exit` v0 (kontingentierter Uplink-Proxy mit SSRF-Schutz) und `sn-fair`
v0 (Fairness-Engine + Demo-CLI) sind implementiert und getestet —
Details in [`server/README.md`](../server/README.md). Nächster
Meilenstein: Pilot mit 2–3 Haushalten (Handbuch: [[Pilot]]).

## Verwandte Projekte (Prior Art)

- [Freifunk](https://freifunk.net/) — deutschlandweite Community-Netz-Initiative
- [NYC Mesh](https://www.nycmesh.net/) — Mitglieder teilen ihre Anschlüsse, Supernodes + Peering
- [guifi.net](https://www.guifi.net/) — Spanien, Community-Fiber in großem Maßstab
- [Althea](https://www.althea.net/) — Mesh mit Bezahl-Protokoll (Kontrast: wir sind bewusst kostenlos/fair-share)
- [Yggdrasil](https://yggdrasilnetwork.org/) — Overlay-Routing-Kandidat für unsere Ebene 2
- [lancache](https://lancache.net/) — Nachbarschafts-Cache-Prinzip für große Downloads
