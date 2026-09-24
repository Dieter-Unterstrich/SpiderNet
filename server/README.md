# SpiderNet-Node — Server/Software

Die Software, die aus "Nachbarschafts-LAN" ein gepooltes Bandbreitennetz
macht. Sprache: **Rust**.

## Status

**`sn-fetch` PoC v0 ist implementiert** (siehe `crates/sn-fetch/`):

- Range-Probe (`HEAD` + 1-Byte-GET-Fallback) erkennt `Accept-Ranges`
- Gewichteter Planner (Exits nach Kapazität, optionale Sub-Segmente pro
  Exit, Mindest-Segmentgröße mit Merge-Fix, validated: lückenlose,
  vollständige Abdeckung)
- Parallele Segment-Downloads (Streaming, eigener reqwest-Client pro Exit)
- **Retry mit Exit-Reassignment:** fehlgeschlagene Segmente werden (wieder-
  holbar, Standard 3 Versuche) bevorzugt auf einem anderen Exit wiederholt;
  nicht-retrybare Fehler (4xx, korrupte Parts) brechen sofort ab — ein
  halbfertiges File wird nie als Erfolg ausgegeben
- **Progress:** `ProgressSink`-Events (Segment started/finished/failed,
  `BytesTracker`) + `tracing`-Logging
- **Per-Exit-Egress:** `Direct` (eigener Uplink) oder `Proxy` (HTTP-Proxy
  eines Nachbarn — der Weg für `sn-exit` über das Overlay); end-to-end
  getestet gegen einen echten Forward-Proxy
- Reassembly in Index-Reihenfolge + Streaming-SHA-256 + Längen-Check
- Fallback: range-hostile Server werden als single-stream geladen
- CLI: `sn-fetch <url> [-o out] [-e "1=2,2=1@http://[ygg]:8080"] [-s n]
  [--max-attempts n] [--sha256 hex] --probe-only`

Verifiziert gegen einen echten Server (Debian-CD-Mirror, 756 MB, 8 Segmente,
SHA-256 entspricht der offiziellen `SHA256SUMS`).

**`sn-node` v0 ist implementiert** (siehe `crates/sn-node/`):

- `sn-node genconf`: erzeugt eine Yggdrasil-Config aus validierten
  Einstellungen — Basis ist immer der echte `yggdrasil -genconf -json`
  Output (keine hartcodierten Defaults), gepatcht mit `Peers`, `Listen`,
  `MulticastInterfaces`, `AdminListen`, `AllowedPublicKeys` (Nachbarschafts-
  ACL), `IfName`, `PrivateKeyPath`; Output-Datei wird auf 600 gesetzt
- `sn-node status`: fragt den Admin-Socket ab (`getSelf`/`getPeers` über
  TCP `localhost:9001` oder Unix-Socket) — Overlay-Adresse, Subnetz,
  Version, aktive Peer-Verbindungen

**`sn-exit` v0 ist implementiert** (siehe `crates/sn-exit/`):

- HTTP-Proxy-Dienst, der auf der Yggdrasil-Adresse des Haushalts lauscht
  und Nachbar-Requests über den eigenen Uplink holt — Sharing ist opt-in
  durch Starten des Dienstes, Stoppen ist der Kill-Switch
- **Transparentes Byte-Relay:** absolute-URI-GETs (HTTP) und CONNECT-
  Tunnel (HTTPS) werden unverändert weitergeleitet; TLS bleibt end-to-end
  zwischen Neighbor-Client und Origin, Range-Header und Retries
  funktionieren (Segmentierung bleibt Sache von `sn-fetch`)
- **Quota pro Neighbor:** Bytes pro Zeitfenster (fixed window),
  erschöpft → HTTP 503 mit `Retry-After`; Accounting auch bei abgebrochenen
  Verbindungen (Counter-Wrapper, nicht Copy-Ergebnis)
- **SSRF-Schutz:** Origins, die zu Loopback/Private/Link-Local/ULA/
  Yggdrasil-Ranges auflösen, werden abgewiesen — Nachbarn können nicht in
  Heim-LAN oder Admin-Sockets des Exit-Haushalts fetchen
- Source-Allowlist: nur gelistete Neighbor-Adressen; ohne Liste nur
  Loopback (Dev-Mode)
- Verifiziert end-to-end: `sn-fetch` über einen laufenden `sn-exit`
  lädt die Debian-ISO (756 MB, 4 Segmente, SHA-256 korrekt)

**`sn-fair` v0 ist implementiert** (siehe `crates/sn-fair/`):

- Gewichtetes Max-Min-Fairness-Engine (zwei Klassen): **Contributors**
  (Haushalte mit Exit-Angebot, konfigurierbares Gewicht) teilen sich
  die Gesamtkapazität nach Max-Min-Fairness; **Leeches** (Empfang ohne
  Beitrag, ausdrücklich legitim) bekommen die Restkapazität — bei
  Sättigung zuerst gedrosselt, nie ausgeschlossen
- Deterministische, overflow-sichere Berechnung (u128-Arithmetik,
  Restverteilung in ID-Ordnung), validierte Newtypes
  (`ParticipantId`, `Weight`, `Rate`, `Capacity`)
- Demo-CLI in Mbit/s für den Piloten: Soll-Aufteilung simulieren und
  gegen die gemessene Ist-Aufteilung halten (siehe [[Pilot]], M4)
- Runtime-Integration in `sn-fetch` (echte gleichzeitige Downloads
  drosseln) ist Phase 2, siehe [[Roadmap]]

**`sn-node` Daemon v1 ist implementiert** (`sn-node daemon`, siehe
`crates/sn-node/src/daemon.rs`):

- **Ein Befehl pro Haushalt:** `sn-node daemon --config node.toml`
  orchestriert den Node — Yggdrasil-Sidecar (generierte Config +
  persistenter Node-Key im `state_dir`, Identität bleibt über Neustarts
  stabil: `PrivateKeyPath` hat Vorrang vor dem eingebetteten Key, siehe
  [Config-Referenz](https://yggdrasil-network.github.io/configurationref.html))
  + optional den in-process `sn-exit`-Dienst
- **TOML-Konfiguration, validiert beim Laden** (`deny_unknown_fields`,
  wiederverwendete validierte Typen; `sn-node daemon --check` prüft
  ohne Start — für Packaging/Ops)
- **Leech-Modus per Konfig** (`[exit] enabled = false`): Overlay läuft,
  kein Exit-Angebot; enabled + leere Allowlist = Start verweigert
  (Ausnahme: Loopback-Dev-Bind)
- **Kill-Switch-Semantik:** SIGINT/SIGTERM → Accept-Loop stoppt,
  Sidecar bekommt SIGTERM (5 s Gnade, dann SIGKILL); stirbt Yggdrasil
  unerwartet, fährt der Daemon mit Fehler herunter (Supervisor-Pflicht)
- Getestet mit Fake-Yggdrasil (Script-Binary) und Fake-Admin-Socket:
  Key-Persistenz, Exit-Bind, Shutdown, Sidecar-Tod

**Deployment: Docker & Nix** (siehe `server/docker/README.md` und
`flake.nix`):

- `server/docker/`: Multi-Stage-Image (alle 4 Binaries + Yggdrasil
  v0.5.14, SHA-256-verifiziert), Compose-Beispiel pro Haushalt
  (`network_mode: host`, `NET_ADMIN`, `/dev/net/tun`, `init: true`) —
  Image gebaut und geraucht-getestet
- `flake.nix`: `nix build` baut das Workspace als ein Paket
  (+ experimentelles NixOS-Modul) — **nicht lokal verifiziert**
  (Nix nicht installiert), Feedback willkommen

**Noch nicht drin:** Freenet-Integration, Cache, Metriken/Onboarding
(Phase 2).

Details zum Overlay: [[Overlay]]. Pilot-Handbuch mit Messprotokoll:
[[Pilot]].

## Haftungsausschluss (muss im Produkt sichtbar sein)

> **KEINE GARANTIE.** Diese Software wird "as is" ohne jegliche Garantie
> bereitgestellt (AGPL §§ 15/16). Die Betreiber und Contributor:innen dieses
> Projekts übernehmen **keinerlei Verantwortung oder Haftung** dafür, wozu
> Nutzer:innen die Software verwenden oder welche Inhalte über sie übertragen
> werden. Die Verantwortung für gesendete und empfangene Inhalte liegt
> vollständig bei den Nutzer:innen.

Dieser Hinweis muss prominent sichtbar sein: im README, im Startup-Banner
des Daemons und in der Weboberfläche.

## Zielbild

Pro Haushalt läuft ein SpiderNet-Node (`sn-node`, Mini-PC oder
Einplatinenrechner), der:

1. Das Nachbarschafts-Overlay bildet (Yggdrasil als erste Wahl)
2. Seinen eigenen ISP-Uplink als **Exit** anbietet (opt-in, Kontingent)
3. Downloads über **alle verfügbaren Exits** segmentiert und parallel lädt
4. Traffic fair zwischen allen Teilnehmenden aufteilt (max-min Fairness)
5. Einen geteilten Nachbarschafts-Cache bedient (lancache-Prinzip)
6. (später) `freenet-core` als Sidecar laufen lässt für verteilte Inhalte/Apps

## Geplante Komponenten

| Crate/Binary | Zweck |
|---|---|
| `sn-node` | Node-Glue: Yggdrasil-Config (`genconf`), Overlay-Status (`status`), Node-Daemon (`daemon`: Sidecar + Exit-Orchestrierung) |
| `sn-exit` | Exit-Dienst: kontingentierter Uplink-Proxy auf der Yggdrasil-Adresse (v0: Byte-Relay + Quota + SSRF-Schutz) |
| `sn-fetch` | Segmentierter Multi-Exit-Downloader (HTTP-API + CLI) |
| `sn-fair` | Fairness-Scheduler (max-min, Exit-Kontingente, Karma, Leech-Modus) — v0: zwei-Klassen-Max-Min-Engine + Demo-CLI |
| `sn-cache` | Transparenter Nachbarschafts-Cache (DNS-basiert) |
| `sn-freenet` | Integration von `freenet-core` (Contracts für Nachbarschafts-Inhalte) |

## MVP (Phase 1→2, siehe [[Roadmap]])

Kleinster brauchbarer Schnitt:

1. Zwei Nodes peeren (Yggdrasil)
2. Ein Download wird über beide Uplinks gleichzeitig per HTTP-Range
   gesplittet, reassembliert, hash-verifiziert
3. Fair-Share: zwei gleichzeitige Downloads teilen sich die Gesamt-
   kapazität fair
4. Exit opt-in per Konfig-Datei; Leech-Modus (Sharing aus, nur empfangen)
   jederzeit umschaltbar; Kill-Switch (Exit aus = Node nutzt nur den
   eigenen Uplink)

## Nicht-Ziele (bewusst)

- Keine Bezahl-/Krypto-Infrastruktur (Kontrast: Althea)
- Keine Anonymitätsversprechen (Freenet selbst bietet keine; wir auch nicht)
- Kein Zentralserver, keine zentrale Datensammlung
- Keine inhaltliche Kontrolle oder Zensur durch die Software selbst —
  die Software transportiert Dateien/Webseiten aller Art; was übertragen
  wird, liegt in der Verantwortung der Nutzer:innen (Haftungsausschluss oben)

## Code-Standards: maximale Type-Safety in Rust

Der Code soll so sicher sein, dass ganze Fehlerklassen schon beim Kompilieren
unmöglich werden. Verbindliche Regeln für jede Crate in `server/`:

**Maschinell erzwungen** (Workspace-Lints in `server/Cargo.toml`, in jeder
Crate via `[lints] workspace = true` aktiviert):

- `unsafe_code = "deny"` + `#![forbid(unsafe_code)]` im Crate-Root
- `clippy::unwrap_used = "deny"` — kein `unwrap()` (Panic-Quelle, kein Test)
- `clippy::expect_used = "deny"` — kein `expect()` (Panic-Quelle, kein Test)
- `clippy::panic = "deny"`, `clippy::unreachable = "deny"`
- `clippy::all = "deny"`, `clippy::pedantic = "warn"` (Deny-Ziel vor Release)

Verifiziert: ein eingebautes `unwrap()` in `lib.rs` wurde von Clippy mit
`error: used unwrap() ... #unwrap_used` abgelehnt. Tests dürfen
`unwrap()`/`expect()` (explizites `#![allow(...)]` pro Test-Crate).

1. **`#![forbid(unsafe_code)]`** in jedem Crate-Root (und `unwrap`,
   `expect` nur in `#[cfg(test)]`-Kontext).
2. **Newtypes statt roher Primitive** für alles, was Bedeutung hat:
   `struct NodeId(NonZeroU64)`, `struct BytesPerSecond(u64)`,
   `struct SegmentIndex(u32)`, `struct ExitQuotaBytes(…)` — keine nackten
   `u64`/`String` in Domänen-Typen.
3. **Exhaustive Enums statt Strings/Bool-Flags** für Zustände (z. B.
   `enum ExitState { OptedIn(Quota), Leeching, Suspended }`),
   `match` ohne `_`-Arm, damit Compiler-Checks beim Erweitern greifen.
4. **Typestate-Pattern** für Protokoll-/Lebenszyklus-Zustände (z. B.
   `Download<Started>` → `Download<Split>` → `Download<Assembled>`),
   damit illegale Zustandsübergänge nicht kompilieren.
5. **`TryFrom`/`TryInto` statt stiller Konvertierung**, keine panicking
   Operationen in nicht-Test-Code; Fehler via `thiserror`-Enums, Bubble-up
   statt Logging-and-continue.
6. **Parsed-data-is-validated:** unsichere Eingaben (Konfig, Netzwerk) werden
   direkt beim Einlesen in validierte Typen transformiert; danach trägt der
   Typ die Invariante (z. B. `struct ValidatedConfig(ConfigInner)` statt
   `Config` mit Optionals).
7. **Keine globalen `Mutex`/`static mut`**; Shared State explizit als
   Besitz-/Kanal-Struktur (`tokio::sync` primitives mit bewusster Wahl).
8. `deny(clippy::all, clippy::pedantic)`-basierte CI-Checks, sobald
   Code existiert; `cargo-deny` für Dependencies.

## Technologiestack (Vorschlag)

- Rust (tokio), `axum` für APIs
- Yggdrasil als Overlay (später evtl. Babel + WireGuard)
- `freenet-stdlib` / WebSocket-API für Freenet-Integration
- HTTP Range Requests + SHA-256-Verifizierung für Segmentierung
- Lizenz: siehe [[Lizenz]] (Empfehlung: AGPL-3.0)
