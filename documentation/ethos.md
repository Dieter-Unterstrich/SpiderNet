# Ethos

Was dieses Netz ist und wie es sich verhält. Diese Werte gelten für Technik
genauso wie für den Umgang miteinander.

## 1. Gemeingut statt Markt

Die Infrastruktur (Glasfaser, Router, Uplinks) gehört den Teilnehmenden
gemeinsam und wird geteilt, nicht verkauft. Bandbreite ist ein **Commons**:
Niemand erwirbt mehr Rechte, indem er mehr bezahlt oder mehr Hardware hat.
Anders als z. B. [Althea](https://www.althea.net/) (Mikrozahlungen pro
weitergeleitetem Paket) wollen wir bewusst **keine Bezahlung** im Kern —
der Fairness-Scheduler (siehe Architektur) ist unsere Währung.

## 2. Fairness als Algorithmus, nicht als Gnade

Jeder Haushalt bekommt standardmäßig einen gleichberechtigten, fairen Anteil
(max-min Fairness). Stärkere Anschlüsse teilen freiwillig — genau das ist der
"anarcho-kommunistische" Kern: voneinander abhängig, freiwillig, ohne Zwang
und ohne Markt.

**Leechen ist erlaubt.** Wer nichts beiträgt (kein Exit, kein Cache,
keine Faser), darf trotzdem empfangen — Nachbarschaftskultur ist Nachsicht,
nicht Gegenleistungszwang. Der Scheduler behandelt "Leeched" als niedrigste
Priorität bei Knappheit, aber nie als Strafe. Es gibt keine Bezahlung und
keine Pflicht zum Mitmachen; Andenken an Fairness ist Sache des Viertels,
nicht der Software.

## 3. Freiwilligkeit und Do-ocracy

Teilnahme ist freiwillig, Austritt jederzeit möglich. Entscheidungen trifft,
wer Arbeit leistet (Do-ocracy), abgestimmt im offenen Gespräch. Niemand kann
gezwungen werden, als Exit für andere zu dienen — Exit-Bereitschaft ist eine
**explizite, jederzeit widerrufliche Zustimmung pro Haushalt**.

## 4. Privacy als Grundrecht, Anonymität als Nirgendwo-Garantie

- Der eigene Traffic läuft so weit wie möglich nicht über die eigene Leitung:
  Verteilung über viele Hausanschlüsse macht Zuordnung für ISPs und Dritte
  deutlich schwerer. Das ist ein echter Sicherheitsgewinn für alle.
- Aber wir versprechen **keine Anonymität**: Freenet selbst sagt klar, dass
  es keine Anonymität wie Tor/I2P bietet. Metadaten, TLS-Verkehr und Exit-
  Knoten bleiben sichtbar. Wer echte Anonymität braucht, legt zusätzlich
  Tor/I2P über das Netz.
- Nodes loggen minimal. Keine Nutzungsprofile, keine Werbepreise.

## 5. Legaler Betrieb als Überlebensbedingung

Ein Netz, dessen öffentliches Gesicht "Piraterie" ist, wird abgemahnt,
zerlegt und diskreditiert — und zieht Risiko auf die schwächsten Teilnehmer
(siehe [recht.md](ideen/recht.md)). Deshalb:

- Das Netz wird als Infrastrukturprojekt betrieben (Zugang, Resilienz,
  Gemeingut), nicht als Tauschbörsen-Produkt.
- Jede:r bleibt für das eigene Tun verantwortlich; das Netz dokumentiert,
  dass viele Menschen Zugang haben (schützt den Anschlussinhaber).
- Technisch kühles Denken ja, "Es ist legal, weil verteilt" — nein.

## 6. Transparenz und offene Technik

Alles ist offen: Code (AGPL-3.0), Doku, Pläne, auch die Fehler. Hardware
besteht möglichst aus Standardkomponenten (Mini-PC, SFP-Medienwandler,
Standard-Glasfaser), die alle reparieren und beschaffen können.

## 7. Resilienz und Selbsthilfe

Das Netz soll funktionieren, wenn das Internet gerade nicht funktioniert:
lokale Dienste, lokale Caches, lokale Kommunikation (siehe NYC Mesh).
Jede:r im Haus kann Teilnahme, Installation und Betrieb lernen — Wissen wird
geteilt wie Bandbreite.

## Die "Pico Peering" Hausregel

Basierend auf dem [Pico Peering Agreement](https://picopeer.net/) halten wir
fest: **Wer mitmacht, teilt. Wer teilt, wird geteilt.** Konkret:

1. Ich verbinde mein Haus mit dem Netz und lasse Traffic durch.
2. Ich benutze das Netz fair (Fair-Share).
3. Ich stelle nichts Unlawfuls bereit und verantwortet mein eigenes Handeln.
4. Konflikte werden im Viertel geklärt, nicht beim Anwalt.
