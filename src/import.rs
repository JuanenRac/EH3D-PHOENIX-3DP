//! Bringing the settings of a file of a person (a 3MF project, a JSON profile, an INI) to the settings of the slicer. Every key of the file ends in one of four places that the
//! report shows: applied, not applied with the reason, read and not used, or not read. A value is taken as it is or not at all: nothing is rounded, lowered or guessed. The
//! value of a key that the program does not use is never printed (a profile may hold keys of access to a printer). Nothing is sent anywhere.

use crate::i18n::{fill, Lang};
use p3dp_core::Limits;
use p3dp_formats::settings_import::{read_settings, Pair, PairNote, SettingsFile, Unread};
use p3dp_formats::FormatError;
use p3dp_profiles::Catalog;
use p3dp_slicer::SliceConfig;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    /// The value may end in `%`.
    Percent,
    /// One of a few words, with the word of this program.
    Word(&'static [(&'static str, &'static str)]),
}

const SEAMS: &[(&str, &str)] = &[
    ("nearest", "nearest"),
    ("aligned", "aligned"),
    ("rear", "rear"),
    ("back", "rear"),
];
const PATTERNS: &[(&str, &str)] = &[
    ("rectilinear", "rectilinear"),
    ("grid", "grid"),
    ("gyroid", "gyroid"),
];

/// The keys of the files of other programs that mean what a setting of this one means: `(key of the file, key of the slicer, how to read it)`.
const MAP: &[(&str, &str, Kind)] = &[
    ("layer_height", "layer_height_mm", Kind::Plain),
    ("first_layer_height", "first_layer_height_mm", Kind::Plain),
    (
        "initial_layer_print_height",
        "first_layer_height_mm",
        Kind::Plain,
    ),
    ("nozzle_diameter", "nozzle_diameter_mm", Kind::Plain),
    ("line_width", "line_width_mm", Kind::Plain),
    ("extrusion_width", "line_width_mm", Kind::Plain),
    ("perimeters", "perimeters", Kind::Plain),
    ("wall_loops", "perimeters", Kind::Plain),
    ("wall_line_count", "perimeters", Kind::Plain),
    ("top_solid_layers", "top_layers", Kind::Plain),
    ("top_shell_layers", "top_layers", Kind::Plain),
    ("top_layers", "top_layers", Kind::Plain),
    ("bottom_solid_layers", "bottom_layers", Kind::Plain),
    ("bottom_shell_layers", "bottom_layers", Kind::Plain),
    ("bottom_layers", "bottom_layers", Kind::Plain),
    ("fill_density", "infill_percent", Kind::Percent),
    ("sparse_infill_density", "infill_percent", Kind::Percent),
    ("infill_sparse_density", "infill_percent", Kind::Percent),
    ("filament_diameter", "filament_diameter_mm", Kind::Plain),
    ("material_diameter", "filament_diameter_mm", Kind::Plain),
    ("extrusion_multiplier", "extrusion_multiplier", Kind::Plain),
    ("filament_flow_ratio", "extrusion_multiplier", Kind::Plain),
    ("temperature", "nozzle_c", Kind::Plain),
    ("nozzle_temperature", "nozzle_c", Kind::Plain),
    ("material_print_temperature", "nozzle_c", Kind::Plain),
    ("bed_temperature", "bed_c", Kind::Plain),
    ("hot_plate_temp", "bed_c", Kind::Plain),
    ("material_bed_temperature", "bed_c", Kind::Plain),
    ("chamber_temperature", "chamber_c", Kind::Plain),
    ("retract_length", "retract_mm", Kind::Plain),
    ("retraction_length", "retract_mm", Kind::Plain),
    ("retraction_amount", "retract_mm", Kind::Plain),
    ("retract_speed", "retract_speed_mm_s", Kind::Plain),
    ("retraction_speed", "retract_speed_mm_s", Kind::Plain),
    ("travel_speed", "travel_speed_mm_s", Kind::Plain),
    ("speed_travel", "travel_speed_mm_s", Kind::Plain),
    (
        "external_perimeter_speed",
        "external_perimeter_speed_mm_s",
        Kind::Plain,
    ),
    (
        "outer_wall_speed",
        "external_perimeter_speed_mm_s",
        Kind::Plain,
    ),
    ("speed_wall_0", "external_perimeter_speed_mm_s", Kind::Plain),
    ("perimeter_speed", "perimeter_speed_mm_s", Kind::Plain),
    ("inner_wall_speed", "perimeter_speed_mm_s", Kind::Plain),
    ("infill_speed", "infill_speed_mm_s", Kind::Plain),
    ("sparse_infill_speed", "infill_speed_mm_s", Kind::Plain),
    ("speed_infill", "infill_speed_mm_s", Kind::Plain),
    ("first_layer_speed", "first_layer_speed_mm_s", Kind::Plain),
    ("initial_layer_speed", "first_layer_speed_mm_s", Kind::Plain),
    ("speed_layer_0", "first_layer_speed_mm_s", Kind::Plain),
    ("brim_width", "brim_width_mm", Kind::Plain),
    ("elefant_foot_compensation", "elephant_foot_mm", Kind::Plain),
    ("retract_lift", "z_hop_mm", Kind::Plain),
    ("z_hop", "z_hop_mm", Kind::Plain),
    (
        "filament_max_volumetric_speed",
        "max_volumetric_flow_mm3_s",
        Kind::Plain,
    ),
    ("pressure_advance", "pressure_advance", Kind::Plain),
    ("seam_position", "seam", Kind::Word(SEAMS)),
    ("fill_pattern", "infill_pattern", Kind::Word(PATTERNS)),
    (
        "sparse_infill_pattern",
        "infill_pattern",
        Kind::Word(PATTERNS),
    ),
];

/// The keys that name the printer of a file, the best first.
const PRINTER_KEYS: [&str; 3] = ["printer_model", "printer_settings_id", "machine_name"];

/// Why a key that means something here was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// The slicer refuses the value; its own words say why.
    Refused(String),
    /// A part of another value (`50%`), which this program does not work out.
    Relative,
    /// No value, or one that means "as the parent profile".
    Empty,
    /// Another key already gave the setting another value.
    Conflict(String),
    /// The file switches the setting off.
    SwitchedOff,
    /// The value was longer than the limit and was cut.
    Cut,
    /// A word this program does not have.
    UnknownWord,
}

/// Where one key of the file ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fate {
    /// The setting of the slicer has this value; `of` says how many different values the file gave (one per extruder) when the first was taken.
    Applied {
        to: &'static str,
        value: String,
        of: Option<usize>,
    },
    NotApplied(Why),
    /// Read, and this program has no use for it.
    NotUsed,
    /// An object, a nested list or a repeated key.
    NotRead(PairNote),
}

/// One key of the file and where it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportLine {
    pub part: String,
    pub key: String,
    pub fate: Fate,
}

/// What an import found.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport {
    pub file: SettingsFile,
    pub lines: Vec<ImportLine>,
    /// The settings that were applied over the ones of the program, in the order they came.
    pub applied: Vec<(&'static str, String)>,
    pub config: SliceConfig,
    /// Why the settings, with the defaults for the rest, do not hold together; they are then not written.
    pub conflict: Option<String>,
    /// The printer the file names, and the printer of the catalog it is, when exactly one matches.
    pub printer_name: Option<String>,
    pub printer_id: Option<String>,
}

impl ImportReport {
    pub fn count(&self, wanted: fn(&Fate) -> bool) -> usize {
        self.lines.iter().filter(|l| wanted(&l.fate)).count()
    }

    /// The settings file that `slice --settings` reads: only the keys that were applied.
    pub fn settings_text(&self) -> String {
        let mut out = String::from("# settings imported by phoenix-3dp; the keys that are not here keep the values of the program\n");
        for (key, value) in &self.applied {
            out.push_str(key);
            out.push_str(" = ");
            out.push_str(value);
            out.push('\n');
        }
        out
    }
}

fn switched_off(pairs: &[Pair]) -> bool {
    pairs.iter().any(|p| {
        p.key == "enable_pressure_advance"
            && p.values
                .first()
                .is_some_and(|v| v == "0" || v.eq_ignore_ascii_case("false"))
    })
}

fn apply(
    pair: &Pair,
    to: &'static str,
    kind: Kind,
    config: &mut SliceConfig,
    applied: &mut Vec<(&'static str, String, String)>,
) -> Fate {
    if pair.note == Some(PairNote::Cut) {
        return Fate::NotApplied(Why::Cut);
    }
    let first = pair.values.first().map(|v| v.trim()).unwrap_or("");
    if first.is_empty() || first == "nil" {
        return Fate::NotApplied(Why::Empty);
    }
    let text = match kind {
        Kind::Plain => {
            if first.ends_with('%') {
                return Fate::NotApplied(Why::Relative);
            }
            first.to_string()
        }
        Kind::Percent => first.trim_end_matches('%').to_string(),
        Kind::Word(words) => {
            let Some((_, ours)) = words.iter().find(|(w, _)| w.eq_ignore_ascii_case(first)) else {
                return Fate::NotApplied(Why::UnknownWord);
            };
            (*ours).to_string()
        }
    };
    let mut next = config.clone();
    if let Err(e) = next.set(to, &text) {
        return Fate::NotApplied(Why::Refused(e.to_string()));
    }
    let Some(value) = next.value_of(to) else {
        return Fate::NotApplied(Why::Empty);
    };
    let differ = pair.values.iter().any(|v| v.trim() != first);
    let of = differ.then_some(pair.values.len());
    match applied.iter().find(|(k, _, _)| *k == to) {
        Some((_, before, from)) if *before != value => {
            return Fate::NotApplied(Why::Conflict(from.clone()));
        }
        Some(_) => {}
        None => {
            applied.push((to, value.clone(), pair.key.clone()));
            *config = next;
        }
    }
    Fate::Applied { to, value, of }
}

/// The printer of the catalog that a name means: the one whose whole title or id it is, or the only one whose model it is.
fn find_printer(catalog: &Catalog, name: &str) -> Option<String> {
    let want = name.trim().to_lowercase();
    if want.is_empty() {
        return None;
    }
    let only = |matches: Vec<&p3dp_profiles::PrinterProfile>| match matches.as_slice() {
        [one] => Some(one.id.clone()),
        _ => None,
    };
    only(
        catalog
            .profiles
            .iter()
            .filter(|p| p.title().to_lowercase() == want || p.id == want)
            .collect(),
    )
    .or_else(|| {
        only(
            catalog
                .profiles
                .iter()
                .filter(|p| p.model.to_lowercase() == want)
                .collect(),
        )
    })
}

/// Reads a file and brings its settings.
pub fn import(
    bytes: &[u8],
    limits: &Limits,
    catalog: &Catalog,
) -> Result<ImportReport, FormatError> {
    let file = read_settings(bytes, limits)?;
    let off = switched_off(&file.pairs);
    let mut config = SliceConfig::default();
    let mut applied = Vec::new();
    let mut lines = Vec::with_capacity(file.pairs.len());
    for pair in &file.pairs {
        let fate = match pair.note {
            Some(note @ (PairNote::Structured | PairNote::Repeated)) => Fate::NotRead(note),
            _ => match MAP.iter().find(|(from, _, _)| *from == pair.key) {
                None => Fate::NotUsed,
                Some((_, "pressure_advance", _)) if off => Fate::NotApplied(Why::SwitchedOff),
                Some((_, to, kind)) => apply(pair, to, *kind, &mut config, &mut applied),
            },
        };
        lines.push(ImportLine {
            part: pair.part.clone(),
            key: pair.key.clone(),
            fate,
        });
    }
    let printer_name = PRINTER_KEYS.iter().find_map(|key| {
        file.pairs
            .iter()
            .find(|p| p.key == *key)
            .and_then(|p| p.values.first())
            .filter(|v| !v.is_empty())
            .cloned()
    });
    let printer_id = printer_name
        .as_deref()
        .and_then(|n| find_printer(catalog, n));
    let conflict = config.check().err().map(|e| e.to_string());
    Ok(ImportReport {
        file,
        lines,
        applied: applied.into_iter().map(|(k, v, _)| (k, v)).collect(),
        config,
        conflict,
        printer_name,
        printer_id,
    })
}

/// The words of the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Word {
    Project,
    JsonProfile,
    IniProfile,
    PartsRead,
    Applied,
    NotApplied,
    NotUsed,
    NotRead,
    FirstOf,
    Relative,
    Empty,
    Conflict,
    SwitchedOff,
    Cut,
    UnknownWord,
    Structured,
    Repeated,
    Geometry,
    PerObject,
    Gcode,
    Picture,
    Unknown,
    TooLarge,
    Damaged,
    UnsafeName,
    Twice,
    BadLine,
    PrinterNamed,
    PrinterUnknown,
    DoesNotHold,
    NothingSent,
    Written,
    NothingToWrite,
}

impl Word {
    pub const ALL: [Word; 33] = [
        Word::Project,
        Word::JsonProfile,
        Word::IniProfile,
        Word::PartsRead,
        Word::Applied,
        Word::NotApplied,
        Word::NotUsed,
        Word::NotRead,
        Word::FirstOf,
        Word::Relative,
        Word::Empty,
        Word::Conflict,
        Word::SwitchedOff,
        Word::Cut,
        Word::UnknownWord,
        Word::Structured,
        Word::Repeated,
        Word::Geometry,
        Word::PerObject,
        Word::Gcode,
        Word::Picture,
        Word::Unknown,
        Word::TooLarge,
        Word::Damaged,
        Word::UnsafeName,
        Word::Twice,
        Word::BadLine,
        Word::PrinterNamed,
        Word::PrinterUnknown,
        Word::DoesNotHold,
        Word::NothingSent,
        Word::Written,
        Word::NothingToWrite,
    ];

    /// The words in a language; `{}` takes a value.
    pub fn tr(self, lang: Lang) -> &'static str {
        let texts: [&'static str; 8] = match self {
            Word::Project => ["3MF project ({})", "Proyecto 3MF ({})", "3MF-Projekt ({})", "Projet 3MF ({})", "Progetto 3MF ({})", "3MF プロジェクト（{}）", "3MF 项目（{}）", "Projeto 3MF ({})"],
            Word::JsonProfile => ["JSON profile", "Perfil JSON", "JSON-Profil", "Profil JSON", "Profilo JSON", "JSON プロファイル", "JSON 配置文件", "Perfil JSON"],
            Word::IniProfile => ["INI profile", "Perfil INI", "INI-Profil", "Profil INI", "Profilo INI", "INI プロファイル", "INI 配置文件", "Perfil INI"],
            Word::PartsRead => ["Parts read", "Partes leídas", "Gelesene Teile", "Parties lues", "Parti lette", "読み込んだ部分", "已读取部分", "Partes lidas"],
            Word::Applied => ["Applied ({})", "Aplicados ({})", "Übernommen ({})", "Appliqués ({})", "Applicati ({})", "適用（{}）", "已应用（{}）", "Aplicados ({})"],
            Word::NotApplied => ["Not applied ({})", "No aplicados ({})", "Nicht übernommen ({})", "Non appliqués ({})", "Non applicati ({})", "未適用（{}）", "未应用（{}）", "Não aplicados ({})"],
            Word::NotUsed => ["Read but not used by this program ({})", "Leídos pero sin uso en este programa ({})", "Gelesen, aber von diesem Programm nicht verwendet ({})", "Lus mais non utilisés par ce programme ({})", "Letti ma non usati da questo programma ({})", "読み込んだがこのプログラムでは使わない項目（{}）", "已读取但本程序不使用（{}）", "Lidos mas não usados por este programa ({})"],
            Word::NotRead => ["Not read ({})", "No leídos ({})", "Nicht gelesen ({})", "Non lus ({})", "Non letti ({})", "読み込まなかったもの（{}）", "未读取（{}）", "Não lidos ({})"],
            Word::FirstOf => ["the first of {} values", "el primero de {} valores", "der erste von {} Werten", "la première de {} valeurs", "il primo di {} valori", "{} 個の値の最初", "{} 个值中的第一个", "o primeiro de {} valores"],
            Word::Relative => ["a percentage of another value", "un porcentaje de otro valor", "ein Prozentsatz eines anderen Werts", "un pourcentage d'une autre valeur", "una percentuale di un altro valore", "別の値に対する割合です", "是另一个值的百分比", "uma percentagem de outro valor"],
            Word::Empty => ["no value of its own", "sin valor propio", "kein eigener Wert", "pas de valeur propre", "nessun valore proprio", "独自の値がありません", "没有自己的值", "sem valor próprio"],
            Word::Conflict => ["{} already gave another value", "{} ya dio otro valor", "{} hat bereits einen anderen Wert gesetzt", "{} a déjà donné une autre valeur", "{} ha già dato un altro valore", "{} がすでに別の値を指定しています", "{} 已给出另一个值", "{} já deu outro valor"],
            Word::SwitchedOff => ["the file switches it off", "el fichero lo desactiva", "die Datei schaltet es aus", "le fichier le désactive", "il file lo disattiva", "ファイルで無効になっています", "文件中已关闭此项", "o ficheiro desativa-o"],
            Word::Cut => ["the value was too long", "el valor era demasiado largo", "der Wert war zu lang", "la valeur était trop longue", "il valore era troppo lungo", "値が長すぎます", "值过长", "o valor era demasiado longo"],
            Word::UnknownWord => ["a choice this program does not have", "una opción que este programa no tiene", "eine Wahl, die dieses Programm nicht hat", "un choix que ce programme n'a pas", "una scelta che questo programma non ha", "このプログラムにない選択肢です", "本程序没有此选项", "uma opção que este programa não tem"],
            Word::Structured => ["a structured or empty value", "un valor compuesto o vacío", "ein zusammengesetzter oder leerer Wert", "une valeur composée ou vide", "un valore composto o vuoto", "構造化された値または空の値", "结构化值或空值", "um valor composto ou vazio"],
            Word::Repeated => ["a repeated key", "una clave repetida", "ein doppelter Schlüssel", "une clé répétée", "una chiave ripetuta", "重複したキー", "重复的键", "uma chave repetida"],
            Word::Geometry => ["the shapes of the model", "las formas del modelo", "die Formen des Modells", "les formes du modèle", "le forme del modello", "モデルの形状", "模型形状", "as formas do modelo"],
            Word::PerObject => ["settings of single objects", "ajustes de objetos concretos", "Einstellungen einzelner Objekte", "réglages d'objets particuliers", "impostazioni di singoli oggetti", "個別オブジェクトの設定", "单个对象的设置", "definições de objetos individuais"],
            Word::Gcode => ["G-code", "G-code", "G-Code", "G-code", "G-code", "G-code", "G-code", "G-code"],
            Word::Picture => ["a picture", "una imagen", "ein Bild", "une image", "un'immagine", "画像", "图片", "uma imagem"],
            Word::Unknown => ["not known to this program", "este programa no lo conoce", "diesem Programm unbekannt", "inconnu de ce programme", "sconosciuto a questo programma", "このプログラムには不明です", "本程序无法识别", "desconhecido deste programa"],
            Word::TooLarge => ["too large", "demasiado grande", "zu groß", "trop volumineux", "troppo grande", "大きすぎます", "过大", "demasiado grande"],
            Word::Damaged => ["damaged", "dañado", "beschädigt", "endommagé", "danneggiato", "破損しています", "已损坏", "danificado"],
            Word::UnsafeName => ["an unsafe name", "un nombre peligroso", "ein unsicherer Name", "un nom dangereux", "un nome non sicuro", "安全でない名前", "不安全的名称", "um nome inseguro"],
            Word::Twice => ["a name used twice", "un nombre usado dos veces", "ein doppelt verwendeter Name", "un nom utilisé deux fois", "un nome usato due volte", "二重に使われた名前", "名称重复使用", "um nome usado duas vezes"],
            Word::BadLine => ["a line that is not a setting", "una línea que no es un ajuste", "eine Zeile, die keine Einstellung ist", "une ligne qui n'est pas un réglage", "una riga che non è un'impostazione", "設定ではない行", "不是设置项的行", "uma linha que não é uma definição"],
            Word::PrinterNamed => ["Printer named in the file: {} (in the catalog: {})", "Impresora citada en el fichero: {} (en el catálogo: {})", "In der Datei genannter Drucker: {} (im Katalog: {})", "Imprimante nommée dans le fichier : {} (dans le catalogue : {})", "Stampante indicata nel file: {} (nel catalogo: {})", "ファイル内のプリンター: {}（カタログ: {}）", "文件中指定的打印机：{}（目录中：{}）", "Impressora indicada no ficheiro: {} (no catálogo: {})"],
            Word::PrinterUnknown => ["Printer named in the file: {} (no single match in the catalog)", "Impresora citada en el fichero: {} (sin coincidencia única en el catálogo)", "In der Datei genannter Drucker: {} (keine eindeutige Entsprechung im Katalog)", "Imprimante nommée dans le fichier : {} (pas de correspondance unique dans le catalogue)", "Stampante indicata nel file: {} (nessuna corrispondenza unica nel catalogo)", "ファイル内のプリンター: {}（カタログに一意の一致がありません）", "文件中指定的打印机：{}（目录中没有唯一匹配）", "Impressora indicada no ficheiro: {} (sem correspondência única no catálogo)"],
            Word::DoesNotHold => ["The settings do not hold together and cannot be written: {}", "Los ajustes no son coherentes y no se pueden escribir: {}", "Die Einstellungen passen nicht zusammen und werden nicht geschrieben: {}", "Les réglages ne sont pas cohérents et ne peuvent pas être écrits : {}", "Le impostazioni non sono coerenti e non possono essere scritte: {}", "設定に矛盾があるため書き出せません: {}", "设置相互矛盾，无法写入：{}", "As definições não são coerentes e não podem ser escritas: {}"],
            Word::NothingSent => ["Nothing was sent to any printer", "No se ha enviado nada a ninguna impresora", "Nichts wurde an einen Drucker gesendet", "Rien n'a été envoyé à une imprimante", "Nulla è stato inviato a una stampante", "プリンターには何も送信していません", "未向任何打印机发送内容", "Nada foi enviado para nenhuma impressora"],
            Word::Written => ["Settings written to {}", "Ajustes escritos en {}", "Einstellungen geschrieben nach {}", "Réglages écrits dans {}", "Impostazioni scritte in {}", "設定を {} に書き出しました", "设置已写入 {}", "Definições escritas em {}"],
            Word::NothingToWrite => ["No setting was applied: there is nothing to write", "No se aplicó ningún ajuste: no hay nada que escribir", "Keine Einstellung übernommen: nichts zu schreiben", "Aucun réglage appliqué : rien à écrire", "Nessuna impostazione applicata: nulla da scrivere", "適用された設定がないため、書き出すものがありません", "没有应用任何设置，无可写入内容", "Nenhuma definição foi aplicada: não há nada para escrever"],
        };
        let i = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
        texts[i]
    }
}

fn unread_word(u: Unread) -> Word {
    match u {
        Unread::Geometry => Word::Geometry,
        Unread::PerObject => Word::PerObject,
        Unread::Gcode => Word::Gcode,
        Unread::Picture => Word::Picture,
        Unread::Unknown => Word::Unknown,
        Unread::TooLarge => Word::TooLarge,
        Unread::Damaged => Word::Damaged,
        Unread::UnsafeName => Word::UnsafeName,
        Unread::Twice => Word::Twice,
        Unread::BadLine => Word::BadLine,
    }
}

fn why_text(why: &Why, lang: Lang) -> String {
    match why {
        Why::Refused(words) => words.clone(),
        Why::Relative => Word::Relative.tr(lang).to_string(),
        Why::Empty => Word::Empty.tr(lang).to_string(),
        Why::Conflict(from) => fill(Word::Conflict.tr(lang), &[from]),
        Why::SwitchedOff => Word::SwitchedOff.tr(lang).to_string(),
        Why::Cut => Word::Cut.tr(lang).to_string(),
        Why::UnknownWord => Word::UnknownWord.tr(lang).to_string(),
    }
}

fn named(part: &str, key: &str) -> String {
    if part.is_empty() {
        key.to_string()
    } else {
        format!("[{part}] {key}")
    }
}

/// The report as lines of text.
pub fn report_lines(report: &ImportReport, lang: Lang) -> Vec<String> {
    use p3dp_formats::settings_import::SourceKind;
    let mut out = Vec::new();
    out.push(match report.file.kind {
        SourceKind::Project(producer) => fill(Word::Project.tr(lang), &[producer.name()]),
        SourceKind::Json => Word::JsonProfile.tr(lang).to_string(),
        SourceKind::Ini => Word::IniProfile.tr(lang).to_string(),
    });
    if !report.file.read.is_empty() {
        out.push(format!(
            "{}: {}",
            Word::PartsRead.tr(lang),
            report.file.read.join(", ")
        ));
    }
    let count = |n: usize, word: Word| fill(word.tr(lang), &[&n.to_string()]);
    out.push(count(
        report.count(|f| matches!(f, Fate::Applied { .. })),
        Word::Applied,
    ));
    for l in &report.lines {
        if let Fate::Applied { to, value, of } = &l.fate {
            let first = of.map_or(String::new(), |n| {
                format!(" ({})", fill(Word::FirstOf.tr(lang), &[&n.to_string()]))
            });
            out.push(format!(
                "  {to} = {value}  <- {}{first}",
                named(&l.part, &l.key)
            ));
        }
    }
    out.push(count(
        report.count(|f| matches!(f, Fate::NotApplied(_))),
        Word::NotApplied,
    ));
    for l in &report.lines {
        if let Fate::NotApplied(why) = &l.fate {
            out.push(format!(
                "  {}: {}",
                named(&l.part, &l.key),
                why_text(why, lang)
            ));
        }
    }
    out.push(count(
        report.count(|f| matches!(f, Fate::NotUsed)),
        Word::NotUsed,
    ));
    for l in &report.lines {
        if l.fate == Fate::NotUsed {
            out.push(format!("  {}", named(&l.part, &l.key)));
        }
    }
    let not_read = report.count(|f| matches!(f, Fate::NotRead(_))) + report.file.unread.len();
    out.push(count(not_read, Word::NotRead));
    for l in &report.lines {
        if let Fate::NotRead(note) = l.fate {
            let word = if note == PairNote::Repeated {
                Word::Repeated
            } else {
                Word::Structured
            };
            out.push(format!("  {}: {}", named(&l.part, &l.key), word.tr(lang)));
        }
    }
    for (name, why) in &report.file.unread {
        out.push(format!("  {name}: {}", unread_word(*why).tr(lang)));
    }
    if let Some(name) = &report.printer_name {
        out.push(match &report.printer_id {
            Some(id) => fill(Word::PrinterNamed.tr(lang), &[name, id]),
            None => fill(Word::PrinterUnknown.tr(lang), &[name]),
        });
    }
    if let Some(conflict) = &report.conflict {
        out.push(fill(Word::DoesNotHold.tr(lang), &[conflict]));
    }
    out.push(Word::NothingSent.tr(lang).to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3dp_formats::zip::Writer;
    use p3dp_profiles::suite_overlay;

    fn catalog() -> Catalog {
        suite_overlay().unwrap().catalog
    }

    fn run(bytes: &[u8]) -> ImportReport {
        import(bytes, &Limits::default(), &catalog()).unwrap()
    }

    fn fate<'a>(r: &'a ImportReport, key: &str) -> &'a Fate {
        &r.lines.iter().find(|l| l.key == key).unwrap().fate
    }

    fn value<'a>(r: &'a ImportReport, key: &str) -> Option<&'a str> {
        r.applied
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn every_key_of_the_map_is_a_setting_of_the_slicer() {
        for (from, to, _) in MAP {
            assert!(p3dp_slicer::config::KEYS.contains(to), "{from} -> {to}");
            assert_eq!(MAP.iter().filter(|m| m.0 == *from).count(), 1, "{from}");
        }
    }

    #[test]
    fn a_json_profile_is_brought_and_every_key_ends_somewhere() {
        let r = run(br#"{"name":"mine","layer_height":"0.16","initial_layer_print_height":"0.2","wall_loops":"3","sparse_infill_density":"15%","nozzle_temperature":["215","220"],"hot_plate_temp":["60","60"],"outer_wall_speed":"50%","chamber_temperature":"nil","seam_position":"back","sparse_infill_pattern":"lightning","printer_model":"Bambu Lab A1","compatible_printers":{"a":1},"access_code":"TOPCODE-1"}"#);
        assert_eq!(value(&r, "layer_height_mm"), Some("0.16"));
        assert_eq!(value(&r, "perimeters"), Some("3"));
        assert_eq!(value(&r, "infill_percent"), Some("15"));
        assert_eq!(value(&r, "nozzle_c"), Some("215"));
        assert_eq!(value(&r, "seam"), Some("rear"));
        assert!(matches!(
            fate(&r, "nozzle_temperature"),
            Fate::Applied { of: Some(2), .. }
        ));
        assert!(matches!(
            fate(&r, "hot_plate_temp"),
            Fate::Applied { of: None, .. }
        ));
        assert_eq!(
            *fate(&r, "outer_wall_speed"),
            Fate::NotApplied(Why::Relative)
        );
        assert_eq!(
            *fate(&r, "chamber_temperature"),
            Fate::NotApplied(Why::Empty)
        );
        assert_eq!(
            *fate(&r, "sparse_infill_pattern"),
            Fate::NotApplied(Why::UnknownWord)
        );
        assert_eq!(*fate(&r, "name"), Fate::NotUsed);
        assert_eq!(
            *fate(&r, "compatible_printers"),
            Fate::NotRead(PairNote::Structured)
        );
        assert_eq!(r.lines.len(), r.file.pairs.len());
        assert_eq!(r.printer_id.as_deref(), Some("bambu-lab-a1"));
        assert!(r.conflict.is_none());
        // the value of a key the program does not use is never printed
        for lang in Lang::ALL {
            let text = report_lines(&r, lang).join("\n");
            assert!(text.contains("access_code"));
            assert!(!text.contains("TOPCODE-1"));
        }
    }

    #[test]
    fn the_settings_that_are_written_are_read_back_by_the_slicer() {
        let r = run(b"layer_height = 0.12\nperimeters = 4\nfill_density = 30%\nbed_temperature = 65\nunknown_thing = 9\n");
        let c = SliceConfig::from_text(&r.settings_text()).unwrap();
        assert_eq!(c, r.config);
        assert_eq!((c.layer_height_mm, c.perimeters, c.bed_c), (0.12, 4, 65.0));
        assert!(!r.settings_text().contains("unknown_thing"));
    }

    #[test]
    fn hostile_values_are_refused_and_change_nothing() {
        for bad in [
            "nan",
            "inf",
            "-inf",
            "1e999",
            "-1",
            "1000000000000",
            "0.2mm",
            "0x10",
        ] {
            let r = run(format!("layer_height = {bad}\nperimeters = {bad}\n").as_bytes());
            assert!(r.applied.is_empty(), "{bad}: {:?}", r.applied);
            assert_eq!(r.config, SliceConfig::default());
            assert_eq!(r.count(|f| matches!(f, Fate::NotApplied(_))), 2, "{bad}");
        }
    }

    #[test]
    fn two_keys_for_one_setting_never_replace_each_other_in_silence() {
        let r = run(b"perimeters = 2\nwall_loops = 5\nwall_line_count = 2\n");
        assert_eq!(value(&r, "perimeters"), Some("2"));
        assert_eq!(
            *fate(&r, "wall_loops"),
            Fate::NotApplied(Why::Conflict("perimeters".to_string()))
        );
        assert!(matches!(fate(&r, "wall_line_count"), Fate::Applied { .. }));
        assert_eq!(r.applied.len(), 1);
    }

    #[test]
    fn settings_that_do_not_hold_together_are_told() {
        let r = run(b"nozzle_diameter = 0.2\nlayer_height = 0.4\n");
        assert!(r.conflict.is_some());
    }

    #[test]
    fn a_pressure_advance_that_is_switched_off_is_not_applied() {
        let r = run(br#"{"enable_pressure_advance":["0"],"pressure_advance":["0.04"]}"#);
        assert_eq!(
            *fate(&r, "pressure_advance"),
            Fate::NotApplied(Why::SwitchedOff)
        );
        let r = run(br#"{"enable_pressure_advance":["1"],"pressure_advance":["0.04"]}"#);
        assert_eq!(value(&r, "pressure_advance"), Some("0.04"));
    }

    #[test]
    fn a_project_of_the_test_is_brought_with_what_was_not_read() {
        let mut z = Writer::new();
        z.add(
            "3D/3dmodel.model",
            b"<?xml version=\"1.0\"?><model><resources/><build/></model>",
        )
        .unwrap();
        z.add(
            "Metadata/project_settings.config",
            br#"{"layer_height":"0.24","printer_model":"Creality K1C"}"#,
        )
        .unwrap();
        z.add("Metadata/plate_1.png", b"x").unwrap();
        let r = run(&z.finish());
        assert_eq!(value(&r, "layer_height_mm"), Some("0.24"));
        assert_eq!(r.printer_id.as_deref(), Some("creality-k1c"));
        let text = report_lines(&r, Lang::En).join("\n");
        assert!(text.contains("Metadata/plate_1.png: a picture"), "{text}");
        assert!(text.contains("3D/3dmodel.model: the shapes of the model"));
    }

    #[test]
    fn a_printer_that_is_not_one_of_the_catalog_is_not_guessed() {
        let r = run(b"printer_model = Some Printer\nlayer_height = 0.2\n");
        assert_eq!(r.printer_name.as_deref(), Some("Some Printer"));
        assert_eq!(r.printer_id, None);
    }

    #[test]
    fn every_word_of_the_report_is_in_the_eight_languages() {
        for word in Word::ALL {
            let holes = word.tr(Lang::En).matches("{}").count();
            for lang in Lang::ALL {
                assert!(!word.tr(lang).is_empty());
                assert_eq!(word.tr(lang).matches("{}").count(), holes, "{word:?}");
            }
        }
    }
}
