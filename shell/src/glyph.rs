//! Small line icons, drawn by hand: one stroke width everywhere (the
//! `shape.icon_stroke` token), round ends, in the widget's own text color.
//! No icon theme is involved, so they look the same on every system.

use crate::tokens;
use gtk::{cairo, prelude::*};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Glyph {
    /// ×, for dismissing.
    Close,
    /// ⌄, for opening a list below.
    ChevronDown,
    /// Three dots, for a menu.
    More,
    /// A lightning stroke, for mains power.
    Bolt,
    /// Three arcs over a dot; `level` (0–3) of them drawn full, the rest faint.
    /// `off`: the radio is off, so a stroke crosses it.
    Wifi { level: u8, off: bool },
    /// A network cable's plug, for a wired connection; crossed when unplugged.
    Wired { off: bool },
    /// The Bluetooth rune; crossed when Bluetooth is off.
    Bluetooth { off: bool },
    /// A loudspeaker with `waves` (0–2) sound waves, or a cross when muted.
    Speaker { waves: u8, muted: bool },
    /// A small padlock, for networks that need a password.
    Lock,
}

/// A `size`×`size` icon that follows the CSS `color` of where it is placed.
pub fn new(glyph: Glyph, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(size)
        .content_height(size)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .can_target(false)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    area.add_css_class("glyph");
    set(&area, glyph);
    area
}

/// Draw `area` as another glyph, such as the Wi‑Fi arcs at a new strength.
pub fn set(area: &gtk::DrawingArea, glyph: Glyph) {
    area.set_draw_func(move |area, cr, w, h| draw(glyph, &area.color(), cr, f64::from(w), f64::from(h)));
}

/// Parts that are not there (a weak signal's outer arcs) are drawn faintly.
fn faint(cr: &cairo::Context, color: &gtk::gdk::RGBA, faint: bool) {
    let alpha = f64::from(color.alpha()) * if faint { 0.3 } else { 1.0 };
    cr.set_source_rgba(color.red().into(), color.green().into(), color.blue().into(), alpha);
}

/// The stroke across a mark that is off.
fn slash(cr: &cairo::Context, at: &dyn Fn(f64, f64) -> (f64, f64)) {
    let (a, b) = (at(0.16, 0.14), at(0.84, 0.86));
    cr.move_to(a.0, a.1);
    cr.line_to(b.0, b.1);
    let _ = cr.stroke();
}

fn draw(glyph: Glyph, color: &gtk::gdk::RGBA, cr: &cairo::Context, w: f64, h: f64) {
    let s = w.min(h);
    let (x0, y0) = ((w - s) / 2.0, (h - s) / 2.0);
    // Points on a unit square, scaled to the icon.
    let at = |x: f64, y: f64| (x0 + x * s, y0 + y * s);
    let stroke = tokens::get().icon_stroke;
    cr.set_source_rgba(color.red().into(), color.green().into(), color.blue().into(), color.alpha().into());
    cr.set_line_width(stroke);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    match glyph {
        Glyph::Close => {
            for ((ax, ay), (bx, by)) in [((0.25, 0.25), (0.75, 0.75)), ((0.75, 0.25), (0.25, 0.75))] {
                let (a, b) = (at(ax, ay), at(bx, by));
                cr.move_to(a.0, a.1);
                cr.line_to(b.0, b.1);
            }
            let _ = cr.stroke();
        }
        Glyph::ChevronDown => {
            let points = [at(0.25, 0.4), at(0.5, 0.64), at(0.75, 0.4)];
            cr.move_to(points[0].0, points[0].1);
            for p in &points[1..] {
                cr.line_to(p.0, p.1);
            }
            let _ = cr.stroke();
        }
        Glyph::Bolt => {
            // One zigzag stroke, like the others a line rather than a filled shape.
            let points = [at(0.6, 0.1), at(0.32, 0.54), at(0.66, 0.46), at(0.4, 0.9)];
            cr.move_to(points[0].0, points[0].1);
            for p in &points[1..] {
                cr.line_to(p.0, p.1);
            }
            let _ = cr.stroke();
        }
        Glyph::Wifi { level, off } => {
            let (cx, cy) = at(0.5, 0.78);
            for (i, radius) in [0.2, 0.38, 0.56].into_iter().enumerate() {
                faint(cr, color, (i as u8) >= level);
                cr.new_sub_path();
                cr.arc(cx, cy, radius * s, -0.75 * std::f64::consts::PI, -0.25 * std::f64::consts::PI);
                let _ = cr.stroke();
            }
            faint(cr, color, level == 0);
            cr.new_sub_path();
            cr.arc(cx, cy, stroke * 0.8, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
            if off {
                faint(cr, color, false);
                slash(cr, &at);
            }
        }
        Glyph::Wired { off } => {
            // The plug's face, its latch, and the cable leaving downwards.
            let (a, b) = (at(0.26, 0.2), at(0.74, 0.62));
            cr.rectangle(a.0, a.1, b.0 - a.0, b.1 - a.1);
            for x in [0.4, 0.5, 0.6] {
                let (p, q) = (at(x, 0.3), at(x, 0.42));
                cr.move_to(p.0, p.1);
                cr.line_to(q.0, q.1);
            }
            let (p, q) = (at(0.5, 0.62), at(0.5, 0.86));
            cr.move_to(p.0, p.1);
            cr.line_to(q.0, q.1);
            let _ = cr.stroke();
            if off {
                slash(cr, &at);
            }
        }
        Glyph::Bluetooth { off } => {
            let points = [at(0.3, 0.32), at(0.7, 0.66), at(0.5, 0.86), at(0.5, 0.14), at(0.7, 0.34), at(0.3, 0.68)];
            cr.move_to(points[0].0, points[0].1);
            for p in &points[1..] {
                cr.line_to(p.0, p.1);
            }
            let _ = cr.stroke();
            if off {
                slash(cr, &at);
            }
        }
        Glyph::Speaker { waves, muted } => {
            let body = [at(0.12, 0.38), at(0.28, 0.38), at(0.5, 0.18), at(0.5, 0.82), at(0.28, 0.62), at(0.12, 0.62)];
            cr.move_to(body[0].0, body[0].1);
            for p in &body[1..] {
                cr.line_to(p.0, p.1);
            }
            cr.close_path();
            let _ = cr.stroke();
            if muted {
                for ((ax, ay), (bx, by)) in [((0.64, 0.38), (0.86, 0.62)), ((0.86, 0.38), (0.64, 0.62))] {
                    let (a, b) = (at(ax, ay), at(bx, by));
                    cr.move_to(a.0, a.1);
                    cr.line_to(b.0, b.1);
                }
                let _ = cr.stroke();
            } else {
                let (cx, cy) = at(0.5, 0.5);
                for (i, radius) in [0.16, 0.32].into_iter().enumerate() {
                    faint(cr, color, (i as u8) >= waves);
                    cr.new_sub_path();
                    cr.arc(cx, cy, radius * s, -0.3 * std::f64::consts::PI, 0.3 * std::f64::consts::PI);
                    let _ = cr.stroke();
                }
            }
        }
        Glyph::Lock => {
            let (a, b) = (at(0.26, 0.46), at(0.74, 0.86));
            cr.rectangle(a.0, a.1, b.0 - a.0, b.1 - a.1);
            let _ = cr.stroke();
            let (cx, cy) = at(0.5, 0.46);
            cr.new_sub_path();
            cr.arc(cx, cy, 0.15 * s, std::f64::consts::PI, 0.0);
            let _ = cr.stroke();
        }
        Glyph::More => {
            for x in [0.2, 0.5, 0.8] {
                let (cx, cy) = at(x, 0.5);
                cr.new_sub_path();
                cr.arc(cx, cy, stroke * 0.75, 0.0, std::f64::consts::TAU);
            }
            let _ = cr.fill();
        }
    }
}
