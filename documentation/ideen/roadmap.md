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

- [ ] SpiderNet-Node auf Mini-PC (Raspberry Pi / N100-Box), Yggdrasil-Overlay
      zwischen 2 Haushalten (Kabel oder Kurzstrecke WLAN/60-GHz)
- [ ] `sn-fetch` PoC: segmentierter Download (HTTP-Range-Splitting) über
      2 Exits, plus Baseline-Messung (1-Exit vs. 2-Exit, gleiche Datei)
- [ ] Fair-Share-Scheduler v0: zwei gleichzeitige Downloads, faire Aufteilung
- [ ] Doku: Messwerte, Erkenntnisse, Stolpersteine → hier in das Repo

## Phase 2 — MVP-Software

Ziel: ein Paket, das 3–10 Haushalte installieren können.

- [ ] `sn-node` Daemon: Discovery, Peering, Metriken (per-Exit Bandbreite/
      Latenz), Onboarding-Flow (QR/Knopfdruck)
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
