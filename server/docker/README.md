# SpiderNet-Node mit Docker

Dieser Ordner enthält alles, um den SpiderNet-Node eines Haushalts als
Container zu betreiben: Multi-Stage-Dockerfile, ein Compose-Beispiel für
einen Host und eine kommentierte Beispiel-Konfiguration.

## Haftungsausschluss

> **KEINE GARANTIE.** Diese Software wird "as is" ohne jegliche Garantie
> bereitgestellt (AGPL §§ 15/16). Die Betreiber und Contributor:innen dieses
> Projekts übernehmen **keinerlei Verantwortung oder Haftung** dafür, wozu
> Nutzer:innen die Software verwenden oder welche Inhalte über sie
> übertragen werden. Die Verantwortung für gesendete und empfangene Inhalte
> liegt vollständig bei den Nutzer:innen.

## Quickstart

```sh
# 1. Konfiguration anlegen und anpassen
cp node.toml.example node.toml
$EDITOR node.toml

# 2. Image bauen und starten (Build-Kontext ist server/, daher "context: ..")
docker compose up --build -d

# 3. Beobachten
docker compose logs -f

# 4. Overlay-Status ansehen (Admin-Socket lauscht auf localhost:9001)
docker compose exec node sn-node status

# 5. Einen großen Download über die gepoolte Bandbreite laden
docker compose exec node sn-fetch "https://…/große-datei.iso" \
    -o /tmp/out.iso -e "1=2,2=1@http://[<ygg-adresse-des-nachbarn>]:8080"
```

Image direkt bauen (ohne Compose), Build-Kontext `server/`:

```sh
docker build -t spidernet-node -f docker/Dockerfile ..
```

## Was die Container-Privilegien bedeuten

| Einstellung | Warum |
|---|---|
| `network_mode: host` | Yggdrasil muss auf dem realen LAN peeren und auf der Overlay-Seite erreichbar sein. Mit Bridge-NAT würden Peer-Verbindungen und der Exit-Port nicht stimmen. |
| `cap_add: [NET_ADMIN]` | Yggdrasil legt das TUN-Interface (`ifname`) an und konfiguriert es — das braucht `CAP_NET_ADMIN`. |
| `/dev/net/tun` | Das Kernel-TUN-Gerät, das Yggdrasil öffnet. |
| `init: true` | Sauberer PID 1: Reaping + SIGTERM erreicht den Daemon, damit `docker compose stop` als sauberer Kill-Switch funktioniert. |

Der Container läuft als `root` — für TUN im Container üblich. `NET_ADMIN`
ist bewusst nur als Capability gesetzt, ohne `privileged`.

## Zwei Haushalte verbinden

Beide Hosts laufen mit demselben Image; die Rolle (Exit oder Leech) steht
nur in der jeweiligen `node.toml`.

- **Die Mesh-Verbindung liegt außerhalb des Containers:** Glasfaser,
  Switch oder WiFi zwischen den Haushalten. Der Container peeret über das
  Host-Netzwerk.
- **Auto-Peering:** Im selben LAN (Multicast aktiv) finden sich
  Yggdrasil-Nodes selbst — `peers = []` reicht.
- **Explizit:** Falls Multicast nicht geht (z. B. VLANs), trägt man in
  `node.toml` bei `peers` URIs wie `tcp://<nachbar-lan-ip>:1337` ein.
- **ACL:** Die 64-Hex-Public-Keys der erlaubten Nachbarn gehören in
  `allow_keys`, sonst kann jeder Node im Overlay peeren.

Die `node.toml` jedes Haushalts enthält persönliche Daten der Nachbarschaft
(Public Keys, Adressen) und später den Node-Private-Key im State-Verzeichnis:
**niemals committen**, außerhalb des Repos aufbewahren.

## Leech-Modus (Empfang ohne Exit)

Die Beispiel-Konfiguration startet sicher im **Leech-Modus** — ein
Haushalt, der nur empfangen will, muss nichts ändern:

```toml
[exit]
enabled = false
```

Das ist ausdrücklich legitim — Leeches werden bei Sättigung zuerst
gedrosselt, aber nie ausgeschlossen. Es gibt kein separates Leech-Image;
das Compose-Beispiel funktioniert unverändert.

Wer seinen Uplink als Exit anbieten will (opt-in), setzt:

```toml
[exit]
enabled = true
allow = ["<overlay-adresse-des-nachbarn>"]
```

`enabled = true` mit leerer `allow`-Liste ist ungültig — der Daemon
verweigert dann den Start (Ausnahme: Dev-Modus mit Loopback-bind).

## Kill-Switch

```sh
docker compose stop
```

Stoppen des Exits bzw. des Daemons ist der Kill-Switch: Sobald der Daemon
nicht läuft, stellt der Haushalt keinen Uplink mehr bereit. Der eigene Node
fällt auf den eigenen Internetanschluss zurück. Wichtig zum Verständnis:
SpiderNet v0 leitet **normalen Verkehr nicht um** — kein Routing, kein
NAT-Trick. Nur Downloads, die `sn-fetch` explizit über das Netz verteilt,
nutzen die gepoolte Bandbreite; alles andere geht wie gewohnt über den
eigenen Anschluss.

## Zustand

`node.key` und die generierte `yggdrasil.conf` liegen im State-Verzeichnis
(`/var/lib/spidernet`, Named Volume `spidernet-state` im Compose-Beispiel).
Der Node behält damit seine Identität über Container-Neustarts. Volume
löschen = neue Node-Identität.

## Noch offen / Hinweise

- Der `daemon`-Subcommand ist implementiert und die Beispiel-Konfiguration
  wurde damit verifiziert (`sn-node daemon --check` → `config ok`).
- Images werden lokal gebaut, nicht in ein Registry gepusht (das Projekt
  kennt keine zentrale Infrastruktur).
