<p align="center">
  <img src="images/EH3D_PHOENIX_3DP_BANNER.svg" alt="EH3D-PHOENIX-3DP banner" width="100%">
</p>

# EH3D-PHOENIX-3DP
<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | 🇮🇹 <b>Italiano</b> | <a href="README_deu.md">🇩🇪 Deutsch</a> | <a href="README_zho.md">🇨🇳 中文</a> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### Prepara e invia stampe 3D: modelli, profili di stampante, G-code sicuro e il collegamento con le tue stampanti e con il server EH3D

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.5-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-537-4CAF50.svg" alt="Tests">
</p>

---

**Controllo di onestà - ciò che funziona davvero oggi:** un workspace Rust con otto librerie e un programma a riga di comando (`phoenix-3dp`) con 537 test che girano su un computer. Legge e controlla modelli STL, OBJ e 3MF, contiene un catalogo di 120 profili di stampante costruiti da dati pubblici (nessuno provato su una macchina), scrive G-code per i dialetti Marlin, Klipper, RepRapFirmware e Bambu con limiti di temperatura che rifiutano un valore invece di tagliarlo, e pilota stampanti Moonraker e OctoPrint e il server EH3D, ma solo contro simulatori (in memoria e su un socket di questo computer). Taglia un modello chiuso in strati piani con perimetri, riempimento rettilineo e pelli piene (un piccolo slicer con riempimento a griglia e giroide, ponti, supporti semplici, giunzioni, larghezza variabile, sollevamento negli spostamenti, skirt, brim o raft, modalità vaso, strati corti rallentati, retrazione che segue lo spostamento e una stima del tempo con accelerazione; ancora niente per più materiali); la finestra non è stata compilata né vista su uno schermo; nulla è stato eseguito su una stampante reale.

---

> **Stato di sicurezza:** questo programma non controlla da solo una macchina e non è un sistema di sicurezza. Ciò che produce deve essere controllato da una persona prima di eseguire qualsiasi cosa su una stampante; nulla di tutto ciò è stato eseguito su hardware reale.

> **Nomi di terzi:** i nomi di produttori, modelli e protocolli di stampanti (Bambu Lab, Prusa, Creality, Klipper, Moonraker, OctoPrint, Marlin e altri) appartengono ai rispettivi proprietari e sono usati solo per dire di quale macchina o protocollo si parla; questo programma non è realizzato, approvato né sponsorizzato da loro. È scritto da documentazione pubblica e fatti, non dal codice, dai testi, dalle icone o dai file di profilo di un altro slicer; nome, grafica e testi sono propri.


## 📋 In breve

| | |
|---|---|
| Versione | `0.0.5` |
| Maturità | Impalcatura |
| Test | 537 |
| Stack tecnologico | Rust |
| Su hardware reale | Non provato: nulla di tutto ciò è stato eseguito su una stampante, nemmeno su una Bambu Lab A1 o una P1S. |

**Vai a:** [Panoramica](#-panoramica) · [Architettura](#-architettura) · [Avvio rapido](#-avvio-rapido) · [Test](#-test) · [Struttura](#-struttura-del-repository) · [Correlati](#-progetti-correlati)

## 🚦 Stato del progetto

Ogni riga è una parte di questo repository e il semaforo dice a che punto è oggi. È scritta in base a ciò che il repository può mostrare (test, compilazioni, esecuzioni), non a ciò che è previsto.

🟢 Fatto e verificato · 🟡 Funziona, con limiti noti · 🟠 Fatto a metà o non ancora verificato · 🔴 Non fatto o non funzionante

| Area | Stato | Cosa significa qui |
|---|---|---|
| Nucleo | 🟡 | unità in nanometri, la mesh e i suoi controlli, i limiti dei lettori, un piccolo lettore JSON e un segreto che non si stampa mai; provato su computer con input normali e danneggiati. |
| Modelli e file | 🟡 | STL (testo e binario), OBJ e 3MF di base vengono letti con limiti di dimensione e rifiuti che dicono cosa hanno trovato; si scrivono il pacchetto 3MF proprio, STL binario, OBJ e il file di G-code; dei progetti 3MF di altri programmi si leggono oggetti, piatti e alcune impostazioni (sola lettura); verificato con test di fuzzing, non con una raccolta di file reali di altri programmi. |
| Profili di stampante | 🟠 | 120 modelli da dati pubblici (volume, ugelli, dialetto, modi di collegamento, limiti del produttore per i primi 46, comprese Bambu Lab A1 e P1S, e limiti prudenti del programma per gli altri, che lo dicono; dodici filamenti (sei generici e sei con fibre o una miscela più tenace) verificati contro la stampante e l'ugello, sette resine e modifiche di un profilo che dicono cosa è cambiato); tutti dicono `verified = false`, perché nessuno è stato provato su una macchina. |
| G-code | 🟡 | scrittori per i quattro dialetti con un inizio e una fine sicuri; una temperatura oltre il limite viene rifiutata e non si scrive nulla; un controllore trova le righe di un programma che superano un limite; mai eseguito su una stampante. |
| Driver di stampante | 🟠 | Moonraker e OctoPrint su un trasporto, provati solo contro simulatori dei loro protocolli; Bambu LAN è un progetto marcato sperimentale che non si collega. |
| Collegamento al server | 🟠 | le impostazioni del collegamento e un client del plugin delle stampanti del server sopra il client comune della suite (accesso, rinnovo, nuovi tentativi, errori in otto lingue), provato contro un server in memoria; mai contro il server reale. |
| Slicer | 🟡 | strati piani di una mesh chiusa, perimetri, riempimento rettilineo, a griglia o giroide, pelli piene, ponti, supporti semplici, giunzioni, larghezza variabile, G-code per dialetto, un'anteprima per strati come dati e una stima del tempo con accelerazione; verificato con proprietà su forme a seme fisso (la plastica depositata è il volume del pezzo, nulla esce dal piatto, nessuna temperatura supera un limite); nessuna stampa ne è uscita su una stampante. |
| Riga di comando | 🟡 | `phoenix-3dp` (`printers`, `info`, `check-profile`, `convert`, `filaments`, `resins`, `slice`, `project`, `version`) esegue le funzioni delle librerie e stampa il risultato; ogni errore torna come messaggio e codice di uscita diverso da zero. |
| Test | 🟡 | 537 test passano (`cargo test --workspace`), con casi normali, casi limite, input danneggiati e un seme fisso dove c'è casualità. |
| Sicurezza | 🟡 | Rust sicuro e semplice (`forbid(unsafe_code)`): un limite è un rifiuto e mai un taglio; il token non si stampa mai; nulla va in panico con i file ricevuti; nessuna revisione indipendente. |
| Finestra | 🔴 | Le quattro schede (Prepara, Anteprima, Dispositivo, Progetto) sono scritte con `--features gui`: elenchi di stampanti e filamenti con filtri, il piano per strati con un cursore, la coda del server con un avvio che chiede conferma, un arresto di emergenza, scorciatoie modificabili e una modalità immagine (`--shot`); la logica è testata senza schermo, ma la finestra non è ancora stata compilata né vista su uno schermo. |
| Hardware reale | 🔴 | Non provato: nulla di tutto ciò è stato eseguito su una stampante, nemmeno su una Bambu Lab A1 o una P1S. |
| Documentazione | 🟡 | README in otto lingue, il registro delle modifiche, la cartella `docs/` (architettura, roadmap, cosa non è ancora fatto, il server) e le due immagini disegnate da `docs/`. |

## 🎯 Panoramica

* **L'idea:** PHOENIX-3DP è il programma di stampa 3D della famiglia PHOENIX. Il server EH3D e i suoi client sono il centro della suite: questo programma è un client in più del server, e la modalità autonoma che parla direttamente con una stampante è un'opzione spenta per impostazione predefinita.
* **I modelli** (`p3dp-formats`): STL (testo e binario), OBJ e 3MF di base vengono letti con limiti su dimensione, numero di triangoli e contenuto di un pacchetto ZIP; ciò che non si può leggere viene rifiutato con il nome di ciò che è stato trovato. Scrive STL binario, OBJ, il proprio pacchetto 3MF e file di G-code. I progetti salvati come 3MF da altri programmi vengono letti per oggetti, piatti e alcune impostazioni (sola lettura, con limiti contro pacchetti fatti per nuocere).
* **I profili** (`p3dp-profiles`): un catalogo di 120 modelli di stampante, FDM e resina, scritto da fatti pubblici: volume, ugelli, dialetto, modi di collegamento, sistema multimateriale e le temperature massime (la cifra del produttore per i primi 46 modelli, i limiti prudenti del programma per gli altri, e ogni profilo dice quali). Dodici filamenti (i sei generici, PLA, PETG, ABS, ASA, TPU e PA, e sei con fibra di carbonio o di vetro o una miscela più tenace) sono verificati contro la stampante e l'ugello: una coppia che non va è rifiutata con le ragioni. Sette resine generiche (standard, lavabile in acqua, tenace, flessibile, fondibile, trasparente e ad alto dettaglio) arrivano con le esposizioni e la salita del loro tipo come punti di partenza. Un profilo si può modificare dalla riga di comando: il risultato è verificato per intero, un profilo modificato non è più `verified`, e un limite di temperatura alzato oltre la cifra del produttore (il piano di una Bambu Lab A1 mini, 80 C, alzato a mano a 100 C) è permesso e segnalato. Ogni profilo dice se è stato provato su una macchina; nessuno lo è stato.
* **Il G-code** (`p3dp-gcode`): scrittori per Marlin, Klipper, RepRapFirmware e Bambu, ciascuno con intestazione, inizio e fine sicuri; una temperatura o un movimento non consentiti vengono rifiutati, mai tagliati. Un controllore legge qualsiasi programma e trova le righe che superano i limiti di un modello.
* **Il collegamento** (`p3dp-link`): un trait di driver, uno stato normalizzato (quindici stati, temperature, lavoro, avvisi) e un elenco di comandi; Moonraker e OctoPrint sono implementati su un trasporto (uno HTTP semplice scritto con la libreria standard, con limiti di tempo e dimensione, provato su un socket di questo computer) e provati contro un simulatore in memoria di ciascun protocollo; Bambu LAN è il progetto dei suoi topic e messaggi, marcato sperimentale.
* **Il server** (`p3dp-server-bridge`): le impostazioni `server_address`, `server_token`, `server_machine`, `server_timeout_s`, `server_retries` e `server_mdns`, e un client del plugin `printer3d` del server sopra il client comune della suite (`eh3d-serverclient`): i lavori entrano in coda e aspettano che una persona li avvii, i limiti di temperatura sono controllati prima di inviare qualsiasi cosa, e c'è l'arresto; provato contro un server in memoria.
* **Lo slicer** (`p3dp-slicer`): strati piani di una mesh chiusa, perimetri, riempimento rettilineo, a griglia o giroide con pelli piene, ponti, supporti semplici, giunzioni, larghezza variabile, sollevamento negli spostamenti, G-code per i dialetti del programma tramite il suo scrittore e l'anteprima per strati come dati, con una stima del tempo che usa l'accelerazione; rifiuta, senza mai tagliare, ciò che non sta sul piatto, supera un limite di temperatura o non è una mesh chiusa.
* **Il visualizzatore** (`p3dp-viewer`): il modello del visualizzatore 3D senza codice grafico: una camera orbitale con prospettiva, la selezione con un clic e con un riquadro sul piano, l'intervallo di strati e le linee di un piano per una scheda grafica. Il disegno con wgpu è dietro una funzionalità, disattivata di default, e non è mai stato compilato.
* **Altri file:** il G-code binario (`bgcode`) e i pacchetti aperti di strati di resina (`.sl1`) si leggono solo dalle intestazioni, con una stima del tempo di esposizione; i formati chiusi delle stampanti a resina sono rifiutati con la ragione. Le miniature di un G-code binario sono elencate e la più grande si può estrarre (`project FILE --thumbnail USCITA`), e i dati e le miniature nei commenti di un G-code semplice (il filamento usato, il tempo, l'altezza dello strato) si leggono allo stesso modo, solo dall'inizio e dalla fine.
* **Non ancora fatto:** il disegno del visualizzatore 3D (il suo modello esiste), uno slicer per più materiali, il collegamento Bambu LAN e gli altri trasporti. La roadmap è in `docs/ROADMAP.md` e ciò che manca in `docs/NOT_YET.md`.

## 🧩 Ruolo in EH3D

EH3D-PHOENIX-3DP è il programma di stampa 3D della famiglia PHOENIX della EH3D Fabrication Suite. Oggi è un insieme di librerie e una riga di comando; la finestra si compila con `--features gui`. Il server `eh3d-server` e i suoi client sono il centro della suite: stampanti, coda e permessi sono pensati per vivere nel server (vi provvede il plugin `printer3d`), e questo programma parla con esso con le impostazioni comuni della suite. Vedi [docs/SERVER.md](docs/SERVER.md).

## 🏛️ Architettura

<p align="center">
  <img src="images/architecture.svg" alt="Architettura di EH3D-PHOENIX-3DP: chi lo usa, che cosa contiene e da che cosa dipende" width="100%">
</p>


Come si incastrano le parti di questo repository: chi lo usa, cosa contiene e da cosa dipende. L'immagine è disegnata da `docs/architecture.json` e verificata con il codice.

## ✨ Funzionalità

* **Un limite è un rifiuto:** una temperatura o un movimento non consentiti sono un errore che nomina il valore e il limite, mai un valore tagliato per farlo entrare; scrittori, driver e controllore lo rispettano.
* **Niente parte da solo:** le richieste alla coda del server dicono `auto_start: false` e chiedono conferma; il G-code diretto è spento per impostazione predefinita e, acceso, va riga per riga con gli stessi limiti.
* **Uno stato per tutti i protocolli:** quindici stati normalizzati, temperature, lavoro e avvisi, con il testo originale della stampante tenuto a parte, così il resto del programma parla una sola lingua.
* **Simulatori per i test:** ogni protocollo ha un simulatore in memoria con guasti a richiesta (nessuna connessione, timeout, 503), quindi tentativi, rifiuti e arresti si provano senza rete.
* **Clean room e grafica propria:** scritto da protocolli pubblici, manuali e fatti; nessun codice, testo, icona o file di profilo di un altro slicer; ogni profilo dice `verified = false` finché non è stato provato.
* **Otto lingue:** i testi del programma e la documentazione esistono in inglese, spagnolo, tedesco, francese, italiano, giapponese, cinese e portoghese del Portogallo.
* **Nessun codice `unsafe`:** `#![forbid(unsafe_code)]` in ogni crate.

## 🔒 Sicurezza e protezione

| Minaccia | Difesa |
|---|---|
| Un file di modello danneggiato, tagliato od ostile | le dimensioni si controllano prima di allocare qualsiasi cosa; triangoli, vertici, estensione, voci di un pacchetto ZIP e ciò a cui si espandono sono limitati, e una bomba viene rifiutata; i test di fuzzing tagliano file validi in ogni posizione e inseriscono rumore, e nulla deve andare in panico |
| Un pacchetto con una sorpresa dentro (componenti, oggetti in altri file, entità dichiarate) | viene nominata e rifiutata, mai saltata; un'entità XML dichiarata è un errore |
| Una temperatura oltre ciò che il modello consente | rifiutata con il valore e il limite; mai tagliata al limite; la stessa regola in scrittori, driver e controllore |
| Una riga di G-code diretto che aggira i limiti | spento per impostazione predefinita; riga per riga; controllato con gli stessi limiti di tutto il resto |
| Un nome di file o un id che entra in un percorso o in un comando | niente cartelle, niente `..`, niente caratteri di controllo, solo estensioni note, codifica percentuale di ciò che entra in un percorso |
| Il token del server o la chiave di una stampante in un log o su uno schermo | tenuto in un tipo che stampa `***`; le impostazioni si mostrano con il token nascosto |
| Una coda che avvia un lavoro da sola | le richieste dicono `auto_start: false` e chiedono conferma; se il server possa avviare lavori da solo è politica del server |
| Una rete instabile | si ritenta solo dove nulla potrebbe accadere due volte: una connessione rifiutata si ritenta per qualsiasi richiesta, un timeout solo per quelle che si possono rinviare, mai per un POST |
| Uso non sicuro della memoria | Rust sicuro e semplice con `forbid(unsafe_code)` |

**Ciò che non è difeso:** un programma scritto in modo pulito non è un programma sicuro da eseguire. I limiti sono quelli dichiarati dai produttori e non sono stati verificati su una macchina; i driver hanno incontrato solo simulatori; non c'è stata una revisione indipendente. Segnala una vulnerabilità in privato (vedi [SECURITY.md](SECURITY.md)).

## 🔌 Interfacce

Le librerie sono `p3dp_core`, `p3dp_formats`, `p3dp_profiles`, `p3dp_gcode`, `p3dp_link`, `p3dp_server_bridge`, `p3dp_slicer` e `p3dp_viewer`; il programma è la libreria `eh3d_phoenix_3dp` e la riga di comando `phoenix-3dp`. Ogni comando stampa il risultato sullo standard output e segnala un errore sullo standard error con un codice di uscita diverso da zero (1 per un fallimento di ciò che era richiesto, 2 per un errore nel comando). L'unico accesso alla rete è il trasporto dei driver e del collegamento al server, e solo verso un indirizzo dato.

```text
phoenix-3dp printers [--vendor NAME] [--tech fdm|msla] [--search TEXT]
phoenix-3dp info MODEL [--printer ID]
phoenix-3dp check-profile ID [--gcode FILE]
phoenix-3dp check-profile --file PROFILE [--gcode FILE]
phoenix-3dp check-profile ID|--file PROFILE --set "key=value;key=value" [--save FILE] [--force]
phoenix-3dp convert IN OUT [--printer ID] [--force]
phoenix-3dp filaments [--printer ID] [--nozzle MM] [--hardened]
phoenix-3dp resins [--kind KIND] [--printer ID] [--part-ml ML]
phoenix-3dp slice MODEL --printer ID [--settings FILE] [--filament ID] [--nozzle MM] [--hardened] [--report] [--out FILE.gcode] [--force]
phoenix-3dp project FILE.3mf|FILE.bgcode|FILE.gcode|FILE.sl1 [--thumbnail FILE] [--force] [--bottom-layers N] [--lift MM] [--light-power MW]
phoenix-3dp version
phoenix-3dp gui [SETTINGS] [--shot FILE.png]                       the window (a build with --features gui)
```

## ⚙️ Configurazione

Le impostazioni sono un file di righe `chiave = valore` (`--config FILE`): i limiti dei lettori (`limit_*`), il collegamento al server (`server_address`, `server_token`, `server_machine`, `server_timeout_s`, `server_retries`, `server_mdns`, `server_max_answer_mib`), `direct_mode` (autonomo, spento per impostazione predefinita), `lang` e `default_printer`, i prezzi (`price_filament_per_kg`, `price_resin_per_litre`, `resin_waste_percent`), il rapporto (`report_slowest_layers`, `report_short_layer_s`), la coda (`queue_refuse_duplicates`, `queue_max_waiting`) e fin dove si cerca in un G-code semplice (`limit_gcode_head_kib`, `limit_gcode_tail_kib`, `limit_gcode_thumbnails`). Un profilo di stampante è un altro file dello stesso tipo (`examples/my-printer.cfg`). Una chiave sconosciuta o un valore fuori intervallo è un errore che nomina la sua riga; tutti i valori predefiniti sono prudenti. Vedi `examples/phoenix-3dp.cfg`.

## 🚀 Avvio rapido

Serve una toolchain Rust stabile (1.88 o successiva). Clona prima EH3D-COMMON accanto a questo repository: da lì viene il client comune del server.

```bash
cargo build --release
cargo test --workspace
cargo run --release -- printers --search bambu
cargo run --release -- info examples/cube-20mm.stl --printer bambu-lab-a1
cargo run --release -- check-profile --file examples/my-printer.cfg --gcode examples/job-too-hot.gcode
cargo run --release -- convert examples/cube-20mm.stl cube.3mf --printer prusa-mk4s
cargo run --release --features gui -- gui   # the window
```

## 🧪 Test

La suite ha 537 test (`cargo test --workspace`, eseguiti su Windows e Linux nella CI), con seme fisso dove c'è casualità. Verificano i casi normali, i casi limite e gli input danneggiati: file tagliati, rumore e intestazioni ostili non vanno mai in panico, e nessuna temperatura di nessuna stampante del catalogo supera il suo limite. La CI esegue anche `cargo fmt --all --check`, `cargo clippy` con gli avvisi come errori e `python -I tools/ci_validate.py`.

## 🏗️ Compilazione e rilascio

La versione vive in `Cargo.toml` e in `eh3d.project.json`; ogni versione ha la sua voce in [CHANGELOG.md](CHANGELOG.md). `build.bat` / `build.sh` compilano il programma, `test.bat` / `test.sh` eseguono i test e `run.bat` / `run.sh` lo eseguono con un esempio. Le versioni sono pubblicate su GitHub. Non ne è stata ancora pubblicata nessuna.

## 📂 Struttura del repository

<p align="center">
  <img src="images/repository-structure.svg" alt="Struttura del repository EH3D-PHOENIX-3DP" width="100%">
</p>

<details>
<summary>Versione testuale dell'albero</summary>

```text
EH3D-PHOENIX-3DP/
├── crates/
│   ├── p3dp-core/                               unità, mesh e controlli, limiti, JSON, impostazioni
│   ├── p3dp-formats/                            lettori di STL, OBJ, 3MF, progetti, bgcode, commenti del G-code e resina; scrittori, ZIP, PNG
│   ├── p3dp-profiles/                           stampanti da fatti pubblici, filamenti, resine, il file di profilo e le sue modifiche
│   ├── p3dp-gcode/                              scrittori di G-code per quattro dialetti, il controllore
│   ├── p3dp-link/                               driver, stato normalizzato, TCP, simulatori, Bambu LAN (byte del protocollo, nessuna connessione)
│   ├── p3dp-server-bridge/                      impostazioni del collegamento al server, client del plugin, regole e quadro della coda
│   ├── p3dp-slicer/                             lo slicer: strati, perimetri, riempimento, ponti, supporti, skirt, brim, raft, G-code
│   └── p3dp-viewer/                             il modello del visualizzatore 3D: camera, selezione, strati, spostamenti
├── src/
│   ├── lib.rs                                   la libreria del programma: ciò che usano la riga di comando e la finestra
│   ├── main.rs                                  la riga di comando e l'avvio della finestra
│   ├── cli.rs                                   i comandi: printers, info, check-profile, convert, filaments, resins, slice, project, version
│   ├── app_state.rs                             lo stato della finestra senza finestra: schede, stampante, modello, rapporti, dispositivo simulato
│   ├── config.rs                                il file di impostazioni: limiti, collegamento al server, lingua, stampante predefinita, prezzi, rapporto e coda
│   ├── device.rs                                le stampanti del server, la coda e le sue regole, un avvio confermato, l'arresto di emergenza
│   ├── i18n.rs                                  i testi del programma nelle otto lingue
│   ├── keys.rs                                  le scorciatoie da tastiera: lo schema della famiglia, uno proprio e le modifiche
│   ├── lists.rs                                 gli elenchi di stampanti e di filamenti con i loro filtri
│   ├── look.rs                                  i colori, il marchio e i disegni della finestra e la sua disposizione per dimensione
│   ├── preview.rs                               elaborare dalla finestra, il cursore degli strati, il piano sullo schermo, il rapporto
│   ├── project.rs                               il progetto: un modello e la sua stampante nel pacchetto 3MF proprio
│   ├── report.rs                                le parole dei rapporti: elaborazione, resina, miniature e modifiche dei profili, nelle otto lingue
│   ├── server_ui.rs                             le impostazioni del server, lo stato del collegamento e l'accesso, sul pannello della suite
│   ├── shot.rs                                  la modalità immagine della finestra (--shot) e il file PNG che scrive
│   ├── view.rs                                  la camera orbitale che trasforma una mesh in linee per l'anteprima
│   └── gui.rs                                   la finestra (egui, con la funzione gui): disegna lo stato e inoltra ciò che fa la persona
├── tests/
│   ├── end_to_end.rs                            test da un modello a un programma a una stampante simulata
│   ├── examples.rs                              test che i file di examples/ funzionino con i comandi
│   ├── fuzz.rs                                  test di fuzzing con seme fisso: file tagliati, danneggiati e ostili
│   ├── readme.rs                                test che i README dicano la verità: il numero di test e i comandi
│   └── safety.rs                                test che nessuna temperatura di nessuna stampante del catalogo superi il suo limite
├── examples/
│   ├── cube-20mm.stl                            un cubo da 20 mm come STL di testo
│   ├── cube-20mm.obj                            lo stesso cubo come OBJ
│   ├── phoenix-3dp.cfg                          il file di impostazioni con ogni chiave e il suo valore predefinito
│   ├── my-printer.cfg                           un profilo di stampante scritto a mano (una macchina Klipper)
│   ├── job-safe.gcode                           un programma che rispetta i limiti di quel profilo
│   └── job-too-hot.gcode                        un programma che li supera su due righe
├── docs/
│   ├── ARCHITECTURE.md                          il programma crate per crate
│   ├── ROADMAP.md                               i nove passi dalla base a un programma completo
│   ├── NOT_YET.md                               cosa non è ancora fatto e di cosa ha bisogno ogni parte
│   ├── SERVER.md                                come si integra il programma con il server EH3D
│   ├── architecture.json                        dati dell'immagine di Architettura
│   └── structure.json                           dati dell'immagine di Struttura
├── tools/                                       controllo della CI e del README e strumenti per le immagini
├── images/                                      banner e le due immagini del README
├── .github/                                     workflow di CI e di rilascio, modelli di issue e pull request
├── eh3d.project.json                            manifesto: versione, ruolo, maturità e comando di build
├── Cargo.toml                                   il workspace e i suoi crate
├── rust-toolchain.toml                          la toolchain Rust fissata
├── CHANGELOG.md                                 cosa è cambiato in ogni versione
├── SECURITY.md SUPPORT.md                       come segnalare una vulnerabilità e dove chiedere
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   come contribuire, il codice di condotta e la licenza
```

</details>

## 🗺️ Roadmap e limiti noti

**Limiti noti oggi**

* Lo slicer è piccolo: i supporti sono semplici (niente albero, niente strati di interfaccia), i ponti non sono rifiniti, non c'è nulla per più materiali, e nessuna stampa ne è uscita su una stampante.
* Nessuna stampante è stata collegata, nemmeno la Bambu Lab A1 o la P1S; ogni profilo dice `verified = false`.
* Il lettore 3MF prende solo la mesh (componenti ed estensione di produzione vengono rifiutati); i progetti di altri programmi sono letti per oggetti, piatti e impostazioni e mai modificati.
* I driver hanno un trasporto HTTP semplice, provato solo su un socket di questo computer e con i simulatori; niente TLS né push WebSocket; Bambu LAN è un progetto che non si collega, e il cloud di Bambu non ne fa parte.
* La finestra non è stata compilata né vista su uno schermo.

**Prossimi passi**

* H1: geometria delle mesh e un visualizzatore 3D proprio.
* H2: un cubo di calibrazione stampato su una macchina reale (il riempimento a griglia e giroide, le giunzioni, la larghezza variabile, i ponti e i supporti semplici sono scritti e provati su forme a seme fisso).
* H3: l'editor dei profili e la finestra di base.
* H7: gli altri trasporti (push WebSocket, PrusaLink, Duet) e il collegamento Bambu LAN.
* H8: provarlo su una Bambu Lab A1 e una P1S e marcare come verificati i profili che funzionano. L'intero piano è in `docs/ROADMAP.md`.

## 🌍 Lingue

Il README e i testi esistono in inglese, spagnolo, francese, italiano, tedesco, cinese, giapponese e portoghese europeo.

## 🔗 Progetti correlati

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON): i crate condivisi, tra cui il client comune del server
* [EH3D-SERVER](https://github.com/JuanenRac/EH3D-SERVER): il server della suite, dove sono previste le stampanti, la coda e i permessi
* [EH3D-STUDIO](https://github.com/JuanenRac/EH3D-STUDIO): il pannello di configurazione della suite
* [EH3D-PHOENIX-LASER](https://github.com/JuanenRac/EH3D-PHOENIX-LASER): incisione e taglio laser, il programma gemello della stessa famiglia
* [EH3D-PHOENIX-PNP](https://github.com/JuanenRac/EH3D-PHOENIX-PNP): pick and place, il programma gemello della stessa famiglia
* [EH3D-SUITE-LAUNCHER](https://github.com/JuanenRac/EH3D-SUITE-LAUNCHER): l'hub che trova e avvia i programmi della suite

## 📚 Documentazione

* [Registro delle modifiche](CHANGELOG.md)
* [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): Architettura del programma, crate per crate
* [docs/ROADMAP.md](docs/ROADMAP.md): Roadmap: i nove passi dalla base a un programma completo
* [docs/NOT_YET.md](docs/NOT_YET.md): Cosa non è ancora fatto e di cosa ha bisogno ogni parte mancante
* [docs/SERVER.md](docs/SERVER.md): Come si integra il programma con il server EH3D
* [docs/architecture.json](docs/architecture.json): Dati delle due immagini

## 🤝 Contribuire e comunità

* Prima di una pull request eseguite `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` e `python -I tools/ci_validate.py`: tutto deve passare. Vedi [CONTRIBUTING.md](CONTRIBUTING.md) e il [codice di condotta](CODE_OF_CONDUCT.md).
* Il README esiste in otto lingue: una modifica al testo si fa in tutti e otto i file, e le due immagini sono disegnate da `docs/architecture.json` e `docs/structure.json` con `tools/make_architecture.py` e `tools/make_structure.py`.
* Segnalate una vulnerabilità in privato: [SECURITY.md](SECURITY.md). Domande, idee e segnalazioni: electrohobby3d@gmail.com ([SUPPORT.md](SUPPORT.md)).

## 👤 AUTORE

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 LICENZA

Software: GPL-3.0-or-later. Vedi [LICENSE](LICENSE). I progetti hardware della suite usano la CERN OHL-S.
