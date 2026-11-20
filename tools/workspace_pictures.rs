//! Writes one picture (SVG) of each workspace of the window, drawn from the model of the arrangement alone: the menu bar on top and, under it, the groups of panels of the workspace with their tabs.
//!
//!     cargo run --example workspace_pictures -- [--out images/layouts] [--lang en] [--size 1280x720]
//!
//! It is not a photograph of the window: it shows how the factory arrangement lays the panels out (what `src/panels.rs` says), with no window opened, so it can be written on any machine and in the
//! eight languages. The photographs of the window are taken with `phoenix-3dp gui --shot` (see `tools/make_screenshots.py`).

use eh3d_dock::preview::layout_svg;
use eh3d_dock::sectors::workspace_title;
use eh3d_phoenix_3dp::keys::{Scheme, SchemeId};
use eh3d_phoenix_3dp::panels;
use eh3d_view::texts::Lang;
use std::path::PathBuf;

const BAR_HEIGHT: f32 = 30.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// The picture of a workspace: the bar of menus and the layout.
fn picture(
    name: &str,
    layout: &eh3d_dock::model::Layout,
    lang: Lang,
    width: f32,
    height: f32,
) -> String {
    let defs = panels::defs();
    let bar = panels::menu_bar(&Scheme::of(SchemeId::Family));
    let inner = layout_svg(layout, &defs, lang, width, height - BAR_HEIGHT).replacen(
        "<svg ",
        &format!("<svg x=\"0\" y=\"{BAR_HEIGHT}\" "),
        1,
    );
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#12151c\"/>\n<rect width=\"100%\" height=\"{BAR_HEIGHT}\" fill=\"#1d222c\"/>\n",
        w = width,
        h = height
    );
    let mut x = 12.0_f32;
    for menu in &bar.menus {
        let title = menu.title.get(lang);
        out.push_str(&format!(
            "<text x=\"{x:.0}\" y=\"20\" fill=\"#d7dce5\" font-family=\"sans-serif\" font-size=\"13\">{}</text>\n",
            esc(title)
        ));
        let units: f32 = title
            .chars()
            .map(|c| if c.is_ascii() { 7.5 } else { 14.0 })
            .sum();
        x += units + 22.0;
    }
    let shown = workspace_title(name).map_or(name, |t| t.get(lang));
    out.push_str(&format!(
        "<text x=\"{:.0}\" y=\"20\" text-anchor=\"end\" fill=\"#8fb4ff\" font-family=\"sans-serif\" font-size=\"13\">{}</text>\n",
        width - 12.0,
        esc(shown)
    ));
    out.push_str(&inner);
    out.push_str("</svg>\n");
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| "images/layouts".to_string()));
    let lang = arg(&args, "--lang")
        .and_then(|c| Lang::from_code(&c))
        .unwrap_or(Lang::En);
    let size = arg(&args, "--size").unwrap_or_else(|| "1280x720".to_string());
    let (width, height) = size
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))
        .unwrap_or((1280.0, 720.0));
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("cannot make {}: {e}", out.display());
        std::process::exit(1);
    }
    for ws in panels::workspaces() {
        let file = out.join(format!("{}.svg", ws.name.to_ascii_lowercase()));
        let svg = picture(&ws.name, &ws.layout, lang, width, height);
        if let Err(e) = std::fs::write(&file, svg) {
            eprintln!("cannot write {}: {e}", file.display());
            std::process::exit(1);
        }
        println!("{}", file.display());
    }
}
