# Rechtslage (Deutschland)

> ⚠️ Keine Rechtsberatung. Stand: September 2026, aus öffentlich zugänglichen
> Quellen zusammengestellt. Für den Ernstfall: Anwält:in für Urheberrecht/
> Telekommunikationsrecht fragen — im Kollektiv, nicht alleine.

## 1. Urheberrecht / Filesharing — das größte Risiko

### Wie die Rechtsprechung heute funktioniert

- Wer im Netz einer IP-Adresse etwas rechtswidrig bereitstellt, führt zur
  **Täterschaftsvermutung** gegen den Anschlussinhaber (BGH, 12.05.2010,
  I ZR 121/08). Diese Vermutung kann erschüttert werden, indem man darlegt,
  dass **andere selbstständigen Zugang** zum Anschluss hatten ("sekundäre
  Darlegungslast") — man muss nicht den Täter benennen (LG Frankenthal,
  06.08.2019, 6 O 398/17; AG Bochum, 16.04.2014, 67 C 57/14 — WG-Fall).
- **Störerhaftung** des Zugangsvermittlers für Unterlassung/Schadensersatz
  wurde 2017 weitgehend abgeschafft (§ 8 Abs. 1 S. 2 TMG a.F., heute in
  DDG/DSA weitergeführt; BGH "Dead Island", I ZR 64/17). Es bleibt ein
  **Sperranspruch** (§ 7 Abs. 4 TMG a.F.): Rechteinhaber können verlangen,
  dass der Anschlussinhaber konkrete rechtsverletzende Inhalte sperrt,
  wenn es ihm zumutbar ist (Verbraucherzentrale: das kann Registrierung,
  Passwortzwang bis hin zur vollständigen Zugangssperre bedeuten).
- **Abmahnungen landen trotzdem zuerst beim Anschlussinhaber** — Rechteinhaber
  wissen nicht, wer wirklich geladen hat; die Ermittlung läuft über die
  zentrale IP-Adresse (Verbraucherzentrale Berlin).
- Wer **Kenntnis** von einer Rechtsverletzung über seinen Anschluss hat und
  nicht einschreitet, kann haften (Störerhaftung bei konkretem Hinweis).
- Wer **bewusst und gezielt** Infrastruktur bereitstellt, um Rechtsbrüche
  zu ermöglichen, kann als Teilnehmer/Beihelfer behandelt werden
  (Analogie: BGH zu Tor-Exit-Betreibern mit Vorbelastung).

### Was das für das AKI-Netz bedeutet

1. **Exit-Haushalt = Verantwortung bleibt beim Exit-Haushalt.** Wenn
   fremder Traffic über meinen Anschluss ins Internet geht, gilt:
   - Ich muss darlegen können, dass andere Zugang hatten → das Netz
     dokumentiert (lokal, nicht zentral!) die Teilnehmerschaft.
   - Bei Kenntnis einer konkreten Verletzung muss ich handeln können →
     deshalb braucht der AKI-Node technische Wege, einzelne Nutzer/Segmente
     zu sperren, ohne das ganze Netz zu killen.
2. **Kein "Das ist ja verteilt, also legal."** Wer lädt, bleibt Täter —
   unabhängig davon, über welchen Exit. Verteilung ändert die Ermittlungs-
   lage (schwerere Zuordnung), nicht die Haftung des wirklich Tätigen.
3. **Piraterie ist kein Feature.** Ethos §5: Das Netz wird als
   Infrastrukturprojekt betrieben. Downloads großteils über Overlays
   (Freenet/I2P/Tor) zu leiten ist ein technischer Schutz für *alle*
   — aber die Werbung dazu gehört nicht auf die Projekt-Homepage.
4. **Sekundäre Darlegungslast brauchen wir als Feature:** Der Node führt
   (lokal, minimal, DSGVO-konform) eine Teilnehmerliste, die im Ernstfall
   belegt: "Auf meinem Anschluss hatten N Personen unabhängigen Zugang."

## 2. ISP-AGB / Vertragslage

- Viele deutsche Provider verbieten in AGB die **Weitergabe an Dritte**.
  Konsequenz: pro ISP prüfen, ggf. Tarif wählen, der das erlaubt, oder
  Formulierung als "privates WLAN-Netz im eigenen Haushaltsverbund"
  (Nachbarn ≠ Gäste ist eine Grauzone). Offenlegen, nicht verstecken.
- Netzneutralität: Pooling ist kein Problem, solange wir nicht nach
  Inhalten filtern oder drosseln.

## 3. Physik: Kabel verlegen

- **Mehr-/Eigentumsfamilienhäuser:** einfachste Route — Innenausbau mit
  Zustimmung Eigentümer:in.
- **Kabel über fremde Grundstücke:** braucht Zustimmung (Dienstbarkeit/
  Gestattungsvertrag). § 918 BGB (Notwegrecht) greift nicht automatisch
  für Leitungen über fremdes Grund; sauberer Weg ist Zustimmung durch
  Vertrag.
- **Unterirdische Verlegung in öffentlichen Flächen/Straßen:** Bauanfrage
  bei der Gemeinde, ggf. Tiefbauamt — das ist der reguläre Weg für
  Nachbarschafts-Fiber-Projekte (guifi.net und Community-Fiber-Initiativen
  in Deutschland machen es vor).
- **Richtfunk als Alternative:** keine Grabungsrechte nötig, nur
  Sichtlinie und ggf. Denkmalschutz am Gebäude.

## 4. Datenschutz

- Nodes loggen minimal (nur Betriebsmetriken, keine Inhalts-/Nutzungs-
  profiles).Logs bleiben lokal, nichts geht zentral ab.
- Teilnehmerdaten (Namen, Adressen) bleiben **außerhalb des Repos** und
  außerhalb der Nodes, wo es nicht nötig ist.

## 5. Zusammengefasste Regeln für die Software

Damit Agents und Developer das im Code umsetzen:

1. Exit ist opt-in, dokumentiert und jederzeit widerrufbar.
2. Node kann einzelne Teilnehmer/Segmente sperren (Sperrpflicht-Umsetzung).
3. Logs: minimal, lokal, keine Inhaltslogs.
4. Keine Funktion, deren alleiniger Zweck das Verstecken von Rechtsbrüchen ist.
5. Heavy-Traffic-Default: Freenet/Overlay bevorzugen, wo sinnvoll.
6. Datenschutz by default; keine zentrale Datensammlung.
