//! The texts of the program in the eight languages (English, Spanish, German, French, Italian, Japanese, Chinese and European Portuguese). A text with `{}` has the
//! value put in its place by `fill`. The codes of error and the keys of the settings are never translated.

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
}

impl Msg {
    pub const ALL: [Msg; 37] = [
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
        assert_eq!(Msg::ALL.len(), 37);
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
            if matches!(m, Msg::Save | Msg::Path | Msg::Volume) {
                continue;
            }
            let en = m.tr(Lang::En);
            let same = Lang::ALL.iter().filter(|l| m.tr(**l) == en).count();
            assert!(same <= 2, "{m:?} is the same text in {same} languages");
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
}
