# Dokumentation — Anarcho-Kommunistisches Internet

Ein Nachbarschaftsnetz aus Glasfaser, in dem sich alle Haushalte ihre
Internetbandbreite teilen: **vom Internetanschluss zum Gemeingut.**

## Inhaltsverzeichnis

| Datei | Inhalt |
|---|---|
| [ethos.md](ethos.md) | Werte, Prinzipien und "Hausregeln" des Netzes |
| [ideen/idee.md](ideen/idee.md) | Die Kernidee und ein ehrlicher Realitätscheck |
| [ideen/architektur.md](ideen/architektur.md) | Technische Architektur (Physik → Routing → Pooling → Inhalt) |
| [ideen/recht.md](ideen/recht.md) | Rechtslage in Deutschland (Störerhaftung, Abmahnrisiko, Verkabelung) |
| [ideen/roadmap.md](ideen/roadmap.md) | Phasenplan vom Pilot bis zum Nachbarschafts-Backbone |

## Die Idee in einem Satz

Häuser werden direkt per Glasfaser verbunden; jede Anfrage ins Internet kann
über **alle** Anschlüsse des Viertels gleichzeitig laufen (segmentiert und
fair gemesht), sodass die effektive Bandbreite ~ der Summe aller Anschlüsse
ist — ohne Mehrkosten und mit besserer Versorgung schwacher Haushalte.

## Status

Konzeptphase. Die Software in `server/` ist noch nicht implementiert; der
erste Meilenstein ist ein PoC "segmentierter Download über zwei Haushalte"
(siehe [Roadmap](ideen/roadmap.md)).

## Verwandte Projekte (Prior Art)

- [Freifunk](https://freifunk.net/) — deutschlandweite Community-Netz-Initiative
- [NYC Mesh](https://www.nycmesh.net/) — Mitglieder teilen ihre Anschlüsse, Supernodes + Peering
- [guifi.net](https://www.guifi.net/) — Spanien, Community-Fiber in großem Maßstab
- [Althea](https://www.althea.net/) — Mesh mit Bezahl-Protokoll (Kontrast: wir sind bewusst kostenlos/fair-share)
- [Yggdrasil](https://yggdrasilnetwork.org/) — Overlay-Routing-Kandidat für unsere Ebene 2
- [lancache](https://lancache.net/) — Nachbarschafts-Cache-Prinzip für große Downloads
