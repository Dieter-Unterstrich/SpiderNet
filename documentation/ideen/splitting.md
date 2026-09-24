# Splitting-Fähigkeit — was lässt sich über mehrere Exits verteilen?

Die zentrale Frage: Welche Traffic-Arten können die gepoolte Bandbreite der
Nachbarschaft nutzen, und was braucht die Quelle (Server) dafür?

## Die drei Kategorien

### 1. Range-basiert splittbar (echtes Splitting)

Standard-HTTP, kein Spezialformat: Der Server muss nur **HTTP Range Requests**
unterstützen ([RFC 9110](https://httpwg.org/specs/rfc9110.html) §14, früher
RFC 7233) — Anfrage mit `Range: bytes=0-999999`-Header, Antwort `206 Partial
Content`. Praktisch alle großen Datei-Server können das; man prüft es mit
einer Kopf-Anfrage: `Accept-Ranges: bytes` in der Antwort.

| Läuft gut | Begründung |
|---|---|
| ISO-Images, große Dateien (Linux-Distros, Archive) | klassischer Range-Fall, CDNs/S3/nginx/Apache unterstützen es |
| Steam / Konsolen-Updates / OS-Updates | laden selbst mehrschichtig parallel, Ranges sind Teil des Designs |
| Video-on-Demand (HLS/DASH-Streaming) | ohnehin in 2–10-s-Segmente zerlegt; Segmente sind einzelne HTTP-Objekte → Verteilung auf Exits gratis |
| Torrents | sind inhärent multi-connection; Verteilung über Exits ist einfach Flow-Verteilung |

**Wichtig bei HTTPS:** Range-Splitting funktioniert trotzdem — jeder Exit
baut seine *eigene* TLS-Verbindung zum Server auf und sieht nur
verschlüsselte Bytes. Der Server sieht dann N Verbindungen von N
verschiedenen IPs (siehe Vorbehalte unten).

### 2. Verbindungs-basiert verteilbar (Aggregation ohne Splitting)

Hier wird **kein einzelner Strom** gesplittet — sondern viele
selbstständige Verbindungen werden auf die Exits verteilt (Flow-Level-
Balancing). Jede einzelne Verbindung läuft vollständig über einen Exit.

| Läuft gut | Begründung |
|---|---|
| Normales Web-Browsing | der Browser baut ohnehin viele parallele Verbindungen zu vielen Hosts auf — die verteilen sich natürlich auf Exits |
| Paketmanager (apt, pip, cargo, npm) | hunderte kleine Dateien = hunderte Verbindungen |
| API-Traffic, RSS, DNS | viele kleine unabhängige Anfragen |

Der Pooling-Gewinn entsteht **nur bei paralleler Last** (viele Verbindungen
gleichzeitig). Ein einzelnes HTTP-Objekt ohne Range-Support bleibt auf
Uplink-Geschwindigkeit.

### 3. Nicht splittbar — sticky single Exit (bzw. Direct-Modus)

Einzelne, latenzempfindliche Ströme, bei denen ein Mid-Stream-Exit-Wechsel
die Verbindung kaputt machen würde (IP-Wechsel beendet die Session):

| Kategorie | Warum nicht splittbar |
|---|---|
| **Video-Calls (WebRTC)** | einzelner DTLS/SRTP-UDP-Strom, echtzeitkritisch, IP-gebunden |
| Online-Gaming | einzelner UDP-Flow, IP-Stickiness wichtig, Latenz >> Bandbreite |
| SSH / Remote-Desktop | interaktive TCP-Session, IP-Stickiness |
| Banking/Logins | Session an IP gekoppelt; aber: funktioniert technisch über jeden Exit, nur nicht *wechselnd mitten in der Session* |
| VPNs (WireGuard etc.) | UDP-Flow mit State |

**Policy-Vorschlag (Default):** realzeitkritischer Traffic geht in den
**Direct-Modus** — und zwar als echter **Pass-Through**: SpiderNet gibt
diese Verbindungen unverändert weiter, sie nehmen den normalen Weg durch
den eigenen Router und den eigenen Uplink, als wäre SpiderNet nicht da
(kein Tunnel, kein MITM). Optional "best single exit" (wenn die eigene
Leitung gestört/überlastet ist, kann man den Call bewusst über einen
Nachbarschafts-Exit pinnen — aber dann sticky für die Dauer der Session).

## Was die Quelle mitbringen muss (Zusammenfassung)

| Anforderung | Betrifft | Fehlt sie… |
|---|---|---|
| `Accept-Ranges: bytes` + `206 Partial Content` | Kategorie 1 | …fällt das Objekt auf Kategorie 2/3 zurück (Download über *einen* Exit komplett) |
| viele unabhängige Verbindungen möglich | Kategorie 2 | nothing — das ist der Normalfall |
| Session-Stabilität (IP-Stickiness) | Kategorie 3 | …darf der Exit nicht wechseln; Direct-Modus ist der sichere Default |

## Vorbehalte bei Range-Splitting (ehrliche Liste)

1. **Per-Connection-Limits:** manche Server/CDNs drosseln pro Verbindung
   (z. B. 5 MB/s) — dann braucht man genug Segmente, um den Pool auszureizen.
2. **Bot-Erkennung/IP-Diversity:** N Verbindungen von N verschiedenen IPs
   für dasselbe Objekt kann Rate-Limits oder Captchas triggern. Gegenmaßnahme:
   gleiche Exits pro Ziel-Host cachen (Affinität), Retry mit anderem Exit.
3. **Signierte URLs:** meist unkritisch (Ranges innerhalb der Gültigkeit
   erlaubt), aber Auth-Cookies müssen pro Exit-Verbindung mitgeliefert
   werden — bei geteilten Logins vorsichtig.
4. **`Accept-Ranges: none`:** Server kann Ranges explizit verweigern →
   Fallback (ein Exit), nur bei großem Objekt schade.
5. **MPTCP ist kein Plan A:** Multipath TCP könnte einen *einzigen* Stream
   über mehrere Wege schicken, aber beide Endpunkte müssen es unterstützen —
   im offenen Internet praktisch nicht gegeben. Deshalb: Segmentierung auf
   Anwendungsebene, nicht Paketebene.

## Konsequenz für die Software (`sn-fetch`)

- Range-Fähigkeit pro URL/Host **automatisch erkennen** (Kopf-Anfrage auf
  `Accept-Ranges`) und pro Host cachen.
- Klassifizierung: Kategorie 1/2/3, je nach Art (auto: bekanntes Schema,
  Konfig: Nutzer-Overrides, z. B. "WebRTC immer Direct").
- Fallback-Kette: Ranges → Flow-Verteilung → single Exit (Direct-Modus).
- Exit-Affinität pro Ziel-Host, um Bot-Erkennung zu vermeiden.

Siehe auch: [[Architektur]] (Schicht 2), [[Idee]] (Realitätscheck).
