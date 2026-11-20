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
    ReportMaterial,
    ReportCost,
    ReportTime,
    ReportShortLayers,
    ReportSlowestLayers,
    ReportTravel,
    ReportResin,
    ReportNone,
    ThumbnailLine,
    NoticeLimitRaised,
    NoticeAboveMaker,
    NoticeVerificationCleared,
    ProgressAtLayer,
    ViewTravel,
    PanelProcess,
    PanelObjects,
    ObjectPlacement,
    ObjectRotation,
    ObjectScale,
    SizeMm,
    KeepProportions,
    AutoOrient,
    Arrange,
    Duplicate,
    Remove,
    ResetObject,
    ViewCorner,
    ViewTop,
    ViewBottom,
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
    WarnOutOfBed,
    WarnOverlap,
    WarnLeftOut,
    Slicing,
    SliceFinished,
    NoPlanYet,
    EstTime,
    EstMaterial,
    EstCost,
    EstLayers,
    ThemeName,
    ThemeDark,
    ThemeLight,
    HelpTitle,
    HintPrepare,
    HintPreview,
    HintDevice,
    ShowHints,
    PanelLimits,
    LimitNozzle,
    LimitBed,
    LimitChamber,
    ApplyLimits,
    BackToCatalog,
    LimitsMine,
    LimitsSaved,
    SimBanner,
    AutoStartOff,
    AutoStartOn,
    QueueRules,
    RuleRefused,
    RuleAllowed,
    LoadDemo,
    EmptyBedHint,
    BedSize,
    GridSpacing,
    ArrangeGap,
    ArrangeOnAdd,
    NoFilamentChosen,
    NothingSelected,
    PartsCount,
    StalePlan,
    SettingsTitle,
    NextTab,
    PreviousTab,
    LayerUp,
    LayerDown,
    SearchHint,
    OpenModelHint,
    SaveToLabel,
    OpenFromLabel,
    SavePathHint,
    OpenPathHint,
    EstimatesPending,
    ViewPeekInside,
    DeviceTemperatures,
    DevicePrintTitle,
    DeviceNoPrint,
    DeviceNoHeaters,
    DeviceReadAgo,
    DeviceTimeLeft,
    DevicePower,
    SettingsFileTitle,
    PanelStatus,
    SendToQueue,
    ExportGcode,
    ExportToLabel,
    ExportPathHint,
    Exported,
    NothingToExport,
    UpdatesViaUpdater,
    OpenTypePath,
    SaveTypePath,
    FilamentSearchHint,
    FilamentMaker,
    FilamentsShown,
    PfTitle,
    PfNoFilament,
    PfClear,
    PfNozzleTemp,
    PfBedTemp,
    PfFlow,
    PfDry,
    PfHardened,
    PfEnclosure,
    PfDirectDrive,
    NozzleHardened,
}

impl Msg {
    pub const ALL: [Msg; 229] = [
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
        Msg::ReportMaterial,
        Msg::ReportCost,
        Msg::ReportTime,
        Msg::ReportShortLayers,
        Msg::ReportSlowestLayers,
        Msg::ReportTravel,
        Msg::ReportResin,
        Msg::ReportNone,
        Msg::ThumbnailLine,
        Msg::NoticeLimitRaised,
        Msg::NoticeAboveMaker,
        Msg::NoticeVerificationCleared,
        Msg::ProgressAtLayer,
        Msg::ViewTravel,
        Msg::PanelProcess,
        Msg::PanelObjects,
        Msg::ObjectPlacement,
        Msg::ObjectRotation,
        Msg::ObjectScale,
        Msg::SizeMm,
        Msg::KeepProportions,
        Msg::AutoOrient,
        Msg::Arrange,
        Msg::Duplicate,
        Msg::Remove,
        Msg::ResetObject,
        Msg::ViewCorner,
        Msg::ViewTop,
        Msg::ViewBottom,
        Msg::ViewFront,
        Msg::ViewBack,
        Msg::ViewLeft,
        Msg::ViewRight,
        Msg::WarnOutOfBed,
        Msg::WarnOverlap,
        Msg::WarnLeftOut,
        Msg::Slicing,
        Msg::SliceFinished,
        Msg::NoPlanYet,
        Msg::EstTime,
        Msg::EstMaterial,
        Msg::EstCost,
        Msg::EstLayers,
        Msg::ThemeName,
        Msg::ThemeDark,
        Msg::ThemeLight,
        Msg::HelpTitle,
        Msg::HintPrepare,
        Msg::HintPreview,
        Msg::HintDevice,
        Msg::ShowHints,
        Msg::PanelLimits,
        Msg::LimitNozzle,
        Msg::LimitBed,
        Msg::LimitChamber,
        Msg::ApplyLimits,
        Msg::BackToCatalog,
        Msg::LimitsMine,
        Msg::LimitsSaved,
        Msg::SimBanner,
        Msg::AutoStartOff,
        Msg::AutoStartOn,
        Msg::QueueRules,
        Msg::RuleRefused,
        Msg::RuleAllowed,
        Msg::LoadDemo,
        Msg::EmptyBedHint,
        Msg::BedSize,
        Msg::GridSpacing,
        Msg::ArrangeGap,
        Msg::ArrangeOnAdd,
        Msg::NoFilamentChosen,
        Msg::NothingSelected,
        Msg::PartsCount,
        Msg::StalePlan,
        Msg::SettingsTitle,
        Msg::NextTab,
        Msg::PreviousTab,
        Msg::LayerUp,
        Msg::LayerDown,
        Msg::SearchHint,
        Msg::OpenModelHint,
        Msg::SaveToLabel,
        Msg::OpenFromLabel,
        Msg::SavePathHint,
        Msg::OpenPathHint,
        Msg::EstimatesPending,
        Msg::ViewPeekInside,
        Msg::DeviceTemperatures,
        Msg::DevicePrintTitle,
        Msg::DeviceNoPrint,
        Msg::DeviceNoHeaters,
        Msg::DeviceReadAgo,
        Msg::DeviceTimeLeft,
        Msg::DevicePower,
        Msg::SettingsFileTitle,
        Msg::PanelStatus,
        Msg::SendToQueue,
        Msg::ExportGcode,
        Msg::ExportToLabel,
        Msg::ExportPathHint,
        Msg::Exported,
        Msg::NothingToExport,
        Msg::UpdatesViaUpdater,
        Msg::OpenTypePath,
        Msg::SaveTypePath,
        Msg::FilamentSearchHint,
        Msg::FilamentMaker,
        Msg::FilamentsShown,
        Msg::PfTitle,
        Msg::PfNoFilament,
        Msg::PfClear,
        Msg::PfNozzleTemp,
        Msg::PfBedTemp,
        Msg::PfFlow,
        Msg::PfDry,
        Msg::PfHardened,
        Msg::PfEnclosure,
        Msg::PfDirectDrive,
        Msg::NozzleHardened,
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
            Msg::ReportMaterial => ["Material: {} m of filament, {} cm3, {} g", "Material: {} m de filamento, {} cm3, {} g", "Material: {} m Filament, {} cm3, {} g", "Matière : {} m de filament, {} cm3, {} g", "Materiale: {} m di filamento, {} cm3, {} g", "材料: フィラメント {} m、{} cm3、{} g", "耗材：耗丝 {} m，{} cm3，{} g", "Material: {} m de filamento, {} cm3, {} g"],
            Msg::ReportCost => ["Cost of the filament: {}", "Coste del filamento: {}", "Kosten des Filaments: {}", "Coût du filament : {}", "Costo del filamento: {}", "フィラメントの費用: {}", "耗材成本：{}", "Custo do filamento: {}"],
            Msg::ReportTime => ["Time: {} (an estimate, not the planner of the printer)", "Tiempo: {} (una estimación, no el planificador de la impresora)", "Zeit: {} (eine Schätzung, nicht die Bewegungsplanung des Druckers)", "Durée : {} (une estimation, pas le planificateur de l'imprimante)", "Tempo: {} (una stima, non il pianificatore della stampante)", "時間: {}（推定値であり、プリンターのプランナーとは異なります）", "时间：{}（估算值，并非打印机规划器的结果）", "Tempo: {} (uma estimativa, não o planeador da impressora)"],
            Msg::ReportShortLayers => ["Layers under {} s: {}", "Capas de menos de {} s: {}", "Schichten unter {} s: {}", "Couches de moins de {} s : {}", "Strati sotto {} s: {}", "{} 秒未満のレイヤー: {}", "不足 {} 秒的层：{}", "Camadas com menos de {} s: {}"],
            Msg::ReportSlowestLayers => ["Slowest layers: {}", "Capas más lentas: {}", "Langsamste Schichten: {}", "Couches les plus lentes : {}", "Strati più lenti: {}", "最も遅いレイヤー: {}", "最慢的层：{}", "Camadas mais lentas: {}"],
            Msg::ReportTravel => ["Travel without printing: {} m", "Desplazamientos sin imprimir: {} m", "Leerfahrten ohne Druck: {} m", "Déplacements sans imprimer : {} m", "Spostamenti senza stampare: {} m", "印刷しない移動: {} m", "不打印的空走：{} m", "Deslocações sem imprimir: {} m"],
            Msg::ReportResin => ["Resin: {} ml in the part, {} ml with the waste, {} g, cost {}", "Resina: {} ml en la pieza, {} ml con el desperdicio, {} g, coste {}", "Harz: {} ml im Teil, {} ml mit Verschnitt, {} g, Kosten {}", "Résine : {} ml dans la pièce, {} ml avec les pertes, {} g, coût {}", "Resina: {} ml nel pezzo, {} ml con lo scarto, {} g, costo {}", "レジン: 造形物 {} ml、ロス込み {} ml、{} g、費用 {}", "树脂：零件 {} ml，含损耗 {} ml，{} g，成本 {}", "Resina: {} ml na peça, {} ml com o desperdício, {} g, custo {}"],
            Msg::ReportNone => ["none", "ninguna", "keine", "aucune", "nessuno", "なし", "无", "nenhuma"],
            Msg::ThumbnailLine => ["Thumbnail {}: {} x {} {}, {} bytes", "Miniatura {}: {} x {} {}, {} bytes", "Vorschaubild {}: {} x {} {}, {} Bytes", "Vignette {} : {} x {} {}, {} octets", "Miniatura {}: {} x {} {}, {} byte", "サムネイル {}: {} x {} {}、{} バイト", "缩略图 {}：{} x {} {}，{} 字节", "Miniatura {}: {} x {} {}, {} bytes"],
            Msg::NoticeLimitRaised => ["{} raised from {} C to {} C", "{} subido de {} C a {} C", "{} von {} C auf {} C erhöht", "{} relevé de {} C à {} C", "{} alzato da {} C a {} C", "{} を {} C から {} C に引き上げました", "{} 已从 {} C 提高到 {} C", "{} aumentado de {} C para {} C"],
            Msg::NoticeAboveMaker => ["This is above what the maker states for the model: the decision is the owner's, and nothing has been tried at that temperature", "Esto supera lo que indica el fabricante para el modelo: la decisión es del propietario y nada se ha probado a esa temperatura", "Das liegt über der Herstellerangabe für das Modell: die Entscheidung liegt beim Besitzer, und bei dieser Temperatur wurde nichts erprobt", "C'est au-dessus de ce que le fabricant indique pour le modèle : la décision revient au propriétaire et rien n'a été essayé à cette température", "È sopra quanto dichiara il produttore per il modello: la decisione spetta al proprietario e nulla è stato provato a quella temperatura", "メーカーがこのモデルに示す値を超えています。判断は所有者に委ねられ、この温度では何も試されていません", "这高于制造商对该型号给出的数值：由所有者自行决定，且从未在该温度下试验过", "Isto está acima do que o fabricante indica para o modelo: a decisão é do proprietário e nada foi testado a essa temperatura"],
            Msg::NoticeVerificationCleared => ["An edited profile has not been tried: it says verified = off", "Un perfil editado no se ha probado: dice verified = off", "Ein bearbeitetes Profil wurde nicht erprobt: es steht auf verified = off", "Un profil modifié n'a pas été essayé : il indique verified = off", "Un profilo modificato non è stato provato: dice verified = off", "編集したプロファイルは試されていません: verified = off になります", "编辑后的配置从未试用过：verified = off", "Um perfil editado não foi testado: indica verified = off"],
            Msg::ProgressAtLayer => ["After this layer: {} gone, {} left ({} %)", "Tras esta capa: {} transcurridos, {} restantes ({} %)", "Nach dieser Schicht: {} vergangen, {} verbleibend ({} %)", "Après cette couche : {} écoulées, {} restantes ({} %)", "Dopo questo strato: {} trascorsi, {} rimanenti ({} %)", "このレイヤーの後: 経過 {}、残り {}（{} %）", "此层之后：已用 {}，剩余 {}（{} %）", "Depois desta camada: {} decorridos, {} em falta ({} %)"],
            Msg::ViewTravel => ["Travel moves", "Desplazamientos", "Leerfahrten", "Déplacements", "Spostamenti", "移動", "空走", "Deslocações"],
            Msg::PanelProcess => ["Process", "Proceso", "Prozess", "Procédé", "Processo", "プロセス", "工艺", "Processo"],
            Msg::PanelObjects => ["Objects", "Objetos", "Objekte", "Objets", "Oggetti", "オブジェクト", "对象", "Objetos"],
            Msg::ObjectPlacement => ["Placement", "Colocación", "Platzierung", "Placement", "Posizionamento", "配置", "位置", "Posicionamento"],
            Msg::ObjectRotation => ["Rotation", "Rotación", "Drehung", "Rotation", "Rotazione", "回転", "旋转", "Rotação"],
            Msg::ObjectScale => ["Scale", "Escala", "Skalierung", "Échelle", "Scala", "拡大縮小", "缩放", "Escala"],
            Msg::SizeMm => ["Size (mm)", "Tamaño (mm)", "Größe (mm)", "Taille (mm)", "Dimensione (mm)", "サイズ (mm)", "尺寸 (mm)", "Tamanho (mm)"],
            Msg::KeepProportions => ["Keep proportions", "Mantener proporciones", "Proportionen beibehalten", "Garder les proportions", "Mantieni le proporzioni", "比率を保つ", "保持比例", "Manter proporções"],
            Msg::AutoOrient => ["Auto-orient", "Orientar solo", "Automatisch ausrichten", "Orienter auto", "Orienta in automatico", "自動で向きを決める", "自动摆放方向", "Orientar automaticamente"],
            Msg::Arrange => ["Arrange", "Organizar", "Anordnen", "Disposer", "Disponi", "整列", "排列", "Organizar"],
            Msg::Duplicate => ["Duplicate", "Duplicar", "Duplizieren", "Dupliquer", "Duplica", "複製", "复制", "Duplicar"],
            Msg::Remove => ["Remove", "Quitar", "Entfernen", "Retirer", "Rimuovi", "削除", "移除", "Remover"],
            Msg::ResetObject => ["Reset turn and scale", "Restablecer giro y escala", "Drehung und Skalierung zurücksetzen", "Réinitialiser rotation et échelle", "Ripristina rotazione e scala", "回転と拡大縮小をリセット", "重置旋转和缩放", "Repor rotação e escala"],
            Msg::ViewCorner => ["Corner", "Esquina", "Ecke", "Coin", "Angolo", "斜め", "斜角", "Canto"],
            Msg::ViewTop => ["Top", "Arriba", "Oben", "Dessus", "Alto", "上", "顶部", "Topo"],
            Msg::ViewBottom => ["Bottom", "Abajo", "Unten", "Dessous", "Basso", "下", "底部", "Fundo"],
            Msg::ViewFront => ["Front", "Frente", "Vorne", "Face", "Fronte", "前", "正面", "Frente"],
            Msg::ViewBack => ["Back", "Atrás", "Hinten", "Arrière", "Retro", "後", "背面", "Trás"],
            Msg::ViewLeft => ["Left", "Izquierda", "Links", "Gauche", "Sinistra", "左", "左侧", "Esquerda"],
            Msg::ViewRight => ["Right", "Derecha", "Rechts", "Droite", "Destra", "右", "右侧", "Direita"],
            Msg::WarnOutOfBed => ["{} part(s) go past the edge of the bed", "{} pieza(s) sobresalen del borde de la cama", "{} Teil(e) ragen über den Rand des Druckbetts", "{} pièce(s) dépassent du bord du plateau", "{} pezzo/i oltre il bordo del piano", "{} 個の部品がベッドの端を越えています", "{} 个零件超出了热床边缘", "{} peça(s) passam o limite da mesa"],
            Msg::WarnOverlap => ["{} pair(s) of parts overlap", "{} par(es) de piezas se solapan", "{} Paar(e) von Teilen überlappen sich", "{} paire(s) de pièces se chevauchent", "{} coppia/e di pezzi si sovrappongono", "{} 組の部品が重なっています", "{} 对零件重叠", "{} par(es) de peças sobrepõem-se"],
            Msg::WarnLeftOut => ["{} part(s) do not fit on the bed", "{} pieza(s) no caben en la cama", "{} Teil(e) passen nicht auf das Druckbett", "{} pièce(s) ne tiennent pas sur le plateau", "{} pezzo/i non entrano sul piano", "{} 個の部品はベッドに収まりません", "{} 个零件放不进热床", "{} peça(s) não cabem na mesa"],
            Msg::Slicing => ["Slicing...", "Laminando...", "Slicing läuft ...", "Découpe en cours...", "Elaborazione...", "スライス中...", "切片中...", "A fatiar..."],
            Msg::SliceFinished => ["Sliced in {} s", "Laminado en {} s", "In {} s geslict", "Découpé en {} s", "Elaborato in {} s", "{} 秒でスライスしました", "切片用时 {} 秒", "Fatiado em {} s"],
            Msg::NoPlanYet => ["Slice the model to see its layers here", "Lamina el modelo para ver sus capas aquí", "Slicen Sie das Modell, um die Schichten hier zu sehen", "Découpez le modèle pour voir ses couches ici", "Elabora il modello per vedere qui i suoi strati", "モデルをスライスするとレイヤーがここに表示されます", "切片后可在此查看各层", "Fatie o modelo para ver aqui as camadas"],
            Msg::EstTime => ["Time", "Tiempo", "Zeit", "Durée", "Tempo", "時間", "时间", "Tempo"],
            Msg::EstMaterial => ["Filament used", "Filamento usado", "Filamentverbrauch", "Filament utilisé", "Filamento usato", "使用量", "耗材用量", "Filamento usado"],
            Msg::EstCost => ["Cost", "Coste", "Kosten", "Coût", "Costo", "費用", "成本", "Custo"],
            Msg::EstLayers => ["Layers", "Capas", "Schichten", "Couches", "Strati", "レイヤー", "层数", "Camadas"],
            Msg::ThemeName => ["Theme", "Tema", "Design", "Thème", "Tema", "テーマ", "主题", "Aspeto"],
            Msg::ThemeDark => ["Dark", "Oscuro", "Dunkel", "Sombre", "Scuro", "ダーク", "深色", "Escuro"],
            Msg::ThemeLight => ["Light", "Claro", "Hell", "Clair", "Chiaro", "ライト", "浅色", "Claro"],
            Msg::HelpTitle => ["Help", "Ayuda", "Hilfe", "Aide", "Aiuto", "ヘルプ", "帮助", "Ajuda"],
            Msg::HintPrepare => ["1. Choose the printer, the filament and the process at the left. 2. Open a model (STL, 3MF or OBJ) or load the demo. 3. Move, turn and scale it at the right, or use auto-orient and arrange. 4. Slice at the bottom.", "1. Elige impresora, filamento y proceso a la izquierda. 2. Abre un modelo (STL, 3MF u OBJ) o carga la demostración. 3. Muévelo, gíralo y escálalo a la derecha, o usa orientar solo y organizar. 4. Lamina abajo.", "1. Wählen Sie links Drucker, Filament und Prozess. 2. Öffnen Sie ein Modell (STL, 3MF oder OBJ) oder laden Sie die Demo. 3. Verschieben, drehen und skalieren Sie es rechts oder nutzen Sie automatisch ausrichten und anordnen. 4. Unten slicen.", "1. Choisissez l'imprimante, le filament et le procédé à gauche. 2. Ouvrez un modèle (STL, 3MF ou OBJ) ou chargez la démo. 3. Déplacez-le, tournez-le et mettez-le à l'échelle à droite, ou utilisez orienter auto et disposer. 4. Découpez en bas.", "1. Scegli stampante, filamento e processo a sinistra. 2. Apri un modello (STL, 3MF o OBJ) o carica la demo. 3. Spostalo, ruotalo e scalalo a destra, oppure usa orienta in automatico e disponi. 4. Elabora in basso.", "1. 左でプリンター、フィラメント、プロセスを選びます。2. モデル (STL、3MF、OBJ) を開くか、デモを読み込みます。3. 右で移動・回転・拡大縮小するか、自動で向きを決める／整列を使います。4. 下でスライスします。", "1. 在左侧选择打印机、耗材和工艺。2. 打开模型（STL、3MF 或 OBJ）或加载演示模型。3. 在右侧移动、旋转、缩放，或使用自动摆放方向和排列。4. 在底部切片。", "1. Escolha a impressora, o filamento e o processo à esquerda. 2. Abra um modelo (STL, 3MF ou OBJ) ou carregue a demonstração. 3. Mova-o, rode-o e dimensione-o à direita, ou use orientar automaticamente e organizar. 4. Fatie em baixo."],
            Msg::HintPreview => ["Drag the slider to walk through the layers. The colours tell the kinds of line; the estimates are at the bottom. Nothing here is sent to a printer.", "Arrastra el deslizador para recorrer las capas. Los colores indican el tipo de línea; las estimaciones están abajo. Nada de esto se envía a una impresora.", "Ziehen Sie den Regler, um die Schichten durchzugehen. Die Farben zeigen die Linienarten; die Schätzungen stehen unten. Nichts davon wird an einen Drucker gesendet.", "Faites glisser le curseur pour parcourir les couches. Les couleurs indiquent le type de ligne ; les estimations sont en bas. Rien n'est envoyé à une imprimante.", "Trascina il cursore per scorrere gli strati. I colori indicano il tipo di linea; le stime sono in basso. Nulla viene inviato a una stampante.", "スライダーを動かしてレイヤーを順に見ます。色は線の種類を表し、見積もりは下にあります。ここからプリンターには何も送られません。", "拖动滑块逐层查看。颜色表示线条类型，预估值在底部。此处不会向打印机发送任何内容。", "Arraste o cursor para percorrer as camadas. As cores indicam o tipo de linha; as estimativas estão em baixo. Nada daqui é enviado para uma impressora."],
            Msg::HintDevice => ["The machines come from the EH3D server. Everything here runs against a simulator: no job starts by itself, and a start always asks first.", "Las máquinas vienen del servidor EH3D. Todo aquí funciona contra un simulador: ningún trabajo arranca solo y un inicio siempre pregunta antes.", "Die Maschinen kommen vom EH3D-Server. Alles hier läuft gegen einen Simulator: kein Auftrag startet von selbst, und ein Start fragt immer vorher.", "Les machines viennent du serveur EH3D. Tout ici tourne sur un simulateur : aucune tâche ne démarre seule et un démarrage demande toujours confirmation.", "Le macchine arrivano dal server EH3D. Tutto qui funziona su un simulatore: nessun lavoro parte da solo e un avvio chiede sempre conferma.", "マシンは EH3D サーバーから取得します。ここはすべてシミュレーター上で動き、ジョブは自動では始まらず、開始の前に必ず確認します。", "设备来自 EH3D 服务器。此处全部在模拟器上运行：任务不会自动开始，开始前一定会先询问。", "As máquinas vêm do servidor EH3D. Tudo aqui corre contra um simulador: nenhum trabalho arranca sozinho e um arranque pergunta sempre primeiro."],
            Msg::ShowHints => ["Show hints", "Mostrar consejos", "Hinweise anzeigen", "Afficher les conseils", "Mostra suggerimenti", "ヒントを表示", "显示提示", "Mostrar sugestões"],
            Msg::PanelLimits => ["Temperature limits", "Límites de temperatura", "Temperaturgrenzen", "Limites de température", "Limiti di temperatura", "温度の上限", "温度上限", "Limites de temperatura"],
            Msg::LimitNozzle => ["Nozzle limit (C)", "Límite de boquilla (C)", "Düsengrenze (C)", "Limite de buse (C)", "Limite ugello (C)", "ノズル上限 (C)", "喷嘴上限 (C)", "Limite do bico (C)"],
            Msg::LimitBed => ["Bed limit (C)", "Límite de cama (C)", "Druckbettgrenze (C)", "Limite de plateau (C)", "Limite piano (C)", "ベッド上限 (C)", "热床上限 (C)", "Limite da mesa (C)"],
            Msg::LimitChamber => ["Chamber limit (C), 0 = none", "Límite de cámara (C), 0 = ninguna", "Kammergrenze (C), 0 = keine", "Limite de caisson (C), 0 = aucun", "Limite camera (C), 0 = nessuna", "チャンバー上限 (C)、0 = なし", "腔体上限 (C)，0 = 无", "Limite da câmara (C), 0 = nenhuma"],
            Msg::ApplyLimits => ["Apply", "Aplicar", "Anwenden", "Appliquer", "Applica", "適用", "应用", "Aplicar"],
            Msg::BackToCatalog => ["Back to the catalog's figures", "Volver a las cifras del catálogo", "Zurück zu den Werten des Katalogs", "Revenir aux valeurs du catalogue", "Torna ai valori del catalogo", "カタログの値に戻す", "恢复为目录数值", "Voltar aos valores do catálogo"],
            Msg::LimitsMine => ["These limits are yours, not the maker's", "Estos límites son tuyos, no del fabricante", "Diese Grenzen sind Ihre, nicht die des Herstellers", "Ces limites sont les vôtres, pas celles du fabricant", "Questi limiti sono tuoi, non del produttore", "この上限はメーカーの値ではなく、あなたの設定です", "这些上限是你自己设定的，不是制造商的", "Estes limites são seus, não do fabricante"],
            Msg::LimitsSaved => ["Limits saved", "Límites guardados", "Grenzen gespeichert", "Limites enregistrées", "Limiti salvati", "上限を保存しました", "上限已保存", "Limites guardados"],
            Msg::SimBanner => ["SIMULATOR ONLY: the printer plugin of the server is a simulator, so nothing here reaches a real printer", "SOLO SIMULADOR: el complemento de impresoras del servidor es un simulador, así que nada de esto llega a una impresora real", "NUR SIMULATOR: Das Drucker-Plugin des Servers ist ein Simulator, daher erreicht nichts davon einen echten Drucker", "SIMULATEUR SEULEMENT : le module d'imprimantes du serveur est un simulateur, donc rien ici n'atteint une vraie imprimante", "SOLO SIMULATORE: il plugin stampanti del server è un simulatore, quindi nulla qui raggiunge una stampante reale", "シミュレーターのみ：サーバーのプリンタープラグインはシミュレーターのため、ここから実機には何も届きません", "仅模拟器：服务器的打印机插件是模拟器，因此这里的任何操作都不会到达真实打印机", "APENAS SIMULADOR: o plugin de impressoras do servidor é um simulador, por isso nada daqui chega a uma impressora real"],
            Msg::AutoStartOff => ["Server auto-start: off", "Arranque automático del servidor: apagado", "Autostart des Servers: aus", "Démarrage auto du serveur : désactivé", "Avvio automatico del server: spento", "サーバーの自動開始：オフ", "服务器自动开始：关", "Arranque automático do servidor: desligado"],
            Msg::AutoStartOn => ["Server auto-start: ON (a policy of the server)", "Arranque automático del servidor: ACTIVO (política del servidor)", "Autostart des Servers: AN (Richtlinie des Servers)", "Démarrage auto du serveur : ACTIF (politique du serveur)", "Avvio automatico del server: ATTIVO (criterio del server)", "サーバーの自動開始：オン（サーバーのポリシー）", "服务器自动开始：开（服务器策略）", "Arranque automático do servidor: LIGADO (política do servidor)"],
            Msg::QueueRules => ["Queue rules: duplicates {}, at most {} waiting", "Reglas de la cola: duplicados {}, máximo {} en espera", "Regeln der Warteschlange: Duplikate {}, höchstens {} wartend", "Règles de la file : doublons {}, au plus {} en attente", "Regole della coda: duplicati {}, al massimo {} in attesa", "キューの規則: 重複 {}、待機は最大 {} 件", "队列规则：重复任务 {}，最多 {} 个等待", "Regras da fila: duplicados {}, no máximo {} em espera"],
            Msg::RuleRefused => ["refused", "rechazados", "abgelehnt", "refusés", "rifiutati", "拒否", "拒绝", "recusados"],
            Msg::RuleAllowed => ["allowed", "permitidos", "erlaubt", "autorisés", "consentiti", "許可", "允许", "permitidos"],
            Msg::LoadDemo => ["Load demo model", "Cargar modelo de demostración", "Demomodell laden", "Charger le modèle de démo", "Carica modello demo", "デモモデルを読み込む", "加载演示模型", "Carregar modelo de demonstração"],
            Msg::EmptyBedHint => ["Type the path of a model (STL, 3MF or OBJ) and press Open, or load the demo", "Escribe la ruta de un modelo (STL, 3MF u OBJ) y pulsa Abrir, o carga la demostración", "Geben Sie den Pfad eines Modells (STL, 3MF oder OBJ) ein und drücken Sie Öffnen, oder laden Sie die Demo", "Saisissez le chemin d'un modèle (STL, 3MF ou OBJ) et appuyez sur Ouvrir, ou chargez la démo", "Digita il percorso di un modello (STL, 3MF o OBJ) e premi Apri, oppure carica la demo", "モデル (STL、3MF、OBJ) のパスを入力して「開く」を押すか、デモを読み込みます", "输入模型（STL、3MF 或 OBJ）的路径并点击“打开”，或加载演示模型", "Escreva o caminho de um modelo (STL, 3MF ou OBJ) e prima Abrir, ou carregue a demonstração"],
            Msg::BedSize => ["Bed {} x {} x {} mm", "Cama {} x {} x {} mm", "Druckbett {} x {} x {} mm", "Plateau {} x {} x {} mm", "Piano {} x {} x {} mm", "ベッド {} x {} x {} mm", "热床 {} x {} x {} mm", "Mesa {} x {} x {} mm"],
            Msg::GridSpacing => ["Grid (mm)", "Rejilla (mm)", "Raster (mm)", "Grille (mm)", "Griglia (mm)", "グリッド (mm)", "网格 (mm)", "Grelha (mm)"],
            Msg::ArrangeGap => ["Gap between parts (mm)", "Separación entre piezas (mm)", "Abstand zwischen Teilen (mm)", "Espace entre pièces (mm)", "Distanza tra i pezzi (mm)", "部品の間隔 (mm)", "零件间距 (mm)", "Espaço entre peças (mm)"],
            Msg::ArrangeOnAdd => ["Arrange when adding", "Organizar al añadir", "Beim Hinzufügen anordnen", "Disposer à l'ajout", "Disponi all'aggiunta", "追加時に整列", "添加时自动排列", "Organizar ao adicionar"],
            Msg::NoFilamentChosen => ["No filament chosen: a generic PLA is assumed for the estimates", "Sin filamento elegido: se supone un PLA genérico para las estimaciones", "Kein Filament gewählt: für die Schätzungen wird ein generisches PLA angenommen", "Aucun filament choisi : un PLA générique sert aux estimations", "Nessun filamento scelto: per le stime si usa un PLA generico", "フィラメント未選択：見積もりには汎用 PLA を使います", "未选择耗材：预估按通用 PLA 计算", "Nenhum filamento escolhido: assume-se um PLA genérico nas estimativas"],
            Msg::NothingSelected => ["Nothing selected", "Nada seleccionado", "Nichts ausgewählt", "Rien de sélectionné", "Nulla di selezionato", "選択なし", "未选择", "Nada selecionado"],
            Msg::PartsCount => ["Parts: {}", "Piezas: {}", "Teile: {}", "Pièces : {}", "Pezzi: {}", "部品: {}", "零件：{}", "Peças: {}"],
            Msg::StalePlan => ["The bed changed after this slice: slice again", "La cama cambió después de este laminado: lamina de nuevo", "Das Druckbett wurde nach diesem Slicing geändert: erneut slicen", "Le plateau a changé après cette découpe : découpez à nouveau", "Il piano è cambiato dopo questa elaborazione: elabora di nuovo", "このスライスの後にベッドが変更されました。もう一度スライスしてください", "此次切片之后热床已更改：请重新切片", "A mesa mudou depois desta fatiagem: fatie de novo"],
            Msg::SettingsTitle => ["Window settings", "Ajustes de la ventana", "Fenstereinstellungen", "Réglages de la fenêtre", "Impostazioni della finestra", "ウィンドウの設定", "窗口设置", "Definições da janela"],
            Msg::NextTab => ["Next tab", "Pestaña siguiente", "Nächster Tab", "Onglet suivant", "Scheda successiva", "次のタブ", "下一个标签页", "Separador seguinte"],
            Msg::PreviousTab => ["Previous tab", "Pestaña anterior", "Vorheriger Tab", "Onglet précédent", "Scheda precedente", "前のタブ", "上一个标签页", "Separador anterior"],
            Msg::LayerUp => ["Layer up", "Capa arriba", "Schicht nach oben", "Couche au-dessus", "Strato su", "上のレイヤー", "上一层", "Camada acima"],
            Msg::LayerDown => ["Layer down", "Capa abajo", "Schicht nach unten", "Couche en dessous", "Strato giù", "下のレイヤー", "下一层", "Camada abaixo"],
            Msg::SearchHint => ["Part of a printer name", "Parte del nombre de la impresora", "Teil eines Druckernamens", "Partie du nom d'une imprimante", "Parte del nome di una stampante", "プリンター名の一部", "打印机名称的一部分", "Parte do nome de uma impressora"],
            Msg::OpenModelHint => ["Path of a model file (STL, OBJ or 3MF)", "Ruta de un fichero de modelo (STL, OBJ o 3MF)", "Pfad einer Modelldatei (STL, OBJ oder 3MF)", "Chemin d'un fichier de modèle (STL, OBJ ou 3MF)", "Percorso di un file di modello (STL, OBJ o 3MF)", "モデルファイルのパス(STL、OBJ、3MF)", "模型文件路径(STL、OBJ 或 3MF)", "Caminho de um ficheiro de modelo (STL, OBJ ou 3MF)"],
            Msg::SaveToLabel => ["Save the project to", "Guardar el proyecto en", "Projekt speichern unter", "Enregistrer le projet dans", "Salva il progetto in", "プロジェクトの保存先", "项目保存到", "Guardar o projeto em"],
            Msg::OpenFromLabel => ["Open a project from", "Abrir un proyecto desde", "Projekt öffnen aus", "Ouvrir un projet depuis", "Apri un progetto da", "プロジェクトを開く場所", "从此处打开项目", "Abrir um projeto de"],
            Msg::SavePathHint => ["Name of the file to write, as project.3mf", "Nombre del fichero que se escribe, como proyecto.3mf", "Name der zu schreibenden Datei, etwa projekt.3mf", "Nom du fichier à écrire, comme projet.3mf", "Nome del file da scrivere, come progetto.3mf", "書き込むファイル名(例: project.3mf)", "要写入的文件名,例如 project.3mf", "Nome do ficheiro a escrever, como projeto.3mf"],
            Msg::OpenPathHint => ["Path of a project file to read", "Ruta de un fichero de proyecto que leer", "Pfad einer Projektdatei zum Lesen", "Chemin d'un fichier de projet à lire", "Percorso di un file di progetto da leggere", "読み込むプロジェクトファイルのパス", "要读取的项目文件路径", "Caminho de um ficheiro de projeto a ler"],
            Msg::EstimatesPending => ["Slice to see the time and the material", "Lamina para ver el tiempo y el material", "Slicen Sie, um Zeit und Material zu sehen", "Découpez pour voir la durée et la matière", "Elabora per vedere tempo e materiale", "スライスすると時間と材料が表示されます", "切片后可查看时间与耗材", "Fatie para ver o tempo e o material"],
            Msg::ViewPeekInside => ["See inside (hide the skins)", "Ver el interior (ocultar las cubiertas)", "Innen ansehen (Außenhaut ausblenden)", "Voir l'intérieur (masquer les peaux)", "Vedi l'interno (nascondi le pelli)", "内部を見る(外殻を隠す)", "查看内部(隐藏表层)", "Ver o interior (ocultar as coberturas)"],
            Msg::DeviceTemperatures => ["Temperatures", "Temperaturas", "Temperaturen", "Températures", "Temperature", "温度", "温度", "Temperaturas"],
            Msg::DevicePrintTitle => ["Print", "Impresión", "Druck", "Impression", "Stampa", "印刷", "打印", "Impressão"],
            Msg::DeviceNoPrint => ["Nothing is printing now", "No se está imprimiendo nada", "Es wird gerade nichts gedruckt", "Aucune impression en cours", "Nessuna stampa in corso", "現在印刷中のジョブはありません", "当前没有在打印", "Não está a imprimir nada neste momento"],
            Msg::DeviceNoHeaters => ["No temperatures reported (the machine may be offline)", "No hay temperaturas (la máquina puede estar sin conexión)", "Keine Temperaturen gemeldet (die Maschine ist evtl. offline)", "Aucune température reçue (la machine est peut-être hors ligne)", "Nessuna temperatura ricevuta (la macchina potrebbe essere offline)", "温度の報告がありません(マシンがオフラインの可能性があります)", "未收到温度数据(设备可能已离线)", "Sem temperaturas (a máquina pode estar sem ligação)"],
            Msg::DeviceReadAgo => ["Last reading: {} ago", "Última lectura: hace {}", "Letzte Messung: vor {}", "Dernière lecture : il y a {}", "Ultima lettura: {} fa", "最終読み取り: {}前", "上次读取:{}前", "Última leitura: há {}"],
            Msg::DeviceTimeLeft => ["Time left: {}", "Tiempo restante: {}", "Verbleibende Zeit: {}", "Temps restant : {}", "Tempo rimanente: {}", "残り時間: {}", "剩余时间:{}", "Tempo restante: {}"],
            Msg::DevicePower => ["Power: {} %", "Potencia: {} %", "Leistung: {} %", "Puissance : {} %", "Potenza: {} %", "出力: {} %", "功率:{} %", "Potência: {} %"],
            Msg::SettingsFileTitle => ["Settings file (read only)", "Fichero de ajustes (solo lectura)", "Einstellungsdatei (nur lesen)", "Fichier de réglages (lecture seule)", "File delle impostazioni (sola lettura)", "設定ファイル(読み取り専用)", "设置文件(只读)", "Ficheiro de definições (só de leitura)"],
            Msg::PanelStatus => ["Machine status", "Estado de la máquina", "Maschinenstatus", "État de la machine", "Stato della macchina", "マシンの状態", "设备状态", "Estado da máquina"],
            Msg::SendToQueue => ["Send to the queue", "Enviar a la cola", "An die Warteschlange senden", "Envoyer à la file d'attente", "Invia alla coda", "キューに送る", "发送到队列", "Enviar para a fila"],
            Msg::ExportGcode => ["Export G-code", "Exportar G-code", "G-Code exportieren", "Exporter le G-code", "Esporta il G-code", "Gコードを書き出す", "导出 G 代码", "Exportar o G-code"],
            Msg::ExportToLabel => ["Export to", "Exportar a", "Exportieren nach", "Exporter vers", "Esporta in", "書き出し先", "导出到", "Exportar para"],
            Msg::ExportPathHint => ["Name of the .gcode file to write", "Nombre del fichero .gcode que se escribe", "Name der zu schreibenden .gcode-Datei", "Nom du fichier .gcode à écrire", "Nome del file .gcode da scrivere", "書き込む .gcode ファイル名", "要写入的 .gcode 文件名", "Nome do ficheiro .gcode a escrever"],
            Msg::Exported => ["G-code written to {}", "G-code escrito en {}", "G-Code nach {} geschrieben", "G-code écrit dans {}", "G-code scritto in {}", "G コードを {} に書き込みました", "G 代码已写入 {}", "G-code escrito em {}"],
            Msg::NothingToExport => ["Slice first: there is no G-code to write yet", "Lamine primero: todavía no hay G-code que escribir", "Zuerst slicen: Es gibt noch keinen G-Code zum Schreiben", "Découpez d'abord : il n'y a pas encore de G-code à écrire", "Elabora prima: non c'è ancora G-code da scrivere", "先にスライスしてください。書き出す G コードがまだありません", "请先切片:还没有可写入的 G 代码", "Fatie primeiro: ainda não há G-code para escrever"],
            Msg::UpdatesViaUpdater => ["Updates are looked for with the Electro Hobby 3D updater: open it to see if there is a newer version", "Las actualizaciones se buscan con el actualizador de Electro Hobby 3D: ábralo para ver si hay una versión más nueva", "Aktualisierungen sucht der Electro-Hobby-3D-Updater: Öffnen Sie ihn, um zu sehen, ob es eine neuere Version gibt", "Les mises à jour se cherchent avec l'outil de mise à jour d'Electro Hobby 3D : ouvrez-le pour voir s'il existe une version plus récente", "Gli aggiornamenti si cercano con l'updater di Electro Hobby 3D: aprilo per vedere se c'è una versione più recente", "更新は Electro Hobby 3D のアップデーターで確認します。開いて新しいバージョンがあるか確かめてください", "更新通过 Electro Hobby 3D 更新程序查找:打开它即可查看是否有新版本", "As atualizações procuram-se com o atualizador da Electro Hobby 3D: abra-o para ver se há uma versão mais recente"],
            Msg::OpenTypePath => ["Type the path of a model in the Objects panel and press Open", "Escriba la ruta de un modelo en el panel de objetos y pulse Abrir", "Geben Sie im Objektbereich den Pfad eines Modells ein und drücken Sie Öffnen", "Saisissez le chemin d'un modèle dans le panneau des objets, puis appuyez sur Ouvrir", "Digita il percorso di un modello nel pannello degli oggetti e premi Apri", "オブジェクトパネルにモデルのパスを入力して「開く」を押してください", "请在对象面板中输入模型路径,然后点击“打开”", "Escreva o caminho de um modelo no painel de objetos e prima Abrir"],
            Msg::SaveTypePath => ["Type the name of the project in the Project panel and press Save", "Escriba el nombre del proyecto en el panel de proyecto y pulse Guardar", "Geben Sie im Projektbereich den Namen des Projekts ein und drücken Sie Speichern", "Saisissez le nom du projet dans le panneau du projet, puis appuyez sur Enregistrer", "Digita il nome del progetto nel pannello del progetto e premi Salva", "プロジェクトパネルにプロジェクト名を入力して「保存」を押してください", "请在项目面板中输入项目名称,然后点击“保存”", "Escreva o nome do projeto no painel do projeto e prima Guardar"],
            Msg::FilamentSearchHint => ["Maker, material or name", "Fabricante, material o nombre", "Hersteller, Material oder Name", "Fabricant, matière ou nom", "Produttore, materiale o nome", "メーカー、素材、名前", "品牌、材料或名称", "Fabricante, material ou nome"],
            Msg::FilamentMaker => ["Maker", "Fabricante", "Hersteller", "Fabricant", "Produttore", "メーカー", "品牌", "Fabricante"],
            Msg::FilamentsShown => ["{} of {} filaments", "{} de {} filamentos", "{} von {} Filamenten", "{} filaments sur {}", "{} filamenti su {}", "{} / {} 件のフィラメント", "{} / {} 种耗材", "{} de {} filamentos"],
            Msg::PfTitle => ["Before printing", "Antes de imprimir", "Vor dem Drucken", "Avant d'imprimer", "Prima di stampare", "印刷の前に", "打印前", "Antes de imprimir"],
            Msg::PfNoFilament => ["Choose a filament to check it before printing", "Elija un filamento para comprobarlo antes de imprimir", "Wählen Sie ein Filament, um es vor dem Drucken zu prüfen", "Choisissez un filament pour le vérifier avant d'imprimer", "Scegli un filamento per controllarlo prima di stampare", "印刷前に確認するフィラメントを選んでください", "请选择一种耗材,以便在打印前检查", "Escolha um filamento para o verificar antes de imprimir"],
            Msg::PfClear => ["Nothing to warn about for this filament", "Nada que advertir para este filamento", "Für dieses Filament gibt es keine Warnung", "Rien à signaler pour ce filament", "Niente da segnalare per questo filamento", "このフィラメントについて警告はありません", "这种耗材没有需要提醒的事项", "Nada a avisar para este filamento"],
            Msg::PfNozzleTemp => ["The nozzle is set to {} C, outside the {} to {} C of this filament", "La boquilla está a {} C, fuera de los {} a {} C de este filamento", "Die Düse ist auf {} C eingestellt, außerhalb der {} bis {} C dieses Filaments", "La buse est réglée à {} C, hors des {} à {} C de ce filament", "L'ugello è a {} C, fuori dai {} - {} C di questo filamento", "ノズルは {} C に設定されていますが、このフィラメントの {} ~ {} C の範囲外です", "喷嘴设为 {} C,超出此耗材的 {} 至 {} C 范围", "O bico está a {} C, fora dos {} a {} C deste filamento"],
            Msg::PfBedTemp => ["The bed is set to {} C, outside the {} to {} C of this filament", "La cama está a {} C, fuera de los {} a {} C de este filamento", "Das Bett ist auf {} C eingestellt, außerhalb der {} bis {} C dieses Filaments", "Le plateau est réglé à {} C, hors des {} à {} C de ce filament", "Il piano è a {} C, fuori dai {} - {} C di questo filamento", "ベッドは {} C に設定されていますが、このフィラメントの {} ~ {} C の範囲外です", "热床设为 {} C,超出此耗材的 {} 至 {} C 范围", "A mesa está a {} C, fora dos {} a {} C deste filamento"],
            Msg::PfFlow => ["The flow of {} mm3/s is over the {} mm3/s this filament takes: slow the print down", "El caudal de {} mm3/s supera los {} mm3/s que admite este filamento: reduzca la velocidad", "Der Durchfluss von {} mm3/s liegt über den {} mm3/s dieses Filaments: Drucken Sie langsamer", "Le débit de {} mm3/s dépasse les {} mm3/s que ce filament supporte : ralentissez l'impression", "Il flusso di {} mm3/s supera i {} mm3/s di questo filamento: rallenta la stampa", "流量 {} mm3/s がこのフィラメントの上限 {} mm3/s を超えています。速度を下げてください", "流量 {} mm3/s 超过此耗材可承受的 {} mm3/s:请降低打印速度", "O caudal de {} mm3/s excede os {} mm3/s que este filamento aceita: reduza a velocidade"],
            Msg::PfDry => ["Dry the filament before printing: {} C for {} h", "Seque el filamento antes de imprimir: {} C durante {} h", "Trocknen Sie das Filament vor dem Drucken: {} C für {} h", "Séchez le filament avant d'imprimer : {} C pendant {} h", "Asciuga il filamento prima di stampare: {} C per {} h", "印刷前にフィラメントを乾燥してください: {} C で {} 時間", "打印前请先烘干耗材:{} C,{} 小时", "Seque o filamento antes de imprimir: {} C durante {} h"],
            Msg::PfHardened => ["This filament wears a brass nozzle: use a hardened one", "Este filamento desgasta una boquilla de latón: use una endurecida", "Dieses Filament verschleißt eine Messingdüse: Verwenden Sie eine gehärtete", "Ce filament use une buse en laiton : utilisez une buse durcie", "Questo filamento consuma un ugello in ottone: usa uno temprato", "このフィラメントは真鍮ノズルを摩耗させます。硬化ノズルを使ってください", "此耗材会磨损黄铜喷嘴:请使用硬化喷嘴", "Este filamento desgasta um bico de latão: use um endurecido"],
            Msg::PfEnclosure => ["A closed printer is advised for this filament", "Se aconseja una impresora cerrada para este filamento", "Für dieses Filament wird ein geschlossener Drucker empfohlen", "Une imprimante fermée est conseillée pour ce filament", "Per questo filamento è consigliata una stampante chiusa", "このフィラメントには密閉型プリンターをお勧めします", "此耗材建议使用封闭式打印机", "Aconselha-se uma impressora fechada para este filamento"],
            Msg::PfDirectDrive => ["A direct drive extruder is advised for this filament", "Se aconseja un extrusor de accionamiento directo para este filamento", "Für dieses Filament wird ein Direktantrieb empfohlen", "Une extrusion directe est conseillée pour ce filament", "Per questo filamento è consigliato un estrusore diretto", "このフィラメントにはダイレクトドライブのエクストルーダーをお勧めします", "此耗材建议使用直驱挤出机", "Aconselha-se um extrusor de acionamento direto para este filamento"],
            Msg::NozzleHardened => ["Hardened nozzle", "Boquilla endurecida", "Gehärtete Düse", "Buse durcie", "Ugello temprato", "硬化ノズル", "硬化喷嘴", "Bico endurecido"],
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
        assert_eq!(Msg::ALL.len(), 229);
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
        let new = &Msg::ALL[73..109];
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
    fn the_words_of_the_report_the_thumbnails_and_the_edits_keep_the_order_of_their_places() {
        let new = &Msg::ALL[109..121];
        assert_eq!(new.len(), 12);
        for m in new {
            for l in Lang::ALL {
                assert!(!m.tr(l).trim().is_empty(), "{m:?} in {}", l.code());
            }
        }
        // the places are filled in order: the values of the material keep their order in every language
        for l in Lang::ALL {
            let s = fill(Msg::ReportMaterial.tr(l), &["M1", "V2", "G3"]);
            let at = |x: &str| s.find(x).unwrap_or_else(|| panic!("{}: {s}", l.code()));
            assert!(
                at("M1") < at("V2") && at("V2") < at("G3"),
                "{}: {s}",
                l.code()
            );
            let t = fill(Msg::ThumbnailLine.tr(l), &["N1", "W2", "H3", "F4", "B5"]);
            let order: Vec<usize> = ["N1", "W2", "H3", "F4", "B5"]
                .iter()
                .map(|x| t.find(x).unwrap())
                .collect();
            assert!(order.windows(2).all(|w| w[0] < w[1]), "{}: {t}", l.code());
            let r = fill(Msg::NoticeLimitRaised.tr(l), &["KEY", "FROM", "TO"]);
            assert!(
                r.contains("KEY") && r.find("FROM") < r.find("TO"),
                "{}: {r}",
                l.code()
            );
        }
        // portuguese of Portugal, and the plain word for an empty list
        assert!(Msg::ReportTime.tr(Lang::Pt).contains("planeador"));
        assert!(Msg::ReportTravel.tr(Lang::Pt).contains("Deslocações"));
        assert!(Msg::NoticeAboveMaker.tr(Lang::Pt).contains("proprietário"));
        assert_eq!(Msg::ReportNone.tr(Lang::En), "none");
        // every notice about a profile names the key it is about, so a person can find the line
        assert!(Msg::NoticeVerificationCleared
            .tr(Lang::De)
            .contains("verified"));
    }

    #[test]
    fn the_texts_of_the_studio_window_have_their_places_in_every_language_and_the_words_of_portugal(
    ) {
        let new = &Msg::ALL[123..205];
        assert_eq!(new.len(), 82);
        let places = |m: Msg| match m {
            Msg::WarnOutOfBed
            | Msg::WarnOverlap
            | Msg::WarnLeftOut
            | Msg::SliceFinished
            | Msg::PartsCount => 1,
            Msg::DeviceReadAgo | Msg::DeviceTimeLeft | Msg::DevicePower => 1,
            Msg::QueueRules => 2,
            Msg::BedSize => 3,
            _ => 0,
        };
        for m in new {
            for l in Lang::ALL {
                assert_eq!(
                    m.tr(l).matches("{}").count(),
                    places(*m),
                    "{m:?} in {}",
                    l.code()
                );
            }
        }
        for l in Lang::ALL {
            let s = fill(Msg::BedSize.tr(l), &["B1", "B2", "B3"]);
            let order: Vec<usize> = ["B1", "B2", "B3"]
                .iter()
                .map(|x| s.find(x).unwrap())
                .collect();
            assert!(order.windows(2).all(|w| w[0] < w[1]), "{}: {s}", l.code());
        }
        // the Portuguese of Portugal, not the Brazilian one
        for m in new {
            let t = m.tr(Lang::Pt);
            for brazilian in ["você", "arquivo", "tela", "mouse", "usuário", "baixar"] {
                assert!(!t.to_lowercase().contains(brazilian), "{m:?}: {t}");
            }
        }
        assert_eq!(Msg::ViewRight.tr(Lang::Pt), "Direita");
        assert!(Msg::HintPreview.tr(Lang::Pt).contains("estão em baixo"));
        assert!(Msg::Slicing.tr(Lang::Pt).starts_with("A "));
        // the three hints and the banner say that nothing reaches a real printer
        for l in Lang::ALL {
            assert!(Msg::SimBanner.tr(l).len() > 40);
        }
    }

    #[test]
    fn the_progress_and_travel_words_keep_the_order_of_their_places_and_the_words_of_portugal() {
        let new = &Msg::ALL[121..123];
        assert_eq!(new.len(), 2);
        for l in Lang::ALL {
            let s = fill(Msg::ProgressAtLayer.tr(l), &["T1", "R2", "P3"]);
            let order: Vec<usize> = ["T1", "R2", "P3"]
                .iter()
                .map(|x| s.find(x).unwrap())
                .collect();
            assert!(order.windows(2).all(|w| w[0] < w[1]), "{}: {s}", l.code());
            assert!(!Msg::ViewTravel.tr(l).trim().is_empty());
        }
        assert!(Msg::ProgressAtLayer.tr(Lang::Pt).contains("camada"));
        assert_eq!(Msg::ViewTravel.tr(Lang::Pt), "Deslocações");
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

    #[test]
    fn the_texts_of_the_panels_the_menus_and_the_check_before_printing_have_their_places_and_the_words_of_portugal(
    ) {
        let new = &Msg::ALL[205..];
        assert_eq!(new.len(), 24);
        let places = |m: Msg| match m {
            Msg::Exported => 1,
            Msg::FilamentsShown | Msg::PfDry | Msg::PfFlow => 2,
            Msg::PfNozzleTemp | Msg::PfBedTemp => 3,
            _ => 0,
        };
        for m in new {
            for l in Lang::ALL {
                assert_eq!(
                    m.tr(l).matches("{}").count(),
                    places(*m),
                    "{m:?} in {}",
                    l.code()
                );
            }
        }
        // the words of Portugal, and the formal register in the languages that have one
        assert!(Msg::PanelStatus.tr(Lang::Pt).contains("máquina"));
        assert!(Msg::NothingToExport.tr(Lang::Pt).starts_with("Fatie"));
        assert!(Msg::UpdatesViaUpdater.tr(Lang::Pt).contains("atualizador"));
        assert!(
            Msg::OpenTypePath.tr(Lang::Pt).contains("ficheiro")
                || Msg::OpenTypePath.tr(Lang::Pt).contains("caminho")
        );
        assert!(Msg::ExportPathHint.tr(Lang::Pt).contains("ficheiro"));
        assert!(Msg::SaveTypePath.tr(Lang::De).contains("Sie"));
        assert!(Msg::PfNoFilament.tr(Lang::Fr).starts_with("Choisissez"));
        assert!(Msg::PfDry.tr(Lang::Es).starts_with("Seque"));
        let both = |m: Msg, l: Lang| fill(m.tr(l), &["a", "b"]);
        assert!(
            both(Msg::FilamentsShown, Lang::En).contains('a')
                && both(Msg::FilamentsShown, Lang::En).contains('b')
        );
    }
}
