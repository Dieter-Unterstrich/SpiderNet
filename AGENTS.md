# AGENTS.md — SpiderNet

> Arbeitsanweisungen und Ziele für Coding-Agents und Contributor:innen.
> Lies diese Datei zuerst, bevor du etwas am Projekt änderst.

## Vision

Ein Nachbarschaftsnetz: Häuser werden per Glasfaser (oder notfalls Funk)
direkt miteinander verbunden. Die Bandbreite aller Internetanschlüsse wird
gepoolt, sodass jeder Haushalt vom gesamten Netz profitiert — schwache
Anschlüsse werden stärker, niemand zahlt mehr, und ausgehender Traffic wird
auf viele Hausanschlüsse verteilt, was Backtracking für außenstehende
erschwert (Privacy als Nebenprodukt, nicht als Produktpromise).

Das Projekt besteht aus zwei Hälften:

1. `documentation/` — Idee, Ethos, Architektur, Rechtslage, Roadmap
2. `server/` — die Software ("Node"), die das technisch möglich macht

## Konventionen

- Sprache: Dokumentation auf Deutsch, Code/Kommentare/Commit-Messages auf Englisch
- Markdown für Doku, keine Binärdateien ins Repo
- Code-Lizenz: AGPL-3.0 (passt zum Ethos; `freenet-core` ist ebenfalls AGPL-3.0)
- Keine Secrets ins Repo (Passwörter, Token, private IP-Listen realer Nachbarn)
- Persönliche Daten von Nachbarn (Namen, Adressen) bleiben außerhalb des Repos
- **Keine Garantie:** Die Software wird ohne jegliche Garantie bereitgestellt
  (AGPL §§ 15/16) und muss überall prominent so gekennzeichnet sein — Nutzung
  auf eigene Verantwortung der Nutzer:innen, wir übernehmen keine Haftung
  für die Verwendung.
- **Maximale Type-Safety in Rust** (Details in `server/README.md`): kein
  `unsafe` (`#![forbid(unsafe_code)]`), Newtypes statt roher Primitive,
  exhaustive Enums/Match, keine `unwrap()`/`expect()` in nicht-Test-Code.

## Server (geplant)

- Sprache: Rust
- Dezentraler Kern: [Freenet 2023](https://freenet.org/) (`freenet-core`,
  AGPL-3.0, <https://github.com/freenet/freenet-core>) für verteilte Inhalte
  und Apps. Wichtig: Freenet bietet **keine Anonymität** (klar im
  [FAQ](https://freenet.org/about/faq/) benannt) — nicht damit werben.
- Kernfeatures: Multi-Exit-Bandbreiten-Pooling, segmentierte Downloads,
  Fairness-Scheduler, Nachbarschafts-Cache, **Leech-Modus** (Empfangen ohne
  Exit-Bereitstellung ist ausdrücklich erlaubt — Netz unter Nachbarn)
- Status: Konzeptphase, Details in `server/README.md`

## Git / Gitea

- Remote: `https://git.dieterunterstrich.de/<user>/spidernet.git`
- Branch: `main`
- Credentials liegen im credential-helper `store` — niemals ausgeben oder committen
- Repo-Setup: `scripts/bootstrap-gitea.sh` (erzeugt das Repo via Gitea-API und pusht)
- **WICHTIG:** Nur dieses eine Repo anfassen. Keine anderen Repos auf dem
  Gitea-Server verändern, löschen, forken oder beschreiben.

## Arbeitsregeln für Agents

- Erst Doku lesen (v. a. `documentation/ideen/architektur.md` und
  `documentation/ideen/recht.md`), dann ändern
- Kleine, beschreibbare Commits; keine offen gelassenen TODOs in `main`
- Keine rechtlich heiklen Features einbauen, ohne dass sie in `recht.md` bedacht sind
- Recherche-Ergebnisse gehören in die Doku (mit Quellen), nicht nur in den Chat
