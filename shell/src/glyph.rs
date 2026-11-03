//! Small line icons, drawn by hand: one stroke width everywhere (the
//! `shape.icon_stroke` token), round ends, in the widget's own text color.
//! No icon theme is involved, so they look the same on every system.

use crate::tokens;
use gtk::{cairo, prelude::*};

#[derive(Clone, Copy)]
pub enum Glyph {
    /// ×, for dismissing.
    Close,
    /// ⌄, for opening a list below.
    ChevronDown,
    /// Three dots, for a menu.
    More,
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
    area.set_draw_func(move |area, cr, w, h| draw(glyph, &area.color(), cr, f64::from(w), f64::from(h)));
    area
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
