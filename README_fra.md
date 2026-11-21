<p align="center">
  <img src="images/EH3D_PHOENIX_3DP_BANNER.svg" alt="EH3D-PHOENIX-3DP banner" width="100%">
</p>

# EH3D-PHOENIX-3DP
<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | 🇫🇷 <b>Français</b> | <a href="README_ita.md">🇮🇹 Italiano</a> | <a href="README_deu.md">🇩🇪 Deutsch</a> | <a href="README_zho.md">🇨🇳 中文</a> | <a href="README_jpn.md">🇯🇵 日本語</a> | <a href="README_por.md">🇵🇹 Português</a></p>

### Préparer et envoyer des impressions 3D : modèles, profils d'imprimante, G-code sûr et la liaison avec vos imprimantes et le serveur EH3D

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.7-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-functional-4CAF50.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-659-4CAF50.svg" alt="Tests">
</p>

---

**Contrôle d'honnêteté - ce qui fonctionne vraiment aujourd'hui :** un espace de travail Rust de huit bibliothèques et un programme en ligne de commande (`phoenix-3dp`) avec 659 tests qui s'exécutent sur un ordinateur. Il lit et vérifie des modèles STL, OBJ et 3MF, contient un catalogue de 120 profils d'imprimante construits à partir de données publiques (aucun essayé sur une machine), écrit du G-code pour les dialectes Marlin, Klipper, RepRapFirmware et Bambu avec des limites de température qui refusent une valeur au lieu de la rogner, et pilote des imprimantes Moonraker et OctoPrint et le serveur EH3D, mais seulement contre des simulateurs (en mémoire et sur un socket de cet ordinateur). Il découpe un modèle fermé en couches planes avec des périmètres, un remplissage rectiligne et des peaux pleines (un petit trancheur avec remplissage en grille et en gyroïde, ponts, supports simples, coutures, largeur variable, levée en déplacement, jupe, bordure ou radeau, mode vase, couches courtes ralenties, rétraction qui suit le déplacement et une estimation du temps avec accélération ; encore rien pour plusieurs matériaux) ; la fenêtre n'a pas été compilée ni vue sur un écran ; rien n'a tourné sur une vraie imprimante.

---

> **État de sécurité :** ce programme ne commande pas une machine à lui seul et n'est pas un système de sécurité. Ce qu'il produit doit être vérifié par une personne avant tout lancement sur une imprimante ; rien de tout cela n'a tourné sur du vrai matériel.

> **Noms de tiers :** les noms de fabricants, de modèles et de protocoles d'imprimantes (Bambu Lab, Prusa, Creality, Klipper, Moonraker, OctoPrint, Marlin et d'autres) appartiennent à leurs propriétaires et ne servent qu'à dire de quelle machine ou de quel protocole il s'agit ; ce programme n'est ni fait, ni approuvé, ni parrainé par eux. Il est écrit à partir de documentation publique et de faits, pas à partir du code, des textes, des icônes ni des fichiers de profil d'un autre trancheur ; son nom, son graphisme et ses textes lui sont propres.


## 📋 En un coup d'œil

| | |
|---|---|
| Version | `0.0.7` |
| Maturité | Fonctionnel |
| Tests | 659 |
| Pile technique | Rust |
| Sur du matériel réel | Pas essayé : rien de tout cela n'a tourné sur une imprimante, pas même sur une Bambu Lab A1 ou une P1S. |

**Aller à  :** [Présentation](#-présentation) · [Architecture](#-architecture) · [Démarrage rapide](#-démarrage-rapide) · [Tests](#-tests) · [Structure](#-structure-du-dépôt) · [Liés](#-projets-liés)

## 🚦 État du projet

Chaque ligne est une partie de ce dépôt et le feu dit où elle en est aujourd'hui. Elle est écrite d'après ce que le dépôt peut montrer (tests, compilations, exécutions), pas d'après ce qui est prévu.

🟢 Fait et vérifié · 🟡 Fonctionne, avec des limites connues · 🟠 À moitié fait ou pas encore vérifié · 🔴 Pas fait ou ne fonctionne pas

| Domaine | État | Ce que cela veut dire ici |
|---|---|---|
| Cœur | 🟡 | unités en nanomètres, le maillage et ses vérifications, les limites des lecteurs, un petit lecteur JSON et un secret qui ne s'affiche jamais ; testé sur ordinateur avec des entrées normales et abîmées. |
| Modèles et fichiers | 🟡 | STL (texte et binaire), OBJ et 3MF de base sont lus avec des limites de taille et des refus qui disent ce qu'ils ont trouvé ; le paquet 3MF propre, le STL binaire, l'OBJ et le fichier de G-code sont écrits ; des projets 3MF d'autres programmes on lit les objets, les plateaux et quelques réglages (lecture seule) ; vérifié avec des tests de fuzzing, pas avec une collection de vrais fichiers d'autres programmes. |
| Profils d'imprimante | 🟠 | 120 modèles à partir de données publiques (volume, buses, dialecte, modes de connexion, limites du fabricant pour les 46 premiers, dont les Bambu Lab A1 et P1S, et limites prudentes du programme pour les autres, qui le disent ; les filaments de la base de données des matériaux d'impression de la suite (477, les profils génériques et ceux que publient les fabricants) vérifiés contre l'imprimante et la buse, sept résines de la même base et des modifications d'un profil qui disent ce qui a changé) ; tous disent `verified = false`, car aucun n'a été essayé sur une machine. |
| G-code | 🟡 | écrivains pour les quatre dialectes avec un début et une fin sûrs ; une température au-dessus de la limite est refusée et rien n'est écrit ; un vérificateur trouve les lignes d'un programme qui dépassent une limite ; jamais exécuté sur une imprimante. |
| Pilotes d'imprimante | 🟠 | Moonraker et OctoPrint sur un transport, essayés seulement contre des simulateurs de leurs protocoles ; Bambu LAN est une conception marquée expérimentale qui ne se connecte pas. |
| Liaison avec le serveur | 🟠 | les réglages de la liaison et un client du greffon d'imprimantes du serveur au-dessus du client commun de la suite (connexion, renouvellement, nouvelles tentatives, erreurs en huit langues), essayé contre un serveur en mémoire ; jamais contre le vrai serveur. |
| Trancheur | 🟡 | couches planes d'un maillage fermé, périmètres, remplissage rectiligne, en grille ou en gyroïde, peaux pleines, ponts, supports simples, coutures, largeur variable, G-code par dialecte, un aperçu par couches sous forme de données et une estimation du temps avec accélération ; vérifié par des propriétés sur des formes à graine fixe (le plastique déposé est le volume de la pièce, rien ne sort du plateau, aucune température ne dépasse une limite) ; aucune impression n'en est sortie sur une imprimante. |
| Ligne de commande | 🟡 | `phoenix-3dp` (`printers`, `info`, `check-profile`, `convert`, `filaments`, `resins`, `slice`, `project`, `version`) exécute les fonctions des bibliothèques et affiche le résultat ; chaque erreur revient sous forme de message et d'un code de sortie non nul. |
| Tests | 🟡 | 659 tests réussissent (`cargo test --workspace`), avec des cas normaux, des cas limites, des entrées abîmées et une graine fixe là où il y a du hasard. |
| Sécurité | 🟡 | Rust sûr et simple (`forbid(unsafe_code)`) : une limite est un refus et jamais une coupe ; le jeton ne s'affiche jamais ; rien ne panique sur les fichiers reçus ; pas de revue indépendante. |
| Fenêtre | 🔴 | Les quatre onglets (Préparer, Aperçu, Appareil, Projet) sont écrits avec `--features gui` : listes d'imprimantes et de filaments avec filtres, le plan par couches avec un curseur, la file du serveur avec un démarrage qui demande une confirmation, un arrêt d'urgence, des raccourcis modifiables, un mode image (`--shot`), une vue 3D du plateau avec les pièces en volumes ombrés, un panneau de pièces avec orienter auto et disposer, la découpe en arrière-plan avec estimations, des limites de température modifiables et un aspect sombre et un aspect clair ; les panneaux sont dans le cadre ancrable de la suite (on peut les retirer, les remettre, les déplacer, les faire flotter et les replier, avec des espaces de travail nommés gardés dans le fichier de réglages et une barre de menus) ; la logique est testée sans écran, mais la fenêtre n'a pas encore été compilée ni vue sur un écran. |
| Matériel réel | 🔴 | Pas essayé : rien de tout cela n'a tourné sur une imprimante, pas même sur une Bambu Lab A1 ou une P1S. |
| Documentation | 🟡 | README en huit langues, le journal des modifications, le dossier `docs/` (architecture, feuille de route, ce qui n'est pas encore fait, le serveur) et les deux images dessinées depuis `docs/`. |

## 🎯 Présentation

* **L'idée :** PHOENIX-3DP est le programme d'impression 3D de la famille PHOENIX. Le serveur EH3D et ses clients sont le centre de la suite : ce programme est un client de plus du serveur, et le mode autonome qui parle directement à une imprimante est une option désactivée par défaut.
* **Les modèles** (`p3dp-formats`) : STL (texte et binaire), OBJ et 3MF de base sont lus avec des limites de taille, de nombre de triangles et de contenu d'un paquet ZIP ; ce qui ne peut pas être lu est refusé avec le nom de ce qui a été trouvé. Il écrit du STL binaire, de l'OBJ, son propre paquet 3MF et des fichiers de G-code. Les projets enregistrés en 3MF par d'autres programmes sont lus pour leurs objets, plateaux et quelques réglages (lecture seule, avec des limites contre les paquets conçus pour nuire).
* **Les profils** (`p3dp-profiles`) : un catalogue de 120 modèles d'imprimante, FDM et résine, écrit à partir de faits publics : volume, buses, dialecte, modes de connexion, système multi-matériaux et températures maximales (le chiffre du fabricant pour les 46 premiers modèles, les limites prudentes du programme pour les autres, et chaque profil dit lesquelles). Les filaments (477 : les profils génériques, qui gardent leurs anciens identifiants comme `generic-pla`, et ceux que publient les fabricants) viennent de la base de données des matériaux d'impression de la suite (`eh3d-printmaterials`), se cherchent par fabricant, matière ou nom et sont vérifiés contre l'imprimante et la buse : un couple qui ne convient pas est refusé avec les raisons. Sept résines génériques de la même base (standard, lavable à l'eau, tenace, flexible, calcinable, transparente et haute définition) viennent avec les expositions et la levée de leur type comme points de départ. Un profil peut être modifié depuis la ligne de commande : le résultat est vérifié en entier, un profil modifié n'est plus `verified`, et une limite de température relevée au-delà du chiffre du fabricant (le plateau d'une Bambu Lab A1 mini, 80 C, relevé à la main à 100 C) est permise et signalée. Chaque profil dit s'il a été essayé sur une machine ; aucun ne l'a été.
* **Le G-code** (`p3dp-gcode`) : des écrivains pour Marlin, Klipper, RepRapFirmware et Bambu, chacun avec un en-tête, un début et une fin sûrs ; une température ou un mouvement non permis est refusé, jamais rogné. Un vérificateur lit n'importe quel programme et trouve les lignes qui dépassent les limites d'un modèle.
* **La liaison** (`p3dp-link`) : un trait de pilote, un état normalisé (quinze états, températures, travail, alertes) et une liste de commandes ; Moonraker et OctoPrint sont implémentés sur un transport (un HTTP simple écrit avec la bibliothèque standard, avec des limites de temps et de taille, essayé sur un socket de cet ordinateur) et testés contre un simulateur en mémoire de chaque protocole ; Bambu LAN est la conception de ses sujets et messages, marquée expérimentale.
* **Le serveur** (`p3dp-server-bridge`) : les réglages `server_address`, `server_token`, `server_machine`, `server_timeout_s`, `server_retries` et `server_mdns`, et un client du greffon `printer3d` du serveur au-dessus du client commun de la suite (`eh3d-serverclient`) : les travaux entrent dans la file et attendent qu'une personne les lance, les limites de température sont vérifiées avant tout envoi, et il y a l'arrêt ; testé contre un serveur en mémoire.
* **Le trancheur** (`p3dp-slicer`) : couches planes d'un maillage fermé, périmètres, remplissage rectiligne, en grille ou en gyroïde avec peaux pleines, ponts, supports simples, coutures, largeur variable, levée en déplacement, G-code pour les dialectes du programme via son écrivain et l'aperçu par couches sous forme de données, avec une estimation du temps qui utilise l'accélération ; il refuse, sans jamais rogner, ce qui ne tient pas sur le plateau, dépasse une limite de température ou n'est pas un maillage fermé.
* **Le visualiseur** (`p3dp-viewer`) : le modèle du visualiseur 3D sans code graphique : une caméra orbitale en perspective, la sélection au clic et par cadre sur le plateau, la plage de couches et les lignes d'un plan pour une carte graphique. Le dessin avec wgpu est derrière une fonctionnalité, désactivée par défaut, et n'a jamais été compilé.
* **Autres fichiers :** le G-code binaire (`bgcode`) et les paquets ouverts de couches de résine (`.sl1`) sont lus par leurs seuls en-têtes, avec une estimation du temps d'exposition ; les formats fermés des imprimantes à résine sont refusés avec la raison. Les vignettes d'un G-code binaire sont listées et la plus grande peut être extraite (`project FICHIER --thumbnail SORTIE`), et les chiffres et vignettes des commentaires d'un G-code simple (le filament utilisé, la durée, la hauteur de couche) sont lus de la même façon, seulement depuis son début et sa fin.
* **Pas encore fait :** le dessin du visualiseur 3D (son modèle existe), un trancheur pour plusieurs matériaux, la liaison Bambu LAN et les autres transports. La feuille de route est dans `docs/ROADMAP.md` et ce qui manque dans `docs/NOT_YET.md`.

## 🧩 Rôle dans EH3D

EH3D-PHOENIX-3DP est le programme d'impression 3D de la famille PHOENIX de la EH3D Fabrication Suite. Aujourd'hui c'est un ensemble de bibliothèques et une ligne de commande ; la fenêtre se compile avec `--features gui`. Le serveur `eh3d-server` et ses clients sont le centre de la suite : les imprimantes, la file et les permissions sont destinées à vivre dans le serveur (le greffon `printer3d` l'y sert), et ce programme lui parle avec les réglages communs de la suite. Voir [docs/SERVER.md](docs/SERVER.md).

## 🏛️ Architecture

<p align="center">
  <img src="images/architecture.svg" alt="Architecture de EH3D-PHOENIX-3DP : qui l'utilise, ce qu'il contient et de quoi il dépend" width="100%">
</p>


Comment les parties de ce dépôt s'articulent : qui l'utilise, ce qu'il contient et de quoi il dépend. L'image est dessinée depuis `docs/architecture.json` et vérifiée avec le code.

## 🖼️ Captures d'écran

<p align="center">
  <img src="images/layouts/prepare.svg" alt="Espace Préparer : la barre de menus en haut ; les réglages d'impression et les matériaux en onglets à gauche, la vue 3D au centre au-dessus de la console et du G-code, et les objets et l'imprimante en onglets à droite." width="100%">
</p>

<p align="center">
  <img src="images/layouts/review.svg" alt="Espace Réviser : la vue 3D et l'aperçu des couches en onglets, avec la légende du G-code au-dessus des réglages d'impression à droite." width="100%">
</p>

<p align="center">
  <img src="images/layouts/run.svg" alt="Espace Exécuter : l'appareil (le serveur et ses machines), l'état de la machine et la file côte à côte." width="100%">
</p>

<p align="center">
  <img src="images/layouts/setup.svg" alt="Espace Réglages : le projet et les réglages de la fenêtre, avec la console en dessous." width="100%">
</p>

<p align="center">
  <img src="images/layouts/compact.svg" alt="Espace Compact : un seul groupe d'onglets, la vue 3D et les réglages d'impression." width="100%">
</p>

Schémas de la disposition d'usine de chaque espace de travail, faits à partir du modèle de la fenêtre (`cargo run --example workspace_pictures`) : la barre de menus et les groupes de panneaux avec leurs onglets. Ce ne sont pas des photographies de la fenêtre ; elles sont prises avec le mode image du programme (`--shot --demo`, un modèle et un serveur créés en mémoire, donc rien n'atteint une imprimante) par le workflow `Window pictures`.

## ✨ Fonctionnalités

* **Une limite est un refus :** une température ou un mouvement non permis est une erreur qui nomme la valeur et la limite, jamais une valeur rognée pour entrer ; les écrivains, les pilotes et le vérificateur la respectent.
* **Rien ne démarre seul :** les demandes à la file du serveur disent `auto_start: false` et demandent une confirmation ; le G-code brut est désactivé par défaut et, activé, passe ligne par ligne sous les mêmes limites.
* **Un état pour tous les protocoles :** quinze états normalisés, températures, travail et alertes, avec le texte d'origine de l'imprimante à part, de sorte que le reste du programme parle une seule langue.
* **Des simulateurs pour les tests :** chaque protocole a un simulateur en mémoire avec des pannes à la demande (pas de connexion, délai dépassé, 503), si bien que reprises, refus et arrêts sont testés sans réseau.
* **Une fenêtre à la manière d'un trancheur :** onglets Préparer, Aperçu, Appareil et Projet ; une vue 3D du plateau avec les pièces en volumes ombrés ; un panneau pour l'imprimante, le filament et le procédé ; les chiffres de chaque pièce (placer, tourner, taille), orienter auto et disposer ; la découpe en arrière-plan avec les estimations en bas ; un aspect sombre et un aspect clair. Les panneaux sont dans le cadre ancrable de la suite : on peut les retirer, les remettre, les déplacer, les grouper en onglets, les faire flotter et les replier, les quatre onglets sont des espaces de travail (Préparer, Réviser, Exécuter, Configurer et Compact), la disposition est gardée dans le fichier de réglages et une barre de menus offre Fichier, Édition, Affichage, Outils, Fenêtre et Aide. La logique est testée sans écran ; la fenêtre elle-même n'a pas encore été vue.
* **Salle blanche et graphisme propre :** écrit à partir de protocoles publics, de manuels et de faits ; aucun code, texte, icône ni fichier de profil d'un autre trancheur ; chaque profil dit `verified = false` tant qu'il n'a pas été essayé.
* **Huit langues :** les textes du programme et la documentation existent en anglais, espagnol, allemand, français, italien, japonais, chinois et portugais du Portugal.
* **Pas de code `unsafe` :** `#![forbid(unsafe_code)]` dans chaque crate.

## 🔒 Sécurité et protection

| Menace | Défense |
|---|---|
| Un fichier de modèle abîmé, coupé ou hostile | les tailles sont vérifiées avant toute allocation ; les triangles, les sommets, l'étendue, les entrées d'un paquet ZIP et ce qu'elles décompressent sont limités, et une bombe est refusée ; les tests de fuzzing coupent des fichiers valides à chaque position et injectent du bruit, et rien ne doit paniquer |
| Un paquet avec une surprise dedans (composants, objets dans d'autres fichiers, entités déclarées) | elle est nommée et refusée, jamais ignorée ; une entité XML déclarée est une erreur |
| Une température au-dessus de ce que permet le modèle | refusée avec la valeur et la limite ; jamais rognée à la limite ; la même règle dans les écrivains, les pilotes et le vérificateur |
| Une ligne de G-code brut qui contourne les limites | désactivé par défaut ; ligne par ligne ; vérifié avec les mêmes limites que tout le reste |
| Un nom de fichier ou un identifiant qui entre dans un chemin ou une commande | pas de dossiers, pas de `..`, pas de caractères de contrôle, seulement des extensions connues, encodage pourcent de ce qui va dans un chemin |
| Le jeton du serveur ou la clé d'une imprimante dans un journal ou sur un écran | conservé dans un type qui affiche `***` ; les réglages sont montrés avec le jeton masqué |
| Une file qui démarre un travail toute seule | les demandes disent `auto_start: false` et demandent confirmation ; que le serveur puisse démarrer des travaux seul est la politique du serveur |
| Un réseau capricieux | on ne réessaie que là où rien ne pourrait se produire deux fois : une connexion refusée est retentée pour toute demande, un délai dépassé seulement pour celles qui peuvent être renvoyées, jamais pour un POST |
| Usage non sûr de la mémoire | Rust sûr et simple avec `forbid(unsafe_code)` |

**Ce qui n'est pas défendu :** un programme écrit proprement n'est pas un programme sûr à exécuter. Les limites sont celles que les fabricants annoncent et n'ont pas été vérifiées sur une machine ; les pilotes n'ont connu que des simulateurs ; il n'y a pas eu de revue indépendante. Signalez une vulnérabilité en privé (voir [SECURITY.md](SECURITY.md)).

## 🔌 Interfaces

Les bibliothèques sont `p3dp_core`, `p3dp_formats`, `p3dp_profiles`, `p3dp_gcode`, `p3dp_link`, `p3dp_server_bridge`, `p3dp_slicer` et `p3dp_viewer` ; le programme est la bibliothèque `eh3d_phoenix_3dp` et la ligne de commande `phoenix-3dp`. Chaque commande affiche son résultat sur la sortie standard et signale une erreur sur la sortie d'erreur avec un code de sortie non nul (1 pour un échec de ce qui était demandé, 2 pour une faute dans la commande). Le seul accès au réseau est le transport des pilotes et de la liaison avec le serveur, et seulement vers une adresse donnée.

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
phoenix-3dp gui [SETTINGS] [--open MODEL] [--printer ID] [--shot FILE.png [--window TAB] [--size WxH] [--slice] [--demo] [--peek]]  the window (a build with --features gui)
```

## ⚙️ Configuration

Les réglages sont un fichier de lignes `clé = valeur` (`--config FICHIER`) : les limites des lecteurs (`limit_*`), la liaison avec le serveur (`server_address`, `server_token`, `server_machine`, `server_timeout_s`, `server_retries`, `server_mdns`, `server_max_answer_mib`), `direct_mode` (autonome, désactivé par défaut), `lang` et `default_printer`, les prix (`price_filament_per_kg`, `price_resin_per_litre`, `resin_waste_percent`), le rapport (`report_slowest_layers`, `report_short_layer_s`), la file (`queue_refuse_duplicates`, `queue_max_waiting`) et jusqu'où un G-code simple est parcouru (`limit_gcode_head_kib`, `limit_gcode_tail_kib`, `limit_gcode_thumbnails`). Un profil d'imprimante est un autre fichier du même genre (`examples/my-printer.cfg`). Une clé inconnue ou une valeur hors plage est une erreur qui nomme sa ligne ; toutes les valeurs par défaut sont prudentes. Voir `examples/phoenix-3dp.cfg`. La fenêtre garde ses propres choix dans le même fichier : `theme` (`dark` ou `light`), `grid_mm`, `arrange_gap_mm`, `arrange_on_add`, `ui_hints`, `peek_wall_alpha` (l'opacité des parois de l'aperçu quand on regarde à l'intérieur), la disposition des panneaux dans des lignes qui commencent par `dock.` et, pour chaque imprimante dont les limites de température ont été changées, `printer_limits_<id>`.

## 🚀 Démarrage rapide

Il vous faut une chaîne d'outils Rust stable (1.88 ou plus récente). Clonez d'abord EH3D-COMMON à côté de ce dépôt : le client commun du serveur en vient.

```bash
cargo build --release
cargo test --workspace
cargo run --release -- printers --search bambu
cargo run --release -- info examples/cube-20mm.stl --printer bambu-lab-a1
cargo run --release -- check-profile --file examples/my-printer.cfg --gcode examples/job-too-hot.gcode
cargo run --release -- convert examples/cube-20mm.stl cube.3mf --printer prusa-mk4s
cargo run --release --features gui -- gui   # the window
```

## 🧪 Tests

La suite compte 659 tests (`cargo test --workspace`, exécutés sous Windows et Linux dans la CI), avec une graine fixe là où il y a du hasard. Ils vérifient les cas normaux, les cas limites et les entrées abîmées : des fichiers coupés, du bruit et des en-têtes hostiles ne font jamais paniquer, et aucune température d'aucune imprimante du catalogue ne dépasse sa limite. La CI exécute aussi `cargo fmt --all --check`, `cargo clippy` avec les avertissements en erreurs et `python -I tools/ci_validate.py`.

## 🏗️ Compilation et publication

La version vit dans `Cargo.toml` et dans `eh3d.project.json` ; chaque version a son entrée dans [CHANGELOG.md](CHANGELOG.md). `build.bat` / `build.sh` compilent le programme, `test.bat` / `test.sh` lancent les tests et `run.bat` / `run.sh` l'exécutent avec un exemple. Les versions sont publiées sur GitHub. Aucune n'a encore été publiée.

## 📂 Structure du dépôt

<p align="center">
  <img src="images/repository-structure.svg" alt="Structure du dépôt EH3D-PHOENIX-3DP" width="100%">
</p>

<details>
<summary>Version texte de l'arborescence</summary>

```text
EH3D-PHOENIX-3DP/
├── crates/
│   ├── p3dp-core/                               unités, maillage et vérifications, limites, JSON, réglages
│   ├── p3dp-formats/                            lecteurs de STL, OBJ, 3MF, projets, bgcode, commentaires de G-code et résine ; écrivains, ZIP, PNG
│   ├── p3dp-profiles/                           imprimantes à partir de faits publics, filaments, résines, le fichier de profil et ses modifications
│   ├── p3dp-gcode/                              écrivains de G-code pour quatre dialectes, le vérificateur
│   ├── p3dp-link/                               pilotes, état normalisé, TCP, simulateurs, Bambu LAN (octets du protocole, pas de connexion)
│   ├── p3dp-server-bridge/                      réglages de la liaison au serveur, client du greffon, règles et tableau de la file
│   ├── p3dp-slicer/                             le trancheur : couches, périmètres, remplissage, ponts, supports, jupe, bordure, radeau, G-code
│   └── p3dp-viewer/                             le modèle du visualiseur 3D : caméra, sélection, couches, déplacements
├── src/
│   ├── lib.rs                                   la bibliothèque du programme : ce qu'utilisent la ligne de commande et la fenêtre
│   ├── main.rs                                  la ligne de commande et le démarrage de la fenêtre
│   ├── cli.rs                                   les commandes : printers, info, check-profile, convert, filaments, resins, slice, project, version
│   ├── app_state.rs                             l'état de la fenêtre sans fenêtre : onglets, imprimante, modèle, rapports, appareil simulé
│   ├── config.rs                                le fichier de réglages : limites, liaison serveur, langue, imprimante par défaut, prix, rapport et file
│   ├── demo.rs                                  le modèle de démonstration pour les images : une coupe tournée à partir de quelques rayons
│   ├── device.rs                                les imprimantes du serveur, la file et ses règles, un démarrage confirmé, l'arrêt d'urgence
│   ├── i18n.rs                                  les textes du programme dans les huit langues
│   ├── keys.rs                                  les raccourcis clavier : le schéma de la famille, un schéma propre et les changements
│   ├── lists.rs                                 les listes d'imprimantes et de filaments avec leurs filtres
│   ├── limits_ui.rs                             modifier et conserver les limites de température d'une imprimante, avec les avertissements d'une modification
│   ├── look.rs                                  les couleurs, la marque et les dessins de la fenêtre et sa disposition selon la taille
│   ├── objects.rs                               les pièces sur le plateau : placer, tourner, échelle, orienter auto, disposer
│   ├── panels.rs                                les panneaux, espaces de travail et menus de la fenêtre comme données
│   ├── preflight.rs                             quoi vérifier avant d'imprimer, d'après la base de matériaux
│   ├── preview.rs                               découper depuis la fenêtre, le curseur de couches, le plan à l'écran, le rapport
│   ├── project.rs                               le projet : un modèle et son imprimante dans le paquet 3MF propre
│   ├── report.rs                                les mots des rapports : découpe, résine, vignettes et modifications de profil, en huit langues
│   ├── server_ui.rs                             les réglages du serveur, l'état du lien et la connexion, sur le panneau de la suite
│   ├── shade.rs                                 l'aspect plein de la vue 3D : polygones ombrés, le plateau, les vues standard et la sélection
│   ├── shot.rs                                  le mode image de la fenêtre (--shot) et le fichier PNG qu'il écrit
│   ├── slice_job.rs                             la découpe en arrière-plan et l'estimation de la barre de progression
│   ├── studio.rs                                la table de travail de Préparer : pièces, caméra, découpe en arrière-plan, estimations
│   ├── view.rs                                  la caméra orbitale qui transforme un maillage en lignes pour l'aperçu
│   └── gui.rs                                   la fenêtre (egui, fonction gui) : barres, panneaux et vue 3D ; transmet ce que fait la personne
├── tests/
│   ├── end_to_end.rs                            tests d'un modèle à un programme à une imprimante simulée
│   ├── examples.rs                              tests que les fichiers de examples/ fonctionnent avec les commandes
│   ├── fuzz.rs                                  tests de fuzzing à graine fixe : fichiers coupés, abîmés et hostiles
│   ├── readme.rs                                tests que les README disent la vérité : le nombre de tests et les commandes
│   ├── safety.rs                                tests qu'aucune température d'aucune imprimante du catalogue ne dépasse sa limite
│   └── reqs.rs                                  tests du registre des exigences
├── examples/
│   ├── cube-20mm.stl                            un cube de 20 mm en STL texte
│   ├── cube-20mm.obj                            le même cube en OBJ
│   ├── phoenix-3dp.cfg                          le fichier de réglages avec chaque clé et sa valeur par défaut
│   ├── my-printer.cfg                           un profil d'imprimante écrit à la main (une machine Klipper)
│   ├── job-safe.gcode                           un programme qui respecte les limites de ce profil
│   └── job-too-hot.gcode                        un programme qui les dépasse sur deux lignes
├── docs/
│   ├── REQUIREMENTS.md                          le registre des exigences : état, preuve et test de chacune
│   ├── SCOPE.md                                 à quoi il sert, ce qu'il ne fera pas et ce qu'il ne décide jamais
│   ├── STATUS.md                                ce qui existe, ce qui manque et ce qui reste ouvert
│   ├── ARCHITECTURE.md                          le programme crate par crate
│   ├── ROADMAP.md                               les neuf étapes de la base à un programme complet
│   ├── NOT_YET.md                               ce qui n'est pas encore fait et ce dont chaque partie a besoin
│   ├── SERVER.md                                comment le programme s'accorde avec le serveur EH3D
│   ├── architecture.json                        données de l'image d'Architecture
│   └── structure.json                           données de l'image de Structure
├── tools/                                       vérification de la CI et du README et outils d'images
├── images/                                      bannière, les deux schémas et les dessins des espaces de travail du README
├── .github/                                     workflows de CI et de publication, modèles de tickets et de pull requests
├── eh3d.project.json                            manifeste : version, rôle, maturité et commande de compilation
├── Cargo.toml                                   l'espace de travail et ses crates
├── rust-toolchain.toml                          la chaîne d'outils Rust épinglée
├── CHANGELOG.md                                 ce qui a changé dans chaque version
├── AGENTS.md                                    les règles pour qui modifie le dépôt
├── SECURITY.md SUPPORT.md                       comment signaler une vulnérabilité et où demander
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   comment contribuer, le code de conduite et la licence
```

</details>

## 🗺️ Feuille de route et limites connues

**Limites connues aujourd'hui**

* Le trancheur est petit : les supports sont simples (ni arbre, ni couches d'interface), les ponts ne sont pas affinés, il n'y a rien pour plusieurs matériaux, et aucune impression n'en est sortie sur une imprimante.
* Aucune imprimante n'a été connectée, pas même la Bambu Lab A1 ou la P1S ; chaque profil dit `verified = false`.
* Le lecteur 3MF ne prend que le maillage (les composants et l'extension de production sont refusés) ; les projets d'autres programmes sont lus pour leurs objets, plateaux et réglages et jamais modifiés.
* Les pilotes ont un transport HTTP simple, essayé seulement sur un socket de cet ordinateur et contre les simulateurs ; ni TLS ni push WebSocket ; Bambu LAN est une conception qui ne se connecte pas, et le cloud de Bambu n'en fait pas partie.
* La fenêtre n'a pas été compilée ni vue sur un écran.

**Ensuite**

* H1 : géométrie des maillages et un visualiseur 3D propre.
* H2 : un cube d'étalonnage imprimé sur une vraie machine (le remplissage en grille et en gyroïde, les coutures, la largeur variable, les ponts et les supports simples sont écrits et testés sur des formes à graine fixe).
* H3 : l'éditeur de profils et la fenêtre de base.
* H7 : les autres transports (push WebSocket, PrusaLink, Duet) et la liaison Bambu LAN.
* H8 : l'essayer sur une Bambu Lab A1 et une P1S et marquer comme vérifiés les profils qui fonctionnent. Tout le plan est dans `docs/ROADMAP.md`.

## 🌍 Langues

Le README et les textes existent en anglais, espagnol, français, italien, allemand, chinois, japonais et portugais européen.

## 🔗 Projets liés

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON): les crates partagés, dont le client commun du serveur
* [EH3D-SERVER](https://github.com/JuanenRac/EH3D-SERVER): le serveur de la suite, où doivent vivre les imprimantes, la file et les permissions
* [EH3D-STUDIO](https://github.com/JuanenRac/EH3D-STUDIO): le panneau de configuration de la suite
* [EH3D-PHOENIX-LASER](https://github.com/JuanenRac/EH3D-PHOENIX-LASER): gravure et découpe laser, le programme frère de la même famille
* [EH3D-PHOENIX-PNP](https://github.com/JuanenRac/EH3D-PHOENIX-PNP): pick and place, le programme frère de la même famille
* [EH3D-SUITE-LAUNCHER](https://github.com/JuanenRac/EH3D-SUITE-LAUNCHER): le hub qui trouve et lance les programmes de la suite

## 📚 Documentation

* [Journal des modifications](CHANGELOG.md)
* [Périmètre](docs/SCOPE.md): à quoi sert le programme, ce qu'il ne fera pas et ce qu'il ne décide jamais.
* [Exigences](docs/REQUIREMENTS.md): le registre des capacités, l'état de chacune et le test qui la montre.
* [État](docs/STATUS.md): ce qui existe, ce qui manque et ce qui reste ouvert.
* [Pas encore](docs/NOT_YET.md): de bonnes idées laissées de côté dans cette version.
* [Règles pour modifier le dépôt](AGENTS.md): les règles pour qui modifie le dépôt.
* [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): Architecture du programme, crate par crate
* [docs/ROADMAP.md](docs/ROADMAP.md): Feuille de route : les neuf étapes de la base à un programme complet
* [docs/NOT_YET.md](docs/NOT_YET.md): Ce qui n'est pas encore fait et ce dont chaque partie manquante a besoin
* [docs/SERVER.md](docs/SERVER.md): Comment le programme s'accorde avec le serveur EH3D
* [docs/architecture.json](docs/architecture.json): Données des deux images

## 🤝 Contribuer et communauté

* Avant une pull request, lancez `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` et `python -I tools/ci_validate.py` : tout doit passer. Voir [CONTRIBUTING.md](CONTRIBUTING.md) et le [code de conduite](CODE_OF_CONDUCT.md).
* Le README existe en huit langues : un changement de texte se fait dans les huit fichiers, et les deux images sont dessinées depuis `docs/architecture.json` et `docs/structure.json` par `tools/make_architecture.py` et `tools/make_structure.py`.
* Signalez une vulnérabilité en privé : [SECURITY.md](SECURITY.md). Questions, idées et rapports : electrohobby3d@gmail.com ([SUPPORT.md](SUPPORT.md)).

## 👤 AUTEUR

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 LICENCE

Logiciel : GPL-3.0-or-later. Voir [LICENSE](LICENSE). Les conceptions matérielles de la suite utilisent la CERN OHL-S.
