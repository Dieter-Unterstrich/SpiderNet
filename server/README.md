# AKI Node — Server/Software (Konzeptphase)

Die Software, die aus "Nachbarschafts-LAN" ein gepooltes Bandbreitennetz
macht. Sprache: **Rust**. Status: **Konzeptphase — noch nichts implementiert.**

## Zielbild

Pro Haushalt läuft ein AKI-Node (Mini-PC oder Einplatinenrechner), der:

1. Das Nachbarschafts-Overlay bildet (Yggdrasil als erste Wahl)
2. Seinen eigenen ISP-Uplink als **Exit** anbietet (opt-in, Kontingent)
3. Downloads über **alle verfügbaren Exits** segmentiert und parallel lädt
4. Traffic fair zwischen allen Teilnehmenden aufteilt (max-min Fairness)
5. Einen geteilten Nachbarschafts-Cache bedient (lancache-Prinzip)
6. (später) `freenet-core` als Sidecar laufen lässt für verteilte Inhalte/Apps

## Geplante Komponenten

| Crate/Binary | Zweck |
|---|---|
| `aki-node` | Daemon: Discovery, Peering, Metriken, Onboarding |
| `aki-exit` | Exit-Dienst: kontingentierter Uplink-Proxy für Nachbarn (verschlüsselt) |
| `aki-fetch` | Segmentierter Multi-Exit-Downloader (HTTP-API + CLI) |
| `aki-fair` | Fairness-Scheduler (max-min, Exit-Kontingente, Karma) |
| `aki-cache` | Transparenter Nachbarschafts-Cache (DNS-basiert) |
| `aki-freenet` | Integration von `freenet-core` (Contracts für Nachbarschafts-Inhalte) |

## MVP (Phase 2 der [Roadmap](../documentation/ideen/roadmap.md))

Kleinster brauchbarer Schnitt:

1. Zwei Nodes peeren (Yggdrasil)
2. Ein Download wird über beide Uplinks gleichzeitig per HTTP-Range
   gesplittet, reassembliert, hash-verifiziert
3. Fair-Share: zwei gleichzeitige Downloads teilen sich die Gesamt-
   kapazität fair
4. Exit opt-in per Konfig-Datei; Kill-Switch (Exit aus = Node nutzt nur
   den eigenen Uplink)

## Nicht-Ziele (bewusst)

- Keine Bezahl-/Krypto-Infrastruktur (Kontrast: Althea)
- Keine Anonymitätsversprechen (Freenet selbst bietet keine; wir auch nicht)
- Kein Zentralserver, keine zentrale Datensammlung
- Kein "Piracy as a Feature" (siehe `documentation/ideen/recht.md`)

## Technologiestack (Vorschlag)

- Rust (tokio), `axum` für APIs
- Yggdrasil als Overlay (später evtl. Babel + WireGuard)
- `freenet-stdlib` / WebSocket-API für Freenet-Integration
- HTTP Range Requests + SHA-256-Verifizierung für Segmentierung
- Lizenz: AGPL-3.0 (freenet-core ist AGPL-3.0; Nutzung über stdlib/API
  erlaubt trotzdem freie Wahl — wir wählen aus Ethos-Gründen AGPL)
