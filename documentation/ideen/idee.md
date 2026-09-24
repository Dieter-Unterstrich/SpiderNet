# Idee — und ein ehrlicher Realitätscheck

## Ursprungsidee

> Man spannt Glasfaser von Haus zu Haus und baut ein lokales Netz auf. Wenn
> man über das Internet etwas nachfragt, läuft die Anfrage nicht (nur) über
> den eigenen Anschluss, sondern die Last wird aufs gesamte Netz verteilt.
> Jede:r nutzt die Gesamtkapazität der Nachbarschaft, ohne dass es mehr
> kostet. Schwache Haushalte werden gestärkt. Nebenbei wird Traffic verteilt
> und ist für Außenstehende schwerer zuzuordnen (Privacy).

Wichtiger Punkt (bewusst gehalten): Die Glasfaserverbindungen zwischen den
Häusern laufen **neben** den normalen ISP-Anschlüssen — das Nachbarschaftsnetz
ist ein *zweites*, schnelles (Ziel: 10 Gbit/s) lokales Netz, nicht ein Ersatz
für die Internetanschlüsse.

Diese Idee ist gut — und sie ist nicht abstrakt: NYC Mesh läuft genau so
("members share their internet"), Freifunk und guifi.net zeigen es in
großem Maßstab. Was hier neu gedacht ist, ist der **konsequente Exit-Jede**
statt Exit-Nirgends: Jeder Haushalt bleibt bei seinem ISP, aber Anfragen
verlassen das Viertel an *allen* Punkten gleichzeitig.

## Was technisch funktioniert

1. **Segmentierte Downloads über mehrere Exits.** Große Dateien werden in
   Byte-Bereiche gesplittet (HTTP Range Requests) und parallel über die
   Internetanschlüsse mehrerer Haushalte geladen. Ein DSL-50000-Haushalt
   kann damit realistisch 5× herunterladen, wenn 4 Nachbarn mitspielen.
   Das geht, weil die Haus-zu-Haus-Verbindungen selbst schnell sein
   sollen (10 Gbit/s) — die Faser im Viertel ist nie der Engpass, die
   Uplinks sind es.
2. **Fair-Share-Scheduling.** Wer gerade nichts lädt, gibt seinen Anteil
   automatisch. Ein einzelner Stream profitiert von fremder Kapazität,
   mehrere Lade gleichzeitig bekommen faire Anteile.
3. **Nachbarschafts-Cache.** Beliebte Inhalte (Updates, Games, Videos,
   Software-Pakete) liegen im Viertel. Das entlastet die Uplinks stärker
   als jedes Pooling — siehe [lancache](https://lancache.net/).
4. **Privacy durch Verteilung.** Der ISP sieht pro Haushalt nur einen
   Bruchteil des ursprünglichen Traffics; Zuordnung "IP → Person" wird
   deutlich schwächer. Backtracking von außen muss die Frage lösen:
   "Welcher der 12 Anschlüsse im Viertel hat das eigentlich gezogen?"
5. **Resilienz.** Fällt ein Anschluss aus, laufen die anderen weiter — auch
   lokaler Traffic funktioniert ohne Internet komplett (Ethos §7).

## Was nicht so einfach ist

1. **Ein einzelner TCP-Stream kennt kein Pooling.** Ohne Segmentierung oder
   Multipath bleibt ein einzelner Download auf der Geschwindigkeit *eines*
   Uplinks. Der Gewinn kommt aus unserer Download-Software, nicht aus dem
   Kabel — deshalb bauen wir `server/`.
2. **Alltagstraffic braucht eine Exit-Wahl.** Video-Calls, Banking etc.
   können nicht sinnvoll gesplittet werden. Für generischen Traffic braucht
   es ein VPN-Overlay mit Load-Balancing (mehrerer Exits gleichzeitig),
   damit z. B. Web-Browsing über "die Nachbarschaft" läuft statt über die
   eigene Leitung.
3. **Physik ist der eigentliche Engpass.** Glasfaser spannen heißt: Genehmigungen,
   Vermieter, Grundstücke, Arbeit. Realistisch beginnt man mit dem, was da
   ist (Kabel im Mehrfamilienhaus, Garten, kurze Richtfunkstrecken), und
   baut Glasfaser dort, wo ohnehin gegraben wird (siehe [[Recht]]).
4. **Volumen-/Drosseltarife.** Wer ein Volumenlimit hat, zahlt mit Volumen,
   wenn Nachbarn über seinen Exit laden. Lösung: Exits haben harte Kontingente
   und der Scheduler respektiert sie ("Exit-Karenz"), plus Karma. Für diese
   (seltene) Fälle gibt es zusätzlich den **Leech-Modus**: Sharing komplett
   aus, nur empfangen — unter Nachbarn legitim (siehe [[Ethos]] §2).
5. **Privacy ist ein Nebenprodukt, keine Anonymität.** Freenet sagt es
   selbst (FAQ): keine Anonymität wie Tor/I2P. Metadaten bleiben. Wer echte
   Anonymität will, legt Tor/I2P über das Netz. Wir verkaufen deshalb
   Privacy-by-Design, nicht Anonymität.
6. **Recht.** Abmahnungen landen zuerst beim Anschlussinhaber, egal wer
   geladen hat; bewusstes Ermöglichen von Rechtsbrüchen kann eigene Haftung
   erzeugen. Details und Gegenmaßnahmen: [[Recht]].

## Der technische Kern (warum es Software braucht)

Das Netz ohne Software ist "nur" ein LAN. Die Mehrwerte entstehen durch
einen **SpiderNet-Node** pro Haushalt, der folgendes macht:

- Overlay-Routing über alle Haushalte (Kandidat: Yggdrasil)
- Exit-Pool verwalten: Wer darf über wen raus, mit welchem Kontingent
- Segmentierten Multi-Exit-Downloader (API + CLI + Browser-Integration)
- Fairness-Scheduler über alle Haushalte
- Transparenten Nachbarschafts-Cache
- (Später) Freenet als Inhalts- und App-Ebene im Viertel

Siehe [[Architektur]] und [[Server-README]].
