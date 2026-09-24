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
- Reassembly in Index-Reihenfolge + Streaming-SHA-256 + Längen-Check
- Fallback: range-hostile Server werden als single-stream geladen
- CLI: `sn-fetch <url> [-o out] [-e "1=2,2=1"] [-s n] [--sha256 hex] --probe-only`

Verifiziert gegen einen echten Server (Debian-CD-Mirror, 756 MB, 8 Segmente,
SHA-256 entspricht der offiziellen `SHA256SUMS`).

**Noch nicht drin:** Multi-Exit-Routing über das Mesh (aktuell laufen alle
Exits als lokale Verbindungen — die Struktur [`HttpExit`]-Trait,
Exit-Registry] ist dafür schon bereit), Retry bei Segment-Fehlern,
Progress-Anzeige während des Laufs.

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
| `sn-node` | Daemon: Discovery, Peering, Metriken, Onboarding |
| `sn-exit` | Exit-Dienst: kontingentierter Uplink-Proxy für Nachbarn (verschlüsselt) |
| `sn-fetch` | Segmentierter Multi-Exit-Downloader (HTTP-API + CLI) |
| `sn-fair` | Fairness-Scheduler (max-min, Exit-Kontingente, Karma, Leech-Modus) |
| `sn-cache` | Transparenter Nachbarschafts-Cache (DNS-basiert) |
| `sn-freenet` | Integration von `freenet-core` (Contracts für Nachbarschafts-Inhalte) |

## MVP (Phase 2 der [[Roadmap]])

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
