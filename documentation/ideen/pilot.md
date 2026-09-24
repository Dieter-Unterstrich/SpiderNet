# Pilot: zwei Haushalte verbinden und messen

> Prototyp-Betrieb, **keine Garantie** (AGPL §§ 15/16) — Nutzung auf eigene
> Verantwortung, siehe [[Recht]] und [[Server-README]]. Persönliche Daten
> (Namen, Adressen, Keys realer Nachbarn) bleiben **außerhalb des Repos**.

Ziel der Phase 1 (siehe [[Roadmap]]) ist der Beweis, dass Bandbreiten-
Pooling über zwei echte Anschlüsse real funktioniert — nicht auf dem
Papier. Der Pilot liefert:

1. **Messwerte:** 1-Exit-Baseline vs. 2-Exit-gepoolt (gleiche Datei)
2. **Fairness-Daten:** zwei gleichzeitige Downloads teilen sich den Pool
3. **Stolpersteine:** was die Doku/Software dadurch lernen muss

Der Pilot ist **link-agnostisch**: Ob die Häuser per Glasfaser,
SFP+-Direktkabel, Switch im Keller, 60-GHz-Richtfunk oder normalem
Ethernet verbunden sind, entscheidet die Nachbarschaft — die Software
misst, was da ist (siehe [[Architektur]], Schicht 0). Alle
Angaben zu Link-Typen in dieser Datei sind Beispiele.

## Voraussetzungen

### Teilnehmer

- 2 Haushalte (ideal 3, der dritte als **Leech** — Empfang ohne
  Exit-Angebot, um genau diesen Modus mitzutesten)
- Ein persönlicher Termin; technisches Onboarding gesicht zu gesicht
  (Nachbarn sind der Trust-Anchor, siehe [[Architektur]])

### Hardware (je Haushalt, Beispiele)

- Ein Mini-PC oder Einplatinenrechner (≥ 2 GB RAM, Debian o. ä.) als
  Node-Host — je eine NIC Richtung Heim-Router und Richtung
  Nachbarschafts-Link (übergangsweise reicht auch eine NIC am
  Mesh-Switch)
- Verbindungsweg — **eins davon genügt:**
  - Glasfaser/DAC-Direktkabel zwischen den SFP/SFP+-Ports
  - gemeinsamer Switch im Keller/Treppenhaus
  - 60-GHz-Richtfunk-Pair (Sichtlinie nötig, siehe [[Recht]] §3)
  - vorhandenes Ethernet/WLAN (langsamer, aber null Aufwand)

### Software

- [yggdrasil](https://yggdrasil-network.github.io/) (Binary oder
  Debian-Paket)
- SpiderNet-Binaries bauen und rüberkopieren:

```sh
cd server
cargo build --release --workspace
# → target/release/sn-node, sn-exit, sn-fetch, sn-fair
```

### Vor dem Termin (Checkliste)

- [ ] ISP-AGB der Teilnehmenden geprüft (Weitergabe an Dritte,
  siehe [[Recht]] §2)
- [ ] Nachbarschafts-Abkommen geklärt: wer spendiert wie viel
  (Quota), Leechen ausdrücklich erlaubt, alles jederzeit widerruflich
- [ ] Hardware vorhanden und bootfähig
- [ ] Binaries gebaut und auf beide Nodes kopiert
- [ ] Testdatei gewählt: Range-fähiger Server, ~750 MB, bekannte
  SHA-256 (im PoC hat sich eine Debian-Netinst-ISO bewährt; die
  offizielle `SHA256SUMS` der jeweiligen Version nutzen)

## Termintag: Schritt-für-Schritt

Die technischen Details (Config, Schlüssel, Admin-Socket) stehen in
[[Overlay]] §Deployment — hier die Reihenfolge mit Checkpunkten.

1. **Physik verbinden.** Link herstellen, L2-Test: `ping` über die
   lokale Verbindung. Optional `iperf3` über den Link → das ist
   **Messpunkt M0** (Kapazität des Nachbarschafts-Links).
2. **Yggdrasil einrichten (beide Häuser).**

   ```sh
   sn-node genconf --listen "tcp://[::]:1337" \
     --peer "tcp://[<Mesh-IP des Nachbarn>]:1337" \
     --allow-key <Public-Key des Nachbarn> \
     --private-key-path /etc/spidernet/node.key \
     --out /etc/yggdrasil.conf
   yggdrasil -useconffile /etc/yggdrasil.conf
   sn-node status   # eigene Overlay-Adresse + Peering-Status
   ```

   - Im selben LAN reicht Multicast-Auto-Peering (ohne `--peer`).
   - Config enthält den **privaten** Key (`chmod 600` macht
     `genconf` selbst) — niemals teilen oder committen.
   - Die eigenen Adressen/Keys werden **persönlich** ausgetauscht,
     nicht per Chat/Repo.
3. **Overlay-Test.** `sn-node status` zeigt auf *beiden* Seiten den
   Nachbarn als Peer; `ping <Overlay-Adresse des Nachbarn>` geht.
4. **Exit starten (Haus B spendiert):**

   ```sh
   sn-exit --bind "[<Overlay-Adresse B>]:8080" \
     --allow <Overlay-Adresse A> \
     --quota-mib 1024
   ```

   - `--allow` nie offen lassen (ohne Liste ist nur Loopback
     erlaubt, das ist der Dev-Mode).
   - Test von Haus A: ein HTTP-Request gegen einen Origin über
     `http://[<Overlay-Adresse B>]:8080` als Proxy (z. B. mit
     `sn-fetch --probe-only` oder `curl -x`).
5. **Erster gepoolter Download (Haus A):**

   ```sh
   sn-fetch <url> -o testdatei.iso \
     -e "1=1,2=1@http://[<Overlay-Adresse B>]:8080" \
     --sha256 <erwarteter-hash>
   ```

6. **Kill-Switch-Test.** `sn-exit` auf B stoppen → laufender und
   nächster Download fällt auf den eigenen Uplink zurück (Retry mit
   Exit-Reassignment). Danach B wieder starten. Beides dokumentieren:
   Dauer der Störung, verlorene Segmente.

Wichtig zur Beruhigung: SpiderNet v0 installiert **keine
Routing-Änderung** im Heimnetz. Geräte nutzen wie gewohnt den eigenen
Router; nur `sn-fetch`-Downloads gehen (explizit) in den Pool. Alles
stoppen (`sn-exit`, `yggdrasil`) = das Haus ist wie vorher.

## Messprotokoll

Regeln für belastbare Werte:

- Gleiche Testdatei, gleiche Uhrzeit ± 1 h (ISP-Congestion schwankt
  tageszeitabhängig)
- Pro Szenario **3 Läufe**, Median notieren; zwischen Läufen Pause
- Nichts anderes auf den Nodes laufen lassen; Messwerte immer mit
  Datum/Uhrzeit und den beidseitigen Uplink-Tarifen notieren

### Szenarien

| # | Szenario | Wo | Was gemessen wird |
|---|---|---|---|
| M0 | Nachbarschafts-Link-Kapazität | A↔B | `iperf3` über den lokalen Link |
| M1 | Uplink-Baseline | je Haus | nur eigener Uplink (`-e "1=1"`), ISP-Kapazität |
| M2 | Gepoolt, 1 Download | A | 2 Exits (`-e "1=1,2=1@http://[B]:8080"`), Gesamt-Durchsatz vs. M1 |
| M3 | Gepoolt, Konkurrenz | A + B | zwei `sn-fetch`-Läufe gleichzeitig, beide über beide Exits — wie teilen sich die Aufstellung |
| M4 | Fairness-Soll | (Simulation) | `sn-fair` mit den M1/M2-Werten → Soll-Aufteilung zum Vergleich mit M3 |

### Tabelle M2/M3 (Vorlage, pro Lauf eine Zeile)

| Lauf | Szenario | Datum/Zeit | Durchsatz Mbit/s | Dauer | Segmente | Retries | Bemerkung |
|---|---|---|---|---|---|---|---|
| 1 | M2 | | | | | | |
| 2 | M2 | | | | | | |
| 3 | M2 | | | | | | |

Zusätzlich notieren: Quota-Rest auf B, CPU/RAM der Node-Hosts,
(evtl.) Firmware/Treiber des Link-Wegs.

### Fairness-Soll mit `sn-fair` (M4)

Die gemessenen Uplink-Kapazitäten (M1) als Pool-Kapazität und die
gleichzeitigen Nachfragen (M3) einsetzen — das CLI arbeitet in
Mbit/s:

```sh
sn-fair --capacity <M1-A + M1-B, Mbit/s> \
  --participant 1=contributing \
  --participant 2=contributing \
  --participant 3=leeching \
  --demand 1=<Nachfrage A> \
  --demand 2=<Nachfrage B> \
  --demand 3=<Nachfrage Leech>
```

Erwartung (Regelwerk in [[Architektur]] §Fairness-Scheduler):
Contributors bekommen die Kapazität nach max-min Fairness (gewichtet),
Leeches profitieren von Restkapazität — bei Sättigung zuerst gedrosselt,
nie ausgeschlossen. Weicht M3 (Ist) deutlich von M4 (Soll) ab, landet
das als Erkenntnis in der Ergebnisse-Sektion unten (die echte
Runtime-Integration in `sn-fetch` ist Phase 2, siehe [[Roadmap]]).

## Sicherheit & Recht (Checkliste während des Piloten)

- [ ] `sn-exit` nur mit `--allow` (Allowlist) und Quota laufen lassen
- [ ] SSRF-Schutz nie deaktivieren (verhindert Nachbarn-Zugriff auf
  Heim-LAN/Admin-Sockets des Exit-Haushalts)
- [ ] Keine Port-Forwards ins Internet, außer ein Peering braucht es
  zwingend (siehe [[Overlay]] §Grenzen)
- [ ] Logs minimal und lokal (keine Inhaltslogs, siehe [[Recht]] §4)
- [ ] Haftungsausschluss ist den Teilnehmenden bekannt gezeigt
- [ ] Sperrbarkeit praktisch geübt: Exit stoppen = Kill-Switch,
  Teilnehmersperre = `--allow`-Liste ändern

## Ergebnisse

*(wird nach dem Piloten gefüllt — Messwerte, Erkenntnisse und
Stolpersteine gehören hier ins Repo, nicht in den Chat)*

### Messwerte

*(Tabelle ergänzen — M0 bis M4, Mediane der 3 Läufe)*

### Erkenntnisse

*(kurz, ehrlich, mit Quellen/Logs wo sinnvoll)*

### Stolpersteine & Anpassungen

*(was in [[Architektur]], [[Overlay]], [[Roadmap]] oder der Software
geändert werden muss, damit der nächste Pilot weniger stolpert)*
