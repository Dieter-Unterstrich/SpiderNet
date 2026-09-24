# Architektur

Von unten (Physik) nach oben (Inhalte). Der SpiderNet-Node (`sn-node`) ist
das Herzstück — eine kleine Box im Wohnungsregal, die zu Hause sitzt und das
Viertel verbindet.

## Übersicht

```mermaid
flowchart TD
    subgraph HausA[Haus A]
        GA[Geräte A] --> NA[sn-node A]
    end
    subgraph HausB[Haus B]
        GB[Geräte B] --> NB[sn-node B]
    end
    subgraph HausC[Haus C]
        GC[Geräte C] --> NC[sn-node C]
    end
    NA <-->|"Glasfaser 10 Gbit/s"| NB
    NB <-->|"Glasfaser 10 Gbit/s"| NC
    NA -->|"ISP-Uplink A"| WA((Internet))
    NB -->|"ISP-Uplink B"| WA
    NC -->|"ISP-Uplink C"| WA
```

Entscheidend:

1. **Jeder Haushalt behält seinen eigenen Uplink.** Es gibt keinen zentralen
   Exit (anders als klassische Community-Meshes mit einem Supernode), sondern
   viele — das verteilt nicht nur Last, sondern auch Risiko und Zensur-
   Angriffsfläche.
2. **Die lokalen Links sind ein *zweites* Netz neben den Internetanschlüssen.**
   Typ und Geschwindigkeit (10 Gbit/s, 1 Gbit/s, 1,6 Tbit/s, Funk) sind Sache
   der Nachbarschaft — die Software ist **link-agnostisch**: Sie misst die
   Kapazität der vorhandenen Links und plant Splits/Scheduling auf Messwerten,
   nicht auf Annahmen. Engpass sind die Uplinks; die lokalen Links werden so
   geplant, dass sie nicht zum Flaschenhals werden.

## Schicht 0 — Physik (lokale Verbindungen, link-agnostisch)

Die Software macht keine Annahmen über die Physik — sie misst, was da ist.
Beispiele, was eine Nachbarschaft einsetzen kann:

- **Glasfaser Haus-zu-Haus** (2-Faser-Duplex G.657A2, LC, SFP/SFP+/QSFP+
  je nach Wunsch-Geschwindigkeit — 1 G, 10 G, 100 G+)
- **Existierendes LAN-Kabel** im Mehrfamilienhaus (bis 100 m)
- **Richtfunk 60-GHz-PTP** (mehrere Gbit/s, Sichtlinie nötig)
- **Normales WLAN/Ethernet**, was schon da ist — langsamer, aber null Aufwand

Die Software plant mit dem, was existiert; mehr Kapazität ergibt automatisch
mehr Pooling-Gewinn.

## Schicht 1 — Routing (Overlay über alle Haushalte)

Kandidaten (Auswahl offen, Kriterium: simpel, wartbar, verschlüsselt):

| Kandidat | Pro | Con |
|---|---|---|
| **Yggdrasil** (gewählt) | Out-of-the-box e2e-verschlüsseltes IPv6-Overlay, null Konfiguration, self-contained | experimentell, Performance-Grenzen bei hohen Gbit/s |
| Babel + WireGuard | bewährt (Althea-Ansatz), Router-Hardware tauglich | mehr Konfigurationsaufwand |
| cjdns | Battle-tested in Hyperboria | ältere Codebasis |

Jeder `sn-node` peert über die Glasfaser-Links mit seinen Nachbarn; Geräte
der Haushalte hängen per NAT/Firewall getrennt dahinter (Home-LAN ≠ Mesh-LAN).

## Schicht 2 — Bandbreiten-Pooling (der eigentliche Kern)

### Exit-Pool

Jeder Haushalt bietet seinem Node an, wie viel Uplink er "spendiert"
(Kontingent, standardmäßig fair-share). Der Exit-Pool ist die aggregierte
Kapazität. Exit ist **opt-in pro Haushalt und jederzeit widerruflich**.

### Leech-Modus

Wer seinen Uplink nicht teilen will (oder kann), schaltet Sharing einfach
aus und läuft im **Leech-Modus**: rein empfangen, nichts beitragen. Das ist
ausdrücklich legitim — "Netz unter Nachbarn" heißt Nachsicht, nicht
Gegenseitigkeitszwang. Technisch gilt:

- Leeched Nodes bekommen im Fairness-Scheduler die niedrigste (aber
  weiterhin faire) Priorität, wenn Kapazität knapp ist.
- Bei Überlast wird Leeched-Nodes zuerst gedrosselt, ohne sie auszuschließen.
- Der Status ist sichtbar (Karma/Anzeige), aber es gibt keine Strafe und
  keine Zwangsfreischaltung.

### Segmentierter Multi-Exit-Download (`sn-fetch`)

```mermaid
sequenceDiagram
    participant User
    participant Node (HAUS)
    participant Exits (alle Haushalte)
    participant Origin (Origin-Server)

    User->>Node: GET bigfile.iso
    Node->>Node: Plan (Dateigröße, Range-Splits, Exit-Kontingente)
    par parallel über alle Exits
        Node->>Exits: Range 0–25%
        Node->>Exits: Range 25–50%
        Node->>Exits: Range 50–75%
        Node->>Exits: Range 75–100%
    end
    Exits->>Origin: (je ein GET mit Range)
    Origin-->>Exits: Daten
    Exits-->>Node: Segmente (verschlüsselt über Mesh)
    Node-->>User: Datei (Hash-verifiziert, reassembliert)
```

- HTTP Range Splits + Segment-Verifizierung (Content-Hash)
- Fallback: entfällt ein Exit, übernimmt ein anderer das Segment
- Für nicht-rangefähige Quellen: klassische Proxy-Rotation

### Generischer Traffic (alles andere)

VPN-artiges Overlay: pro Gerät ein verschlüsselter Tunnel zu *einem*
gewählten Exit, Exit-Wahl rotiert; Browsing/Video-Calls laufen über einen
Exit, große Transfers über alle. Video-Calls werden explizit auf einen
Exit gepinnt (kein Splitting).

### Fairness-Scheduler (`sn-fair`)

- Max-min Fairness über alle aktiven Nutzer:innen des Netzes
- Exit-Kontingente (Volumenlimit, Wartungszeit) werden respektiert
- Leech-Nodes: niedrigste Priorität bei Knappheit, nie ausgeschlossen

### Nachbarschafts-Cache (`sn-cache`)

- Transparenter Shared-Cache (lancache-Prinzip): große, populäre Objekte
  (Steam, Konsolen-Updates, OS-Images, Software-Pakete) kommen aus dem
  Viertel statt aus dem Internet
- DNS-basiert, pro Haushalt kleine Cache-Slots; gemeinsam sind sie groß

## Schicht 3 — Inhalte & Apps (Freenet)

- `freenet-core` (Rust, AGPL-3.0) läuft als Sidecar auf ausgewählten Nodes
  und stellt verteilte Inhalte/Apps im Viertel bereit: Nachbarschafts-Wiki,
  Microblogging, lokale Services — ohne zentralen Server, auch offline
  erreichbar.
- Freenet ist **keine Anonymitätslösung** (siehe FAQ) und wird auch nicht
  als solche beworben.
- Unsere Software nutzt Freenet über `freenet-stdlib` bzw. die
  WebSocket-API (nicht-derivativ); die Lizenzwahl steht in [[Lizenz]].

## Sicherheit und Vertrauen

- Nodes authentifizieren sich gegenseitig per Keypair; Onboarding im
  persönlichen Gespräch (Nachbar:innen sind der Trust-Anchor, nicht PKI).
- Strikte Netz-Trennung: Mesh-LAN hat keinen Zugriff auf Home-LAN der
  Teilnehmer (Client-Isolation).
- Kill-Switch: jeder Haushalt kann seine Faser physisch/softwareseitig
  abklemmen; der Node fällt auf "nur eigener Anschluss" zurück.
- Minimale Logs (DSGVO, siehe [[Recht]]).

## Offene Fragen

- DSLite/Carrier-NAT-Haushalte: Synchrone Exits gehen, aber
  Konfigurationsaufwand pro Router-Modell prüfen.
- ISP-AGB: Weitergabe an "Dritte" kann verboten sein (per ISP prüfen,
  siehe [[Recht]]).
- Wie viel Cache lohnt sich real bei 3–10 Haushalten? (Messung in Phase 2)
