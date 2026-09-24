# Roadmap

Kleine Schritte, alles muss für sich allein funktionieren. Jede Phase
endet mit etwas, das man benutzen kann.

## Phase 0 — Grundlagen (jetzt)

- [x] Doku-Grundstruktur (Ethos, Idee, Architektur, Recht, Roadmap)
- [x] AGENTS.md mit Projektregeln
- [x] Gitea-Repo + initialer Push
- [ ] Prior Art vertiefen: Freifunk-Community in der Nähe kontaktieren
- [ ] ISP-AGB-Check für die Teilnehmer-Haushalte

## Phase 1 — Pilot (2–3 Haushalte)

Ziel: Beweis, dass Pooling über zwei Anschlüsse real funktioniert.
Handbuch mit Messprotokoll: [[Pilot]].

- [x] `sn-fetch` PoC (lokal): Range-Probe, gewichteter Planner, parallele
      Segment-Downloads, Reassembly + SHA-256 — verifiziert gegen einen
      echten Server (Debian-Mirror); Multi-Exit-Routing folgt mit dem Mesh
- [x] `sn-fetch`: Segment-Retry mit Exit-Reassignment, Progress-Sinks,
      per-Exit-Egress (`Direct`/`Proxy`) — Nachbar-Exit über HTTP-Proxy
      end-to-end getestet
- [x] `sn-node` v0: Yggdrasil-Config-Generierung (`genconf`) + Admin-Socket-
      Client (`status`) — die Overlay-Anbindung beginnt
- [ ] Yggdrasil-Overlay zwischen 2 Haushalten (Kabel oder Kurzstrecke
      WLAN/60-GHz), `sn-node`-Daemon orchestriert den Node
- [x] `sn-exit` v0: HTTP-Proxy auf der Yggdrasil-Adresse (Kontingente,
      SSRF-Schutz, Kill-Switch = Prozess stoppen)
- [x] `sn-fair` v0: Fairness-Engine (gewichtetes Max-Min über zwei
      Klassen, Leech-Politik) + Demo-CLI; Runtime-Integration in
      `sn-fetch` ist Phase 2
- [ ] `sn-fetch` PoC über 2 echte Exits, plus Baseline-Messung
      (1-Exit vs. 2-Exit, gleiche Datei — Protokoll in [[Pilot]])
- [ ] Doku: Messwerte, Erkenntnisse, Stolpersteine → in [[Pilot]] §Ergebnisse

## Phase 2 — MVP-Software

Ziel: ein Paket, das 3–10 Haushalte installieren können.

- [ ] `sn-node` Daemon v1: Metriken aus Yggdrasil-Peering (per-Exit
      Bandbreite/Latenz), Onboarding-Flow (QR/Knopfdruck)
- [ ] Exit-Pool: Kontingente, opt-in/opt-out, Leech-Modus, Kill-Switch
- [ ] `sn-fetch`: API + CLI + einfache Browser-Integration
- [ ] `sn-cache`: lancache-artiger transparenter Cache (DNS-basiert)
- [ ] Fairness-Scheduler v1 (max-min, Exit-Kontingente, Karma)
- [ ] Telemetrie: keine zentrale Sammlung; lokale Metriken im Node

## Phase 3 — Nachbarschafts-Rollout

- [ ] Onboarding-Doku für Nachbarn (Hardware-Einkaufsliste, Schritt-für-Schritt)
- [ ] Physische Ausbaupläne pro Häuserblock (Glasfaser wo gegraben wird,
      Richtfunk als Standard-Alternative)
- [ ] Gemeinschafts-Verträge: Gestattungsverträge für Kabel über Grundstücke,
      Nachbarschafts-Abkommen (Pico-Peering-artig)

## Phase 4 — Inhalte & Apps (Freenet)

- [ ] `freenet-core` als Sidecar auf 1–2 Nodes
- [ ] Nachbarschafts-Wiki / lokale Dienste als Freenet-Contracts
- [ ] Offline-Fähigkeit: Netz funktioniert ohne Internet (lokale Dienste)

## Phase 5 — Vernetzung darüber hinaus

- [ ] Peering mit Freifunk-Clouds oder anderen Nachbarschaftsnetzen
- [ ] Echtes Backbone: eigene Glasfaser in Straßen, ggf. IXP-Peering
      (langfristig, NYC-Mesh-Stil)
- [ ] Federated Viertel: mehrere SpiderNet-Netze verbinden sich
