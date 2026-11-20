//! The texts of the program in the eight languages (English, Spanish, German, French, Italian, Japanese, Chinese and European Portuguese). A text with `{}` has the
//! value put in its place by `fill`. The codes of error and the keys of the settings are never translated.

use p3dp_link::JobState;

/// The languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Es,
    De,
    Fr,
    It,
    Ja,
    Zh,
    Pt,
}

impl Lang {
    pub const ALL: [Lang; 8] = [
        Lang::En,
        Lang::Es,
        Lang::De,
        Lang::Fr,
        Lang::It,
        Lang::Ja,
        Lang::Zh,
        Lang::Pt,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::It => "it",
            Lang::Ja => "ja",
            Lang::Zh => "zh",
            Lang::Pt => "pt",
        }
    }

    /// The name of the language in itself.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::Fr => "Français",
            Lang::It => "Italiano",
            Lang::Ja => "日本語",
            Lang::Zh => "中文",
            Lang::Pt => "Português",
        }
    }

    /// The language of a code such as `es`, `pt-PT` or `zh_CN` (anything after the first two letters is ignored).
    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().to_ascii_lowercase();
        let two: String = c.chars().take(2).collect();
        Lang::ALL.into_iter().find(|l| l.code() == two)
    }

    /// The language to start in: the code given, else the one of the environment variables, else English.
    pub fn detect(given: Option<&str>, env_value: Option<&str>) -> Lang {
        given
            .and_then(Lang::from_code)
            .or_else(|| env_value.and_then(Lang::from_code))
            .unwrap_or(Lang::En)
    }
}

/// The messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Msg {
    TabPrepare,
    TabPreview,
    TabDevice,
    TabProject,
    Printer,
    Model,
    Open,
    Save,
    Path,
    Triangles,
    Volume,
    Size,
    FitsYes,
    FitsNo,
    NotWatertight,
    Degenerate,
    Inverted,
    VerifiedNo,
    VerifiedYes,
    Limits,
    Nozzle,
    Bed,
    Chamber,
    NoChamber,
    SimulatorOnly,
    BambuExperimental,
    ModeServer,
    ModeStandalone,
    ModeNotConfigured,
    Language,
    State,
    Progress,
    NoModel,
    Loaded,
    Saved,
    Problem,
    Safety,
    PanelFilament,
    PanelInfill,
    PanelSupports,
    PanelSeam,
    PanelBridges,
    PanelLayers,
    PanelEstimate,
    PanelResin,
    PanelGcodeFile,
    PatternRectilinear,
    PatternGrid,
    PatternGyroid,
    SeamNearest,
    SeamAligned,
    SeamRear,
    SupportsOff,
    SupportsBed,
    SupportsEverywhere,
    BridgeTooLong,
    ZHop,
    FanRamp,
    WidthVariation,
    EstimateAbout,
    ResinExposure,
    ResinLayers,
    ClosedFormat,
    LimitsFromMaker,
    LimitsConservative,
    LimitsOwner,
    ViewOrbit,
    ViewPan,
    ViewZoom,
    ViewSelectBox,
    ViewLayerRange,
    ViewReset,
    ViewFit,
    FilterSearch,
    FilterTechnology,
    TechFdm,
    TechResin,
    FilterAll,
    FilamentMaterial,
    FilamentCompatible,
    PrintersShown,
    SliceNow,
    SliceSummary,
    KindPerimeters,
    KindInfill,
    KindSolid,
    KindAdhesion,
    LayerOnlyOne,
    LayerAll,
    ServerPanelTitle,
    QueueTitle,
    QueueEmpty,
    JobStart,
    JobConfirmStart,
    Confirm,
    CancelAction,
    JobPause,
    JobResume,
    JobCancel,
    EmergencyStop,
    Refresh,
    NoServerSet,
    MachinesTitle,
    ProjectObjects,
    ProjectPlates,
    ProjectMadeBy,
    ShortcutsTitle,
    SchemeFamily,
    SchemeOwn,
}

impl Msg {
    pub const ALL: [Msg; 109] = [
        Msg::TabPrepare,
        Msg::TabPreview,
        Msg::TabDevice,
        Msg::TabProject,
        Msg::Printer,
        Msg::Model,
        Msg::Open,
        Msg::Save,
        Msg::Path,
        Msg::Triangles,
        Msg::Volume,
        Msg::Size,
        Msg::FitsYes,
        Msg::FitsNo,
        Msg::NotWatertight,
        Msg::Degenerate,
        Msg::Inverted,
        Msg::VerifiedNo,
        Msg::VerifiedYes,
        Msg::Limits,
        Msg::Nozzle,
        Msg::Bed,
        Msg::Chamber,
        Msg::NoChamber,
        Msg::SimulatorOnly,
        Msg::BambuExperimental,
        Msg::ModeServer,
        Msg::ModeStandalone,
        Msg::ModeNotConfigured,
        Msg::Language,
        Msg::State,
        Msg::Progress,
        Msg::NoModel,
        Msg::Loaded,
        Msg::Saved,
        Msg::Problem,
        Msg::Safety,
        Msg::PanelFilament,
        Msg::PanelInfill,
        Msg::PanelSupports,
        Msg::PanelSeam,
        Msg::PanelBridges,
        Msg::PanelLayers,
        Msg::PanelEstimate,
        Msg::PanelResin,
        Msg::PanelGcodeFile,
        Msg::PatternRectilinear,
        Msg::PatternGrid,
        Msg::PatternGyroid,
        Msg::SeamNearest,
        Msg::SeamAligned,
        Msg::SeamRear,
        Msg::SupportsOff,
        Msg::SupportsBed,
        Msg::SupportsEverywhere,
        Msg::BridgeTooLong,
        Msg::ZHop,
        Msg::FanRamp,
        Msg::WidthVariation,
        Msg::EstimateAbout,
        Msg::ResinExposure,
        Msg::ResinLayers,
        Msg::ClosedFormat,
        Msg::LimitsFromMaker,
        Msg::LimitsConservative,
        Msg::LimitsOwner,
        Msg::ViewOrbit,
        Msg::ViewPan,
        Msg::ViewZoom,
        Msg::ViewSelectBox,
        Msg::ViewLayerRange,
        Msg::ViewReset,
        Msg::ViewFit,
        Msg::FilterSearch,
        Msg::FilterTechnology,
        Msg::TechFdm,
        Msg::TechResin,
        Msg::FilterAll,
        Msg::FilamentMaterial,
        Msg::FilamentCompatible,
        Msg::PrintersShown,
        Msg::SliceNow,
        Msg::SliceSummary,
        Msg::KindPerimeters,
        Msg::KindInfill,
        Msg::KindSolid,
        Msg::KindAdhesion,
        Msg::LayerOnlyOne,
        Msg::LayerAll,
        Msg::ServerPanelTitle,
        Msg::QueueTitle,
        Msg::QueueEmpty,
        Msg::JobStart,
        Msg::JobConfirmStart,
        Msg::Confirm,
        Msg::CancelAction,
        Msg::JobPause,
        Msg::JobResume,
        Msg::JobCancel,
        Msg::EmergencyStop,
        Msg::Refresh,
        Msg::NoServerSet,
        Msg::MachinesTitle,
        Msg::ProjectObjects,
        Msg::ProjectPlates,
        Msg::ProjectMadeBy,
        Msg::ShortcutsTitle,
        Msg::SchemeFamily,
        Msg::SchemeOwn,
    ];

    /// The texts in the order English, Spanish, German, French, Italian, Japanese, Chinese, Portuguese (Portugal).
    fn texts(self) -> [&'static str; 8] {
        match self {
            Msg::TabPrepare => ["Prepare", "Preparar", "Vorbereiten", "Préparer", "Prepara", "準備", "准备", "Preparar"],
            Msg::TabPreview => ["Preview", "Vista previa", "Vorschau", "Aperçu", "Anteprima", "プレビュー", "预览", "Pré-visualização"],
            Msg::TabDevice => ["Device", "Dispositivo", "Gerät", "Appareil", "Dispositivo", "デバイス", "设备", "Dispositivo"],
            Msg::TabProject => ["Project", "Proyecto", "Projekt", "Projet", "Progetto", "プロジェクト", "项目", "Projeto"],
            Msg::Printer => ["Printer", "Impresora", "Drucker", "Imprimante", "Stampante", "プリンター", "打印机", "Impressora"],
            Msg::Model => ["Model", "Modelo", "Modell", "Modèle", "Modello", "モデル", "模型", "Modelo"],
            Msg::Open => ["Open", "Abrir", "Öffnen", "Ouvrir", "Apri", "開く", "打开", "Abrir"],
            Msg::Save => ["Save", "Guardar", "Speichern", "Enregistrer", "Salva", "保存", "保存", "Guardar"],
            Msg::Path => ["File", "Archivo", "Datei", "Fichier", "File", "ファイル", "文件", "Ficheiro"],
            Msg::Triangles => ["Triangles: {}", "Triángulos: {}", "Dreiecke: {}", "Triangles : {}", "Triangoli: {}", "三角形: {}", "三角形: {}", "Triângulos: {}"],
            Msg::Volume => ["Volume: {} cm³", "Volumen: {} cm³", "Volumen: {} cm³", "Volume : {} cm³", "Volume: {} cm³", "体積: {} cm³", "体积: {} cm³", "Volume: {} cm³"],
            Msg::Size => ["Size: {} mm", "Tamaño: {} mm", "Größe: {} mm", "Taille : {} mm", "Dimensioni: {} mm", "サイズ: {} mm", "尺寸: {} mm", "Tamanho: {} mm"],
            Msg::FitsYes => ["It fits in the build volume", "Cabe en el volumen de impresión", "Es passt in den Bauraum", "Il tient dans le volume d'impression", "Entra nel volume di stampa", "造形範囲に収まります", "可放入打印空间", "Cabe no volume de impressão"],
            Msg::FitsNo => ["It does NOT fit in the build volume", "NO cabe en el volumen de impresión", "Es passt NICHT in den Bauraum", "Il ne tient PAS dans le volume d'impression", "NON entra nel volume di stampa", "造形範囲に収まりません", "无法放入打印空间", "NÃO cabe no volume de impressão"],
            Msg::NotWatertight => ["The mesh is not watertight ({} open edges)", "La malla no es estanca ({} aristas abiertas)", "Das Netz ist nicht wasserdicht ({} offene Kanten)", "Le maillage n'est pas étanche ({} arêtes ouvertes)", "La mesh non è chiusa ({} bordi aperti)", "メッシュが閉じていません（開いた辺 {}）", "网格不封闭（{} 条开放边）", "A malha não é estanque ({} arestas abertas)"],
            Msg::Degenerate => ["{} triangles have no area", "{} triángulos no tienen área", "{} Dreiecke haben keine Fläche", "{} triangles n'ont pas de surface", "{} triangoli non hanno area", "面積のない三角形が {} 個あります", "{} 个三角形没有面积", "{} triângulos não têm área"],
            Msg::Inverted => ["The triangles face inwards", "Los triángulos miran hacia dentro", "Die Dreiecke zeigen nach innen", "Les triangles sont tournés vers l'intérieur", "I triangoli sono rivolti verso l'interno", "三角形が内向きです", "三角形朝向内侧", "Os triângulos estão virados para dentro"],
            Msg::VerifiedNo => ["This profile has not been tried on a machine", "Este perfil no se ha probado en una máquina", "Dieses Profil wurde an keiner Maschine getestet", "Ce profil n'a pas été essayé sur une machine", "Questo profilo non è stato provato su una macchina", "このプロファイルは実機で試されていません", "此配置尚未在机器上试过", "Este perfil não foi testado numa máquina"],
            Msg::VerifiedYes => ["This profile has been tried on a machine", "Este perfil se ha probado en una máquina", "Dieses Profil wurde an einer Maschine getestet", "Ce profil a été essayé sur une machine", "Questo profilo è stato provato su una macchina", "このプロファイルは実機で試されています", "此配置已在机器上试过", "Este perfil foi testado numa máquina"],
            Msg::Limits => ["Temperature limits of the model", "Límites de temperatura del modelo", "Temperaturgrenzen des Modells", "Limites de température du modèle", "Limiti di temperatura del modello", "モデルの温度上限", "型号的温度上限", "Limites de temperatura do modelo"],
            Msg::Nozzle => ["Nozzle", "Boquilla", "Düse", "Buse", "Ugello", "ノズル", "喷嘴", "Bico"],
            Msg::Bed => ["Bed", "Cama", "Druckbett", "Plateau", "Piatto", "ベッド", "热床", "Mesa"],
            Msg::Chamber => ["Chamber", "Cámara", "Kammer", "Enceinte", "Camera", "チャンバー", "腔体", "Câmara"],
            Msg::NoChamber => ["no heated chamber", "sin cámara calefactada", "keine beheizte Kammer", "pas d'enceinte chauffée", "nessuna camera riscaldata", "加熱チャンバーなし", "无加热腔体", "sem câmara aquecida"],
            Msg::SimulatorOnly => ["Simulator only: nothing here has run on a real printer", "Solo simulador: nada de esto se ha ejecutado en una impresora real", "Nur Simulator: nichts davon lief auf einem echten Drucker", "Simulateur seulement : rien n'a tourné sur une vraie imprimante", "Solo simulatore: nulla è stato eseguito su una stampante reale", "シミュレーターのみ：実機では動かしていません", "仅模拟器：尚未在真实打印机上运行", "Apenas simulador: nada disto correu numa impressora real"],
            Msg::BambuExperimental => ["Bambu LAN is experimental and does not connect yet", "Bambu LAN es experimental y todavía no se conecta", "Bambu LAN ist experimentell und verbindet noch nicht", "Bambu LAN est expérimental et ne se connecte pas encore", "Bambu LAN è sperimentale e non si collega ancora", "Bambu LAN は実験的で、まだ接続できません", "Bambu LAN 为实验性功能，尚不能连接", "O Bambu LAN é experimental e ainda não liga"],
            Msg::ModeServer => ["Through the server: {}", "A través del servidor: {}", "Über den Server: {}", "Via le serveur : {}", "Tramite il server: {}", "サーバー経由: {}", "通过服务器: {}", "Através do servidor: {}"],
            Msg::ModeStandalone => ["Stand-alone: straight to a printer, no shared queue", "Autónomo: directo a una impresora, sin cola compartida", "Eigenständig: direkt zum Drucker, ohne gemeinsame Warteschlange", "Autonome : directement vers l'imprimante, sans file partagée", "Autonomo: direttamente alla stampante, senza coda condivisa", "スタンドアロン：プリンターへ直接、共有キューなし", "独立模式：直连打印机，无共享队列", "Autónomo: diretamente para a impressora, sem fila partilhada"],
            Msg::ModeNotConfigured => ["No server and no stand-alone mode: nothing is sent anywhere", "Sin servidor ni modo autónomo: no se envía nada a ningún sitio", "Kein Server und kein eigenständiger Modus: nichts wird gesendet", "Pas de serveur ni de mode autonome : rien n'est envoyé", "Nessun server e nessuna modalità autonoma: non viene inviato nulla", "サーバーもスタンドアロンも未設定：何も送信されません", "未配置服务器或独立模式：不会发送任何内容", "Sem servidor nem modo autónomo: nada é enviado"],
            Msg::Language => ["Language", "Idioma", "Sprache", "Langue", "Lingua", "言語", "语言", "Idioma"],
            Msg::State => ["State: {}", "Estado: {}", "Zustand: {}", "État : {}", "Stato: {}", "状態: {}", "状态: {}", "Estado: {}"],
            Msg::Progress => ["Progress: {} %", "Progreso: {} %", "Fortschritt: {} %", "Progression : {} %", "Avanzamento: {} %", "進捗: {} %", "进度: {} %", "Progresso: {} %"],
            Msg::NoModel => ["No model loaded", "Ningún modelo cargado", "Kein Modell geladen", "Aucun modèle chargé", "Nessun modello caricato", "モデルが読み込まれていません", "未加载模型", "Nenhum modelo carregado"],
            Msg::Loaded => ["Loaded {}", "Cargado {}", "{} geladen", "{} chargé", "Caricato {}", "{} を読み込みました", "已加载 {}", "Carregado {}"],
            Msg::Saved => ["Saved {}", "Guardado {}", "{} gespeichert", "{} enregistré", "Salvato {}", "{} を保存しました", "已保存 {}", "Guardado {}"],
            Msg::Problem => ["Problem: {}", "Problema: {}", "Problem: {}", "Problème : {}", "Problema: {}", "問題: {}", "问题: {}", "Problema: {}"],
            Msg::Safety => [
                "This program does not control a machine by itself and is not a safety system: check what it produces before anything is run",
                "Este programa no controla una máquina por sí solo y no es un sistema de seguridad: revisa lo que produce antes de ejecutar nada",
                "Dieses Programm steuert keine Maschine selbst und ist kein Sicherheitssystem: prüfe das Ergebnis, bevor etwas ausgeführt wird",
                "Ce programme ne commande pas une machine à lui seul et n'est pas un système de sécurité : vérifiez ce qu'il produit avant tout lancement",
                "Questo programma non controlla da solo una macchina e non è un sistema di sicurezza: controlla ciò che produce prima di eseguire qualsiasi cosa",
                "このプログラムは単独で機械を制御せず、安全装置でもありません。実行する前に出力を確認してください",
                "本程序不会单独控制机器，也不是安全系统：运行任何内容之前请先检查其输出",
                "Este programa não controla uma máquina sozinho e não é um sistema de segurança: verifique o que produz antes de executar seja o que for",
            ],
            Msg::PanelFilament => ["Filament", "Filamento", "Filament", "Filament", "Filamento", "フィラメント", "耗材", "Filamento"],
            Msg::PanelInfill => ["Infill", "Relleno", "Füllung", "Remplissage", "Riempimento", "インフィル", "填充", "Preenchimento"],
            Msg::PanelSupports => ["Supports", "Soportes", "Stützen", "Supports", "Supporti", "サポート", "支撑", "Suportes"],
            Msg::PanelSeam => ["Seam", "Costura", "Naht", "Couture", "Cucitura", "シーム", "接缝", "Costura"],
            Msg::PanelBridges => ["Bridges", "Puentes", "Brücken", "Ponts", "Ponti", "ブリッジ", "桥接", "Pontes"],
            Msg::PanelLayers => ["Layers", "Capas", "Schichten", "Couches", "Strati", "レイヤー", "层", "Camadas"],
            Msg::PanelEstimate => ["Time estimate", "Estimación del tiempo", "Zeitschätzung", "Estimation du temps", "Stima del tempo", "時間の見積もり", "时间估算", "Estimativa do tempo"],
            Msg::PanelResin => ["Resin", "Resina", "Harz", "Résine", "Resina", "レジン", "树脂", "Resina"],
            Msg::PanelGcodeFile => ["G-code file", "Archivo G-code", "G-Code-Datei", "Fichier G-code", "File G-code", "G-code ファイル", "G-code 文件", "Ficheiro G-code"],
            Msg::PatternRectilinear => ["Lines", "Líneas", "Linien", "Lignes", "Linee", "直線", "直线", "Linhas"],
            Msg::PatternGrid => ["Grid", "Rejilla", "Gitter", "Grille", "Griglia", "グリッド", "网格", "Grelha"],
            Msg::PatternGyroid => ["Gyroid", "Giroide", "Gyroid", "Gyroïde", "Giroide", "ジャイロイド", "螺旋体", "Giroide"],
            Msg::SeamNearest => ["Nearest", "Más cercana", "Nächste", "La plus proche", "Più vicina", "最寄り", "最近处", "Mais próxima"],
            Msg::SeamAligned => ["Aligned", "Alineada", "Ausgerichtet", "Alignée", "Allineata", "整列", "对齐", "Alinhada"],
            Msg::SeamRear => ["Rear", "Trasera", "Hinten", "Arrière", "Posteriore", "背面", "后方", "Traseira"],
            Msg::SupportsOff => ["No supports", "Sin soportes", "Keine Stützen", "Sans supports", "Senza supporti", "サポートなし", "无支撑", "Sem suportes"],
            Msg::SupportsBed => ["Only from the bed", "Solo desde la cama", "Nur vom Druckbett", "Seulement depuis le plateau", "Solo dal piano", "ベッドからのみ", "仅从热床", "Só a partir da base"],
            Msg::SupportsEverywhere => ["Everywhere", "En todas partes", "Überall", "Partout", "Ovunque", "どこでも", "所有位置", "Em todo o lado"],
            Msg::BridgeTooLong => ["A bridge of {} mm is longer than the limit", "Un puente de {} mm supera el límite", "Eine Brücke von {} mm ist länger als die Grenze", "Un pont de {} mm dépasse la limite", "Un ponte di {} mm supera il limite", "{} mm のブリッジは上限を超えています", "{} mm 的桥接超过了上限", "Uma ponte de {} mm excede o limite"],
            Msg::ZHop => ["Lift on travel", "Elevar al desplazarse", "Anheben beim Verfahren", "Lever en déplacement", "Solleva negli spostamenti", "移動時に持ち上げ", "移动时抬升", "Elevar nas deslocações"],
            Msg::FanRamp => ["Fan ramp", "Rampa del ventilador", "Lüfterrampe", "Rampe du ventilateur", "Rampa della ventola", "ファンの段階変化", "风扇渐变", "Rampa da ventoinha"],
            Msg::WidthVariation => ["Variable width", "Ancho variable", "Variable Breite", "Largeur variable", "Larghezza variabile", "可変幅", "可变线宽", "Largura variável"],
            Msg::EstimateAbout => ["About {}", "Aprox. {}", "Etwa {}", "Environ {}", "Circa {}", "約 {}", "约 {}", "Cerca de {}"],
            Msg::ResinExposure => ["Exposure: {} s ({} s at the bottom)", "Exposición: {} s ({} s en la base)", "Belichtung: {} s ({} s unten)", "Exposition : {} s ({} s en bas)", "Esposizione: {} s ({} s alla base)", "露光: {} 秒（底面 {} 秒）", "曝光：{} 秒（底层 {} 秒）", "Exposição: {} s ({} s na base)"],
            Msg::ResinLayers => ["{} layers of {} mm", "{} capas de {} mm", "{} Schichten zu {} mm", "{} couches de {} mm", "{} strati da {} mm", "{} 層（{} mm）", "{} 层，每层 {} mm", "{} camadas de {} mm"],
            Msg::ClosedFormat => ["This resin format is closed and is not read", "Este formato de resina es cerrado y no se lee", "Dieses Harzformat ist geschlossen und wird nicht gelesen", "Ce format de résine est fermé et n'est pas lu", "Questo formato per resina è chiuso e non viene letto", "このレジン形式は非公開のため読み込みません", "此树脂格式不公开，不予读取", "Este formato de resina é fechado e não é lido"],
            Msg::LimitsFromMaker => ["Limits stated by the maker", "Límites que indica el fabricante", "Vom Hersteller angegebene Grenzen", "Limites indiquées par le fabricant", "Limiti dichiarati dal produttore", "メーカー公表の上限", "制造商标明的上限", "Limites indicados pelo fabricante"],
            Msg::LimitsConservative => ["Conservative limits of the program (not the maker's figure)", "Límites prudentes del programa (no son los del fabricante)", "Vorsichtige Grenzen des Programms (nicht die des Herstellers)", "Limites prudentes du programme (pas celles du fabricant)", "Limiti prudenti del programma (non quelli del produttore)", "プログラムの安全側の上限（メーカー値ではありません）", "程序的保守上限（并非制造商数值）", "Limites prudentes do programa (não são as do fabricante)"],
            Msg::LimitsOwner => ["Limits written by the owner", "Límites escritos por el dueño", "Vom Besitzer festgelegte Grenzen", "Limites écrites par le propriétaire", "Limiti scritti dal proprietario", "所有者が設定した上限", "由所有者设定的上限", "Limites escritos pelo dono"],
            Msg::ViewOrbit => ["Orbit", "Orbitar", "Drehen", "Orbiter", "Orbita", "回転", "旋转", "Orbitar"],
            Msg::ViewPan => ["Pan", "Desplazar", "Verschieben", "Déplacer", "Sposta", "移動", "平移", "Deslocar"],
            Msg::ViewZoom => ["Zoom", "Zoom", "Zoom", "Zoom", "Zoom", "ズーム", "缩放", "Zoom"],
            Msg::ViewSelectBox => ["Selection box", "Caja de selección", "Auswahlrahmen", "Cadre de sélection", "Riquadro di selezione", "選択ボックス", "选择框", "Caixa de seleção"],
            Msg::ViewLayerRange => ["Layers {} to {} of {}", "Capas {} a {} de {}", "Schichten {} bis {} von {}", "Couches {} à {} sur {}", "Strati da {} a {} di {}", "レイヤー {}〜{}（全 {}）", "第 {} 至 {} 层，共 {} 层", "Camadas {} a {} de {}"],
            Msg::ViewReset => ["Reset view", "Restablecer vista", "Ansicht zurücksetzen", "Réinitialiser la vue", "Ripristina vista", "表示をリセット", "重置视图", "Repor vista"],
            Msg::ViewFit => ["Fit to bed", "Ajustar a la cama", "An Druckbett anpassen", "Ajuster au plateau", "Adatta al piano", "ベッドに合わせる", "适合热床", "Ajustar à base"],
            Msg::FilterSearch => ["Search", "Buscar", "Suchen", "Rechercher", "Cerca", "検索", "搜索", "Pesquisar"],
            Msg::FilterTechnology => ["Technology", "Tecnología", "Technik", "Technologie", "Tecnologia", "方式", "技术", "Tecnologia"],
            Msg::TechFdm => ["Filament (FDM)", "Filamento (FDM)", "Filament (FDM)", "Filament (FDM)", "Filamento (FDM)", "フィラメント（FDM）", "熔丝（FDM）", "Filamento (FDM)"],
            Msg::TechResin => ["Resin (mSLA)", "Resina (mSLA)", "Harz (mSLA)", "Résine (mSLA)", "Resina (mSLA)", "レジン（mSLA）", "树脂（mSLA）", "Resina (mSLA)"],
            Msg::FilterAll => ["Show all", "Mostrar todo", "Alle anzeigen", "Tout afficher", "Mostra tutto", "すべて表示", "显示全部", "Mostrar tudo"],
            Msg::FilamentMaterial => ["Material", "Material", "Material", "Matériau", "Materiale", "素材", "材料", "Material"],
            Msg::FilamentCompatible => ["Only those the printer can use", "Solo los que la impresora puede usar", "Nur die, die der Drucker verwenden kann", "Seulement ceux que l'imprimante peut utiliser", "Solo quelli che la stampante può usare", "プリンターで使えるものだけ", "仅显示打印机可用的", "Só os que a impressora pode usar"],
            Msg::PrintersShown => ["{} of {} printers", "{} de {} impresoras", "{} von {} Druckern", "{} imprimantes sur {}", "{} stampanti su {}", "プリンター {} / {} 台", "打印机 {} / {} 台", "{} de {} impressoras"],
            Msg::SliceNow => ["Slice", "Laminar", "Slicen", "Découper", "Elabora", "スライス", "切片", "Fatiar"],
            Msg::SliceSummary => ["{} layers, about {}", "{} capas, aprox. {}", "{} Schichten, etwa {}", "{} couches, environ {}", "{} strati, circa {}", "{} 層、約 {}", "{} 层，约 {}", "{} camadas, cerca de {}"],
            Msg::KindPerimeters => ["Perimeters", "Perímetros", "Umfänge", "Périmètres", "Perimetri", "外周", "外壁", "Perímetros"],
            Msg::KindInfill => ["Sparse infill", "Relleno parcial", "Teilfüllung", "Remplissage partiel", "Riempimento parziale", "まばらな充填", "稀疏填充", "Preenchimento parcial"],
            Msg::KindSolid => ["Solid skins", "Pieles sólidas", "Feste Deckflächen", "Peaux pleines", "Pelli piene", "ソリッド面", "实心表层", "Peles sólidas"],
            Msg::KindAdhesion => ["Skirt, brim and raft", "Falda, borde y balsa", "Skirt, Brim und Raft", "Jupe, bordure et radeau", "Skirt, brim e raft", "スカート・ブリム・ラフト", "裙边、帽檐和筏板", "Skirt, brim e raft"],
            Msg::LayerOnlyOne => ["Only this layer", "Solo esta capa", "Nur diese Schicht", "Seulement cette couche", "Solo questo strato", "このレイヤーのみ", "仅此层", "Só esta camada"],
            Msg::LayerAll => ["All layers", "Todas las capas", "Alle Schichten", "Toutes les couches", "Tutti gli strati", "すべてのレイヤー", "所有层", "Todas as camadas"],
            Msg::ServerPanelTitle => ["EH3D server", "Servidor EH3D", "EH3D-Server", "Serveur EH3D", "Server EH3D", "EH3D サーバー", "EH3D 服务器", "Servidor EH3D"],
            Msg::QueueTitle => ["Queue", "Cola", "Warteschlange", "File d'attente", "Coda", "キュー", "队列", "Fila"],
            Msg::QueueEmpty => ["No jobs in the queue", "No hay trabajos en la cola", "Keine Aufträge in der Warteschlange", "Aucun travail dans la file", "Nessun lavoro in coda", "キューにジョブはありません", "队列中没有任务", "Não há trabalhos na fila"],
            Msg::JobStart => ["Start", "Iniciar", "Starten", "Démarrer", "Avvia", "開始", "开始", "Iniciar"],
            Msg::JobConfirmStart => ["Start \"{}\" on {}? Nothing starts without this confirmation", "¿Iniciar «{}» en {}? Nada empieza sin esta confirmación", "»{}« auf {} starten? Ohne diese Bestätigung startet nichts", "Démarrer « {} » sur {} ? Rien ne démarre sans cette confirmation", "Avviare «{}» su {}? Nulla parte senza questa conferma", "「{}」を {} で開始しますか？この確認なしには何も始まりません", "开始任务“{}”（打印机 {}）吗？没有此确认不会开始任何任务", "Iniciar «{}» em {}? Nenhum trabalho arranca sem esta confirmação"],
            Msg::Confirm => ["Confirm", "Confirmar", "Bestätigen", "Confirmer", "Conferma", "確認", "确认", "Confirmar"],
            Msg::CancelAction => ["Cancel", "Cancelar", "Abbrechen", "Annuler", "Annulla", "キャンセル", "取消", "Cancelar"],
            Msg::JobPause => ["Pause", "Pausar", "Pausieren", "Pause", "Pausa", "一時停止", "暂停", "Pausar"],
            Msg::JobResume => ["Resume", "Reanudar", "Fortsetzen", "Reprendre", "Riprendi", "再開", "继续", "Retomar"],
            Msg::JobCancel => ["Cancel job", "Cancelar trabajo", "Auftrag abbrechen", "Annuler le travail", "Annulla lavoro", "ジョブを取り消す", "取消任务", "Cancelar trabalho"],
            Msg::EmergencyStop => ["EMERGENCY STOP", "PARADA DE EMERGENCIA", "NOT-HALT", "ARRÊT D'URGENCE", "ARRESTO DI EMERGENZA", "非常停止", "紧急停止", "PARAGEM DE EMERGÊNCIA"],
            Msg::Refresh => ["Refresh", "Actualizar", "Aktualisieren", "Actualiser", "Aggiorna", "更新", "刷新", "Atualizar"],
            Msg::NoServerSet => ["No server is set: the queue and the machines are not available", "No hay servidor configurado: la cola y las máquinas no están disponibles", "Kein Server eingestellt: Warteschlange und Maschinen sind nicht verfügbar", "Aucun serveur configuré : la file et les machines ne sont pas disponibles", "Nessun server impostato: coda e macchine non sono disponibili", "サーバー未設定：キューとマシンは利用できません", "未设置服务器：队列和设备不可用", "Sem servidor definido: a fila e as máquinas não estão disponíveis"],
            Msg::MachinesTitle => ["Machines on the server", "Máquinas del servidor", "Maschinen am Server", "Machines du serveur", "Macchine sul server", "サーバー上のマシン", "服务器上的设备", "Máquinas no servidor"],
            Msg::ProjectObjects => ["Objects: {}", "Objetos: {}", "Objekte: {}", "Objets : {}", "Oggetti: {}", "オブジェクト: {}", "对象: {}", "Objetos: {}"],
            Msg::ProjectPlates => ["Plates: {}", "Placas: {}", "Platten: {}", "Plateaux : {}", "Piatti: {}", "プレート: {}", "构建板: {}", "Placas: {}"],
            Msg::ProjectMadeBy => ["Made by: {}", "Creado con: {}", "Erstellt mit: {}", "Créé avec : {}", "Creato con: {}", "作成元: {}", "创建自: {}", "Criado com: {}"],
            Msg::ShortcutsTitle => ["Keyboard shortcuts", "Atajos de teclado", "Tastenkürzel", "Raccourcis clavier", "Scorciatoie da tastiera", "キーボードショートカット", "键盘快捷键", "Atalhos de teclado"],
            Msg::SchemeFamily => ["Family scheme", "Esquema de la familia", "Schema der Familie", "Schéma de la famille", "Schema della famiglia", "ファミリー共通の割り当て", "系列通用方案", "Esquema da família"],
            Msg::SchemeOwn => ["Own scheme", "Esquema propio", "Eigenes Schema", "Schéma propre", "Schema proprio", "独自の割り当て", "自定义方案", "Esquema próprio"],
        }
    }

    /// The text in a language.
    pub fn tr(self, lang: Lang) -> &'static str {
        let i = match lang {
            Lang::En => 0,
            Lang::Es => 1,
            Lang::De => 2,
            Lang::Fr => 3,
            Lang::It => 4,
            Lang::Ja => 5,
            Lang::Zh => 6,
            Lang::Pt => 7,
        };
        self.texts()[i]
    }
}

/// The name of a state of a printer in a language. The fifteen states are the ones every protocol is mapped onto (`p3dp_link::JobState`); the raw text a printer sends is never shown
/// as a state, because it is in the language of its firmware.
pub fn state_text(state: JobState, lang: Lang) -> &'static str {
    let texts: [&'static str; 8] = match state {
        JobState::Offline => [
            "Offline",
            "Sin conexión",
            "Offline",
            "Hors ligne",
            "Non in linea",
            "オフライン",
            "离线",
            "Sem ligação",
        ],
        JobState::Idle => [
            "Idle",
            "En reposo",
            "Bereit",
            "Au repos",
            "Inattiva",
            "待機中",
            "空闲",
            "Em repouso",
        ],
        JobState::Preparing => [
            "Preparing",
            "Preparando",
            "Wird vorbereitet",
            "Préparation",
            "In preparazione",
            "準備中",
            "准备中",
            "A preparar",
        ],
        JobState::Printing => [
            "Printing",
            "Imprimiendo",
            "Druckt",
            "Impression en cours",
            "In stampa",
            "印刷中",
            "打印中",
            "A imprimir",
        ],
        JobState::Pausing => [
            "Pausing",
            "Pausando",
            "Wird pausiert",
            "Mise en pause",
            "Messa in pausa",
            "一時停止しています",
            "正在暂停",
            "A pausar",
        ],
        JobState::Paused => [
            "Paused",
            "En pausa",
            "Pausiert",
            "En pause",
            "In pausa",
            "一時停止中",
            "已暂停",
            "Em pausa",
        ],
        JobState::Finishing => [
            "Finishing",
            "Terminando",
            "Wird abgeschlossen",
            "Finalisation",
            "Completamento",
            "終了処理中",
            "正在完成",
            "A concluir",
        ],
        JobState::Finished => [
            "Finished",
            "Terminado",
            "Fertig",
            "Terminé",
            "Completato",
            "完了",
            "已完成",
            "Concluída",
        ],
        JobState::Failed => [
            "Failed",
            "Fallido",
            "Fehlgeschlagen",
            "Échec",
            "Non riuscito",
            "失敗",
            "失败",
            "Falhou",
        ],
        JobState::Cancelling => [
            "Cancelling",
            "Cancelando",
            "Wird abgebrochen",
            "Annulation",
            "Annullamento",
            "キャンセルしています",
            "正在取消",
            "A cancelar",
        ],
        JobState::Cancelled => [
            "Cancelled",
            "Cancelado",
            "Abgebrochen",
            "Annulé",
            "Annullato",
            "キャンセル済み",
            "已取消",
            "Cancelado",
        ],
        JobState::Error => [
            "Error",
            "Error",
            "Fehler",
            "Erreur",
            "Errore",
            "エラー",
            "错误",
            "Erro",
        ],
        JobState::Emergency => [
            "Emergency stop",
            "Parada de emergencia",
            "Not-Halt",
            "Arrêt d'urgence",
            "Arresto di emergenza",
            "非常停止",
            "紧急停止",
            "Paragem de emergência",
        ],
        JobState::Updating => [
            "Updating",
            "Actualizando",
            "Wird aktualisiert",
            "Mise à jour",
            "Aggiornamento",
            "更新中",
            "正在更新",
            "A atualizar",
        ],
        JobState::Busy => [
            "Busy",
            "Ocupada",
            "Beschäftigt",
            "Occupée",
            "Occupata",
            "ビジー",
            "忙碌",
            "Ocupada",
        ],
    };
    let i = match lang {
        Lang::En => 0,
        Lang::Es => 1,
        Lang::De => 2,
        Lang::Fr => 3,
        Lang::It => 4,
        Lang::Ja => 5,
        Lang::Zh => 6,
        Lang::Pt => 7,
    };
    texts[i]
}

/// Puts the values in place of the `{}` of a text, in order. A value with `{}` in it is not read again.
pub fn fill(text: &str, values: &[&str]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut it = values.iter();
    let mut rest = text;
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        out.push_str(it.next().copied().unwrap_or(""));
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_has_all_eight_texts() {
        for m in Msg::ALL {
            for l in Lang::ALL {
                assert!(!m.tr(l).trim().is_empty(), "{m:?} in {}", l.code());
            }
        }
    }

    #[test]
    fn the_list_of_messages_is_complete_and_has_no_repeats() {
        let mut seen = Msg::ALL.to_vec();
        seen.sort_by_key(|m| format!("{m:?}"));
        seen.dedup();
        assert_eq!(seen.len(), Msg::ALL.len());
        assert_eq!(Msg::ALL.len(), 109);
    }

    #[test]
    fn every_language_has_the_same_places_for_values() {
        for m in Msg::ALL {
            let n = m.tr(Lang::En).matches("{}").count();
            for l in Lang::ALL {
                assert_eq!(m.tr(l).matches("{}").count(), n, "{m:?} in {}", l.code());
            }
        }
    }

    #[test]
    fn translations_are_really_different_from_english_where_they_should_be() {
        // a message left in English by mistake in the other languages would be the same text eight times
        for m in Msg::ALL {
            if matches!(
                m,
                Msg::Save
                    | Msg::Path
                    | Msg::Volume
                    | Msg::ViewZoom
                    | Msg::PanelFilament
                    | Msg::TechFdm
                    | Msg::FilamentMaterial
            ) {
                continue;
            }
            let en = m.tr(Lang::En);
            let same = Lang::ALL.iter().filter(|l| m.tr(**l) == en).count();
            assert!(same <= 2, "{m:?} is the same text in {same} languages");
        }
    }

    #[test]
    fn the_panels_and_the_controls_of_the_new_parts_have_their_eight_values_and_the_words_of_portugal(
    ) {
        let new = &Msg::ALL[37..73];
        assert_eq!(new.len(), 36);
        let mut values = 0;
        for m in new {
            for l in Lang::ALL {
                assert!(!m.tr(l).trim().is_empty(), "{m:?} in {}", l.code());
                values += 1;
            }
        }
        assert_eq!(values, 36 * 8);
        assert_eq!(Msg::SupportsEverywhere.tr(Lang::Pt), "Em todo o lado");
        assert_eq!(Msg::ViewSelectBox.tr(Lang::Pt), "Caixa de seleção");
        assert_eq!(Msg::PanelGcodeFile.tr(Lang::Pt), "Ficheiro G-code");
        assert!(Msg::ZHop.tr(Lang::Pt).contains("deslocações"));
        // the places for values: the layer range has three, the exposure and the size of a resin job two
        for l in Lang::ALL {
            assert_eq!(
                fill(Msg::ViewLayerRange.tr(l), &["1", "2", "3"])
                    .matches(|c: char| c.is_ascii_digit())
                    .count(),
                3
            );
            assert_eq!(
                fill(Msg::ResinExposure.tr(l), &["7", "8"])
                    .matches(['7', '8'])
                    .count(),
                2
            );
        }
    }

    #[test]
    fn the_texts_of_the_tabs_and_the_lists_have_their_eight_values_and_keep_the_order_of_their_places(
    ) {
        let new = &Msg::ALL[73..];
        assert_eq!(new.len(), 36);
        let mut values = 0;
        for m in new {
            for l in Lang::ALL {
                assert!(!m.tr(l).trim().is_empty(), "{m:?} in {}", l.code());
                values += 1;
            }
        }
        assert_eq!(values, 36 * 8);
        // the question about a job holds the job first and the machine second in every language, so that the values go to their places
        for l in Lang::ALL {
            let q = fill(Msg::JobConfirmStart.tr(l), &["JOB", "MACHINE"]);
            let (j, m) = (q.find("JOB"), q.find("MACHINE"));
            assert!(j.is_some() && m.is_some() && j < m, "{}: {q}", l.code());
        }
        assert_eq!(Msg::EmergencyStop.tr(Lang::Pt), "PARAGEM DE EMERGÊNCIA");
        assert_eq!(Msg::SliceNow.tr(Lang::Pt), "Fatiar");
        assert_eq!(Msg::Refresh.tr(Lang::Pt), "Atualizar");
        assert!(Msg::NoServerSet.tr(Lang::Pt).contains("definido"));
        // the emergency stop is shouted in the languages that write capitals
        for l in [Lang::En, Lang::Es, Lang::De, Lang::Fr, Lang::It, Lang::Pt] {
            let s = Msg::EmergencyStop.tr(l);
            assert_eq!(s, s.to_uppercase(), "{}", l.code());
        }
    }

    #[test]
    fn portuguese_is_the_one_of_portugal() {
        assert_eq!(Msg::Path.tr(Lang::Pt), "Ficheiro");
        assert!(Msg::VerifiedNo.tr(Lang::Pt).contains("testado"));
        assert!(!Msg::Path.tr(Lang::Pt).contains("Arquivo"));
        assert!(Msg::TabPreview.tr(Lang::Pt).contains("Pré-visualização"));
    }

    #[test]
    fn codes() {
        assert_eq!(Lang::from_code("pt-PT"), Some(Lang::Pt));
        assert_eq!(Lang::from_code("zh_CN.UTF-8"), Some(Lang::Zh));
        assert_eq!(Lang::from_code("ES"), Some(Lang::Es));
        assert_eq!(Lang::from_code("xx"), None);
        assert_eq!(Lang::from_code(""), None);
        assert_eq!(Lang::detect(Some("de"), Some("fr_FR")), Lang::De);
        assert_eq!(Lang::detect(None, Some("fr_FR.UTF-8")), Lang::Fr);
        assert_eq!(Lang::detect(Some("nonsense"), None), Lang::En);
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
            assert!(!l.native_name().is_empty());
        }
    }

    #[test]
    fn fill_replaces_in_order() {
        assert_eq!(fill("a {} b {}", &["1", "2"]), "a 1 b 2");
        assert_eq!(fill("a {} b {}", &["{}"]), "a {} b ");
        assert_eq!(fill("none", &["x"]), "none");
        assert_eq!(fill("{}", &[]), "");
    }
    #[test]
    fn every_state_of_a_printer_has_its_name_in_the_eight_languages() {
        let mut values = 0;
        for st in JobState::ALL {
            for l in Lang::ALL {
                assert!(
                    !state_text(st, l).trim().is_empty(),
                    "{st:?} in {}",
                    l.code()
                );
                values += 1;
            }
        }
        assert_eq!(values, 15 * 8);
        // two states never share a name in a language
        for l in Lang::ALL {
            let mut names: Vec<&str> = JobState::ALL.iter().map(|s| state_text(*s, l)).collect();
            names.sort_unstable();
            let before = names.len();
            names.dedup();
            assert_eq!(
                names.len(),
                before,
                "two states have the same name in {}",
                l.code()
            );
        }
        // a name left in English by mistake would be the same text in many languages
        for st in JobState::ALL {
            let en = state_text(st, Lang::En);
            let same = Lang::ALL
                .iter()
                .filter(|l| state_text(st, **l) == en)
                .count();
            assert!(same <= 4, "{st:?} is the same text in {same} languages");
        }
        assert_eq!(state_text(JobState::Printing, Lang::Pt), "A imprimir");
        assert_eq!(
            state_text(JobState::Emergency, Lang::Pt),
            "Paragem de emergência"
        );
        assert!(!state_text(JobState::Offline, Lang::Pt).contains("Desconectado"));
    }
}
