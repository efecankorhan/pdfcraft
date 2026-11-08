//! Radial tool wheel for graphic tablets, styluses and handwriting.
//!
//! Opened by a secondary click (a stylus barrel button) on the page. In its main mode the wheel has
//! two rings around a centre button: the inner ring is the colour palette, the outer ring the
//! tools (the four pen modes, eraser, lasso, undo/redo, text, shapes, size, palette, more). Sub-menus
//! (shapes, sizes, colours, …) use the outer ring and turn the centre into Back. Every slot is a
//! wedge, so the whole wedge is the target, and each carries an accessibility label.

use egui::{Color32, Mesh, Pos2, Rect, Stroke, Vec2, vec2};
use pdfcraft_engine::Rgb;

use crate::comments::{self, CommentTool, PenMode, color32};
use crate::{PdfCraftApp, QuickTool, icons};

#[derive(Clone, Debug)]
pub struct RadialMenu {
    pub screen_pos: Pos2,
    pub page: usize,
    pub at: [f64; 2],
    pub mode: RadialMode,
    pub opened_at: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RadialMode {
    #[default]
    Tools,
    Colors,
    CustomColor,
    Thickness,
    FontSize,
    Shapes,
    More,
}

impl RadialMode {
    /// Where the centre's Back button goes from this sub-menu.
    fn back(self) -> RadialMode {
        match self {
            Self::CustomColor => Self::Colors,
            _ => Self::Tools,
        }
    }
}

/// Wheel geometry at full size (logical points from the centre).
const R_CENTER: f32 = 32.0;
const R_INNER_IN: f32 = 38.0;
const R_INNER_OUT: f32 = 68.0;
const R_OUTER_IN: f32 = 72.0;
const R_OUTER_OUT: f32 = 126.0;
/// Gap between neighbouring wedges, in radians.
const GAP: f32 = 0.035;
const ACCENT: Color32 = Color32::from_rgb(10, 132, 255);

struct Slot {
    icon: Option<&'static str>,
    color: Option<Rgb>,
    dot_radius: Option<f32>,
    /// A stroke sample this many points wide (the size choices).
    line_width: Option<f32>,
    /// A small colour mark at the wedge's inner edge (each pen mode's own colour).
    mark: Option<Rgb>,
    alpha: f32,
    label: &'static str,
    active: bool,
    action: SlotAction,
}

impl Slot {
    fn icon(icon: &'static str, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: Some(icon), color: None, dot_radius: None, line_width: None, mark: None, alpha: 1.0, label, active, action }
    }

    fn swatch(color: Rgb, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: None, color: Some(color), dot_radius: None, line_width: None, mark: None, alpha: 1.0, label, active, action }
    }

    fn dot(radius: f32, color: Rgb, alpha: f32, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: None, color: Some(color), dot_radius: Some(radius), line_width: None, mark: None, alpha, label, active, action }
    }

    fn line(width: f32, color: Rgb, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: None, color: Some(color), dot_radius: None, line_width: Some(width), mark: None, alpha: 1.0, label, active, action }
    }

    fn marked(mut self, c: Rgb) -> Self {
        self.mark = Some(c);
        self
    }

    fn dimmed(mut self, dim: bool) -> Self {
        if dim {
            self.alpha = 0.35;
        }
        self
    }
}

#[derive(Clone, Copy)]
enum SlotAction {
    PenMode(PenMode),
    Lasso,
    Tool(CommentTool),
    SelectTool,
    Undo,
    Redo,
    SwitchMode(RadialMode),
    SetColor(Rgb),
    SelectPaletteSlot(usize),
    SetWidth(f64),
    SetOpacity(f64),
    SetFontSize(f64),
    ToggleStrokeEraser,
}

/// What the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Center,
    Inner(usize),
    Outer(usize),
}

/// The wedge index a direction falls in, for `n` wedges with wedge 0 centred at the top and the
/// rest clockwise (screen y grows downwards).
fn wedge_at(dir: Vec2, n: usize) -> Option<usize> {
    if n == 0 || !dir.x.is_finite() || !dir.y.is_finite() {
        return None;
    }
    let tau = std::f32::consts::TAU;
    let rel = (dir.y.atan2(dir.x) + std::f32::consts::FRAC_PI_2).rem_euclid(tau);
    let step = tau / n as f32;
    // Saturating float-to-int cast; `% n` keeps it in range.
    Some(((rel / step + 0.5).floor() as usize) % n)
}

/// The angle of wedge `k`'s centre (see [`wedge_at`]).
fn wedge_angle(k: usize, n: usize) -> f32 {
    -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / n.max(1) as f32
}

fn hit_test(p: Pos2, center: Pos2, scale: f32, inner: usize, outer: usize) -> Option<Hit> {
    let d = p - center;
    let r = d.length();
    if r <= R_CENTER * scale + 2.0 {
        Some(Hit::Center)
    } else if inner > 0 && r < (R_INNER_OUT + R_OUTER_IN) * 0.5 * scale {
        wedge_at(d, inner).map(Hit::Inner)
    } else if r <= R_OUTER_OUT * scale + 8.0 {
        // With no inner ring (sub-menus) the outer slots reach down to the centre button.
        wedge_at(d, outer).map(Hit::Outer)
    } else {
        None
    }
}

/// An annular sector from angle `a0` to `a1` between radii `r0` and `r1`.
fn sector(center: Pos2, r0: f32, r1: f32, a0: f32, a1: f32, color: Color32) -> egui::Shape {
    let mut mesh = Mesh::default();
    let steps = (((a1 - a0).abs() / 0.06).ceil() as usize).clamp(2, 128);
    for k in 0..=steps {
        let a = a0 + (a1 - a0) * k as f32 / steps as f32;
        let dir = vec2(a.cos(), a.sin());
        mesh.colored_vertex(center + dir * r0, color);
        mesh.colored_vertex(center + dir * r1, color);
    }
    for k in 0..steps as u32 {
        let i = k * 2;
        mesh.add_triangle(i, i + 1, i + 3);
        mesh.add_triangle(i, i + 3, i + 2);
    }
    egui::Shape::mesh(mesh)
}

/// An arc outline from `a0` to `a1` at radius `r`.
fn arc(center: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) -> egui::Shape {
    let steps = (((a1 - a0).abs() / 0.05).ceil() as usize).clamp(2, 160);
    let pts = (0..=steps).map(|k| center + Vec2::angled(a0 + (a1 - a0) * k as f32 / steps as f32) * r).collect();
    egui::Shape::line(pts, stroke)
}

/// Where to put the wheel so all of it, and the label above it, stays inside `avail`.
fn clamp_center(p: Pos2, avail: Rect) -> Pos2 {
    let m = R_OUTER_OUT + 6.0;
    // A view too small for the wheel gets it centred (and `clamp` never sees lo > hi).
    let fit = |v: f32, lo: f32, hi: f32, mid: f32| if lo <= hi { v.clamp(lo, hi) } else { mid };
    let x = fit(p.x, avail.left() + m, avail.right() - m, avail.center().x);
    let y = fit(p.y, avail.top() + m + 30.0, avail.bottom() - m, avail.center().y);
    if x.is_finite() && y.is_finite() { egui::pos2(x, y) } else { p }
}

pub(crate) fn show(app: &mut PdfCraftApp, view_index: usize, ui: &mut egui::Ui, avail: Rect) {
    let (radial, view_id) = match app.views.get(view_index) {
        Some(v) => (v.comments.radial_menu.clone(), v.id.0),
        None => return,
    };
    let is_open = radial.is_some();

    let anim_id = egui::Id::new(("radial-menu-anim", view_id));
    let t = ui.ctx().animate_bool_with_time_and_easing(anim_id, is_open, 0.18, egui::emath::easing::cubic_out);
    if is_open && t < 1.0 {
        ui.ctx().request_repaint();
    }
    let Some(menu) = radial else {
        return;
    };
    let t = if is_open { t.max(0.18) } else { t };
    if t <= 0.005 {
        return;
    }

    let center = clamp_center(menu.screen_pos, avail);
    let scale = 0.86 + 0.14 * t;

    // Tool targeted by style adjustments (colours, size and opacity). With the Eraser or Select
    // tool on, styling targets the pen, so the next stroke gets it.
    let style_tool = match app.quick_tool {
        QuickTool::Comment(t) if t != CommentTool::Eraser => t,
        _ => CommentTool::Ink,
    };
    let current_style = app.comment_prefs.style(style_tool);
    let text_mode = matches!(app.quick_tool, QuickTool::Comment(CommentTool::TextBox | CommentTool::Callout));
    let ink_on = app.quick_tool == QuickTool::Comment(CommentTool::Ink);
    let pen_mode = app.comment_prefs.pen_mode();
    let palette = app.comment_prefs.radial_palette;
    let palette_name = |c: &Rgb| comments::SWATCHES.iter().find(|(_, s)| comments::same_colour(s, c)).map(|(n, _)| *n).unwrap_or("Custom colour");

    let inner: Vec<Slot> = if menu.mode == RadialMode::Tools {
        palette.iter().map(|c| Slot::swatch(*c, palette_name(c), comments::same_colour(&current_style.color, c), SlotAction::SetColor(*c))).collect()
    } else {
        Vec::new()
    };

    let outer: Vec<Slot> = match menu.mode {
        RadialMode::Tools => {
            let doc = app.active_ids().and_then(|(_, id)| app.session.get(id));
            let can_undo = doc.and_then(|d| d.can_undo()).is_some();
            let can_redo = doc.and_then(|d| d.can_redo()).is_some();
            let mut slots: Vec<Slot> = PenMode::ALL
                .iter()
                .map(|m| {
                    Slot::icon(m.icon(), m.label(), ink_on && pen_mode == *m, SlotAction::PenMode(*m)).marked(app.comment_prefs.pen_style(*m).color)
                })
                .collect();
            slots.extend([
                Slot::icon("eraser", "Eraser", app.quick_tool == QuickTool::Comment(CommentTool::Eraser), SlotAction::Tool(CommentTool::Eraser)),
                Slot::icon("lasso", "Lasso Select", app.quick_tool == QuickTool::Lasso, SlotAction::Lasso),
                Slot::icon("undo-2", "Undo", false, SlotAction::Undo).dimmed(!can_undo),
                Slot::icon("redo-2", "Redo", false, SlotAction::Redo).dimmed(!can_redo),
                Slot::icon("type", "Text Box", app.quick_tool == QuickTool::Comment(CommentTool::TextBox), SlotAction::Tool(CommentTool::TextBox)),
                Slot::icon("square", "Shapes…", false, SlotAction::SwitchMode(RadialMode::Shapes)),
                Slot::icon(
                    if text_mode { "type" } else { "sliders-horizontal" },
                    if text_mode { "Font size" } else { "Size & Opacity…" },
                    false,
                    SlotAction::SwitchMode(if text_mode { RadialMode::FontSize } else { RadialMode::Thickness }),
                ),
                Slot::icon("palette", "Palette & Custom…", false, SlotAction::SwitchMode(RadialMode::CustomColor)),
                Slot::icon("ellipsis", "More…", false, SlotAction::SwitchMode(RadialMode::More)),
            ]);
            slots
        }
        RadialMode::Colors => {
            let mut slots: Vec<Slot> = palette
                .iter()
                .map(|c| Slot::swatch(*c, palette_name(c), comments::same_colour(&current_style.color, c), SlotAction::SetColor(*c)))
                .collect();
            slots.push(Slot::icon("palette", "Palette & Custom…", false, SlotAction::SwitchMode(RadialMode::CustomColor)));
            slots
        }
        RadialMode::CustomColor => {
            let selected = app.comment_prefs.selected_palette_slot;
            palette.iter().enumerate().map(|(i, c)| Slot::swatch(*c, palette_name(c), i == selected, SlotAction::SelectPaletteSlot(i))).collect()
        }
        RadialMode::Thickness => {
            let (c, w, o) = (current_style.color, current_style.width, current_style.opacity);
            vec![
                Slot::line(1.0, c, "Fine (1 pt)", (w - 1.0).abs() < 0.25, SlotAction::SetWidth(1.0)),
                Slot::line(2.0, c, "Thin (2 pt)", (w - 2.0).abs() < 0.5, SlotAction::SetWidth(2.0)),
                Slot::line(3.0, c, "Normal (3 pt)", (w - 3.0).abs() < 0.5, SlotAction::SetWidth(3.0)),
                Slot::line(6.0, c, "Thick (6 pt)", (w - 6.0).abs() < 1.0, SlotAction::SetWidth(6.0)),
                Slot::line(12.0, c, "Marker (12 pt)", (w - 12.0).abs() < 1.5, SlotAction::SetWidth(12.0)),
                Slot::dot(11.0, c, 1.0, "Opaque (100%)", o >= 0.9, SlotAction::SetOpacity(1.0)),
                Slot::dot(11.0, c, 0.7, "Semi-Transparent (70%)", (o - 0.7).abs() < 0.15, SlotAction::SetOpacity(0.7)),
                Slot::dot(11.0, c, 0.4, "Translucent (40%)", o <= 0.5, SlotAction::SetOpacity(0.4)),
            ]
        }
        RadialMode::FontSize => {
            let cur = app.comment_prefs.text_box_font_size;
            [(9.0, "9 pt"), (12.0, "12 pt"), (14.0, "14 pt"), (18.0, "18 pt"), (24.0, "24 pt"), (36.0, "36 pt")]
                .into_iter()
                .map(|(size, label)| Slot::icon("type", label, (cur - size).abs() < 0.5, SlotAction::SetFontSize(size)))
                .chain(std::iter::once(Slot::icon("sliders-horizontal", "Stroke & Opacity…", false, SlotAction::SwitchMode(RadialMode::Thickness))))
                .collect()
        }
        RadialMode::Shapes => {
            let on = |tool| app.quick_tool == QuickTool::Comment(tool);
            vec![
                Slot::icon("square", "Rectangle", on(CommentTool::Rectangle), SlotAction::Tool(CommentTool::Rectangle)),
                Slot::icon("circle", "Oval", on(CommentTool::Oval), SlotAction::Tool(CommentTool::Oval)),
                Slot::icon("minus", "Line", on(CommentTool::Line), SlotAction::Tool(CommentTool::Line)),
                Slot::icon("move-right", "Arrow", on(CommentTool::Arrow), SlotAction::Tool(CommentTool::Arrow)),
                Slot::icon("pentagon", "Polygon", on(CommentTool::Polygon), SlotAction::Tool(CommentTool::Polygon)),
                Slot::icon("cloud", "Cloud", on(CommentTool::Cloud), SlotAction::Tool(CommentTool::Cloud)),
            ]
        }
        RadialMode::More => {
            let on = |tool| app.quick_tool == QuickTool::Comment(tool);
            vec![
                Slot::icon("mouse-pointer-2", "Select Tool", app.quick_tool == QuickTool::Select, SlotAction::SelectTool),
                Slot::icon("message-square-plus", "Sticky Note", on(CommentTool::Note), SlotAction::Tool(CommentTool::Note)),
                Slot::icon("message-square-quote", "Callout", on(CommentTool::Callout), SlotAction::Tool(CommentTool::Callout)),
                Slot::icon("type", "Font size", false, SlotAction::SwitchMode(RadialMode::FontSize)),
                Slot::icon("text-select", "Text Highlight", on(CommentTool::Highlight), SlotAction::Tool(CommentTool::Highlight)),
                Slot::icon("underline", "Underline", on(CommentTool::Underline), SlotAction::Tool(CommentTool::Underline)),
                Slot::icon("strikethrough", "Strikethrough", on(CommentTool::StrikeOut), SlotAction::Tool(CommentTool::StrikeOut)),
                Slot::icon(
                    "eraser",
                    if app.comment_prefs.stroke_eraser { "Whole-Stroke Eraser (On)" } else { "Partial Eraser (On)" },
                    app.comment_prefs.stroke_eraser,
                    SlotAction::ToggleStrokeEraser,
                ),
            ]
        }
    };

    let now = ui.input(|i| i.time);
    let mut picker_rect: Option<Rect> = None;
    let mut picked_custom = false;

    if menu.mode == RadialMode::CustomColor {
        let below = center.y + R_OUTER_OUT + 12.0 + 300.0 < avail.bottom();
        let (pivot, picker_pos) = if below {
            (egui::Align2::CENTER_TOP, center + vec2(0.0, R_OUTER_OUT * scale + 12.0))
        } else {
            (egui::Align2::CENTER_BOTTOM, center - vec2(0.0, R_OUTER_OUT * scale + 40.0))
        };
        let slot_idx = app.comment_prefs.selected_palette_slot.min(palette.len().saturating_sub(1));
        let slot_color = palette.get(slot_idx).copied().unwrap_or(current_style.color);
        let area_resp = egui::Area::new(egui::Id::new(("radial-color-picker-card", view_id)))
            .order(egui::Order::Tooltip)
            .pivot(pivot)
            .fixed_pos(picker_pos)
            .constrain_to(avail)
            .show(ui.ctx(), |ui| {
                *ui.visuals_mut() = egui::Visuals::dark();
                egui::Frame::popup(ui.style())
                    .corner_radius(14)
                    .fill(Color32::from_rgb(28, 28, 31))
                    .stroke(Stroke::new(1.0, Color32::from_white_alpha(40)))
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_max_width(220.0);
                        ui.label(egui::RichText::new(tl!("Change palette colour")).font(crate::theme::semibold(12.0)).color(Color32::WHITE));
                        ui.add_space(4.0);
                        if let Some(c) = comments::swatch_grid(ui, Some(slot_color)) {
                            app.comment_prefs.update_radial_palette(c);
                            app.comment_prefs.set_color(style_tool, c);
                            if !matches!(app.quick_tool, QuickTool::Comment(t) if t != CommentTool::Eraser) {
                                app.quick_tool = QuickTool::Comment(style_tool);
                            }
                            apply_style_to_selected(app, style_tool);
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.cancelled_drag = true;
                                if let Some(r) = v.comments.radial_menu.as_mut() {
                                    r.mode = RadialMode::Colors;
                                    r.opened_at = now;
                                }
                            }
                            picked_custom = true;
                        }
                    });
            });
        picker_rect = Some(area_resp.response.rect);
    }
    if picked_custom {
        return;
    }

    let mut action: Option<SlotAction> = None;
    let mut center_clicked = false;
    let mut close = false;

    egui::Area::new(egui::Id::new(("radial-menu-area", view_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .constrain(false)
        .interactable(false)
        .show(ui.ctx(), |ui| {
            let painter = ui.painter().clone();
            let a = |v: f32| (v * t).round().clamp(0.0, 255.0) as u8;
            let r_out = R_OUTER_OUT * scale;
            let ring_in = if inner.is_empty() { R_CENTER * scale + 6.0 } else { R_OUTER_IN * scale };

            // Pointer and hit test (the picker card, when shown, takes the pointer first).
            let pointer = ui.input(|i| i.pointer.latest_pos().or_else(|| i.pointer.hover_pos()));
            let press_origin = ui.input(|i| i.pointer.press_origin());
            let over_picker = picker_rect.is_some_and(|r| {
                let r = r.expand(6.0);
                pointer.is_some_and(|p| r.contains(p)) || press_origin.is_some_and(|o| r.contains(o)) || ui.ctx().egui_is_using_pointer()
            });
            let hovered = if over_picker { None } else { pointer.and_then(|p| hit_test(p, center, scale, inner.len(), outer.len())) };

            // 1. Body: soft shadow, dark glass disc, hairline border.
            for (grow, alpha) in [(8.0, 8.0), (5.0, 12.0), (2.5, 18.0)] {
                painter.circle_filled(center + vec2(0.0, 3.0), r_out + grow, Color32::from_black_alpha(a(alpha)));
            }
            painter.circle_filled(center, r_out, Color32::from_rgba_unmultiplied(24, 24, 27, a(244.0)));
            painter.circle_stroke(center, r_out, Stroke::new(1.0, Color32::from_white_alpha(a(34.0))));

            // 2. Outer ring: tools, or a sub-menu's choices.
            let n = outer.len();
            let step = std::f32::consts::TAU / n.max(1) as f32;
            for (k, slot) in outer.iter().enumerate() {
                let mid = wedge_angle(k, n);
                let (a0, a1) = (mid - step * 0.5 + GAP, mid + step * 0.5 - GAP);
                let is_hovered = hovered == Some(Hit::Outer(k));
                let fill = if slot.active {
                    Color32::from_rgba_unmultiplied(10, 132, 255, a(if is_hovered { 92.0 } else { 64.0 }))
                } else if is_hovered {
                    Color32::from_white_alpha(a(30.0))
                } else {
                    Color32::from_white_alpha(a(9.0))
                };
                painter.add(sector(center, ring_in, r_out - 3.0, a0, a1, fill));
                if slot.active {
                    painter.add(arc(center, r_out - 4.5, a0 + 0.02, a1 - 0.02, Stroke::new(3.0, ACCENT.gamma_multiply(t))));
                }
                let pos = center + vec2(mid.cos(), mid.sin()) * (ring_in + r_out - 3.0) * 0.5;
                paint_slot(ui, &painter, slot, pos, scale, t, is_hovered);
                if let Some(c) = slot.mark {
                    let at = center + vec2(mid.cos(), mid.sin()) * (ring_in + 9.0 * scale);
                    painter.circle_filled(at, 3.5 * scale, color32(c).gamma_multiply(t));
                    painter.circle_stroke(at, 3.5 * scale, Stroke::new(1.0, Color32::from_white_alpha(a(90.0))));
                }
                // An accessibility node per wedge: screen readers and UI tests find slots by name.
                let id = ui.id().with(("radial-slot", k));
                let r = ui.interact(Rect::from_center_size(pos, Vec2::splat(26.0 * scale)), id, egui::Sense::hover());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!(slot.label)));
            }

            // 3. Inner ring: the palette (main menu only).
            let m = inner.len();
            let istep = std::f32::consts::TAU / m.max(1) as f32;
            for (k, slot) in inner.iter().enumerate() {
                let mid = wedge_angle(k, m);
                let (a0, a1) = (mid - istep * 0.5 + GAP * 1.6, mid + istep * 0.5 - GAP * 1.6);
                let is_hovered = hovered == Some(Hit::Inner(k));
                let (r0, r1) = (R_INNER_IN * scale, R_INNER_OUT * scale + if is_hovered { 3.0 } else { 0.0 });
                let c = slot.color.map(color32).unwrap_or(Color32::GRAY);
                painter.add(sector(center, r0, r1, a0, a1, c.gamma_multiply(t)));
                if slot.active {
                    painter.add(arc(center, r1 + 2.5, a0, a1, Stroke::new(2.5, Color32::WHITE.gamma_multiply(t))));
                }
                let pos = center + vec2(mid.cos(), mid.sin()) * (r0 + r1) * 0.5;
                let id = ui.id().with(("radial-swatch", k));
                let r = ui.interact(Rect::from_center_size(pos, Vec2::splat(20.0 * scale)), id, egui::Sense::hover());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!(slot.label)));
            }

            // 4. Centre: a preview of the current pen (main menu) or Back (sub-menus).
            let rc = R_CENTER * scale;
            let center_hovered = hovered == Some(Hit::Center);
            painter.circle_filled(center, rc, Color32::from_rgba_unmultiplied(38, 38, 42, a(if center_hovered { 255.0 } else { 235.0 })));
            painter.circle_stroke(center, rc, Stroke::new(1.0, Color32::from_white_alpha(a(if center_hovered { 70.0 } else { 30.0 }))));
            let center_label = if menu.mode == RadialMode::Tools {
                let width = (current_style.width as f32 * 1.4).clamp(1.5, 14.0) * scale;
                let colour = color32(current_style.color).gamma_multiply(t * current_style.opacity.clamp(0.1, 1.0) as f32);
                let pts: Vec<Pos2> = (0..=24)
                    .map(|i| {
                        let u = i as f32 / 24.0;
                        center + vec2((u - 0.5) * rc * 1.15, (u * std::f32::consts::TAU).sin() * rc * 0.22)
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, Stroke::new(width, colour)));
                "Close"
            } else {
                icons::paint(
                    ui,
                    Rect::from_center_size(center, Vec2::splat(20.0 * scale)),
                    "chevron-left",
                    20.0 * scale,
                    Color32::from_gray(235).gamma_multiply(t),
                );
                "Back"
            };
            let r = ui.interact(Rect::from_center_size(center, Vec2::splat(rc * 1.2)), ui.id().with("radial-center"), egui::Sense::hover());
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!(center_label)));

            // 5. Label of what the pointer is over, above the wheel.
            let label = match hovered {
                Some(Hit::Outer(k)) => outer.get(k).map(|s| s.label),
                Some(Hit::Inner(k)) => inner.get(k).map(|s| s.label),
                Some(Hit::Center) => Some(center_label),
                None => None,
            };
            if let Some(label) = label
                && menu.mode != RadialMode::CustomColor
            {
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(tl!(label).to_string(), crate::theme::semibold(12.0), Color32::WHITE));
                let pos = center - vec2(0.0, r_out + 20.0);
                let badge = Rect::from_center_size(pos, galley.size() + vec2(18.0, 10.0));
                painter.rect_filled(badge, egui::CornerRadius::same(12), Color32::from_rgba_unmultiplied(24, 24, 27, a(245.0)));
                painter.rect_stroke(
                    badge,
                    egui::CornerRadius::same(12),
                    Stroke::new(1.0, Color32::from_white_alpha(a(34.0))),
                    egui::StrokeKind::Outside,
                );
                painter.galley(badge.center() - galley.size() * 0.5, galley, Color32::WHITE);
            }

            // 6. Clicks and dismissal.
            let ready = now > menu.opened_at;
            let primary = ui.input(|i| i.pointer.primary_clicked() || i.pointer.button_released(egui::PointerButton::Primary));
            if ready && !over_picker && primary {
                match hovered {
                    Some(Hit::Outer(k)) => action = outer.get(k).map(|s| s.action),
                    Some(Hit::Inner(k)) => action = inner.get(k).map(|s| s.action),
                    Some(Hit::Center) => center_clicked = true,
                    None => close = pointer.is_some_and(|p| p.distance(center) > r_out + 10.0),
                }
            }
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            let secondary = ready && ui.input(|i| i.pointer.secondary_clicked() || i.pointer.button_released(egui::PointerButton::Secondary));
            close |= escape || secondary;
        });

    let set_mode = |app: &mut PdfCraftApp, mode: RadialMode| {
        if let Some(r) = app.views.get_mut(view_index).and_then(|v| v.comments.radial_menu.as_mut()) {
            r.mode = mode;
            r.opened_at = now;
        }
    };
    let finish = |app: &mut PdfCraftApp| {
        if let Some(v) = app.views.get_mut(view_index) {
            v.comments.radial_menu = None;
        }
    };
    let style_tool_on = |app: &mut PdfCraftApp| {
        if !matches!(app.quick_tool, QuickTool::Comment(t) if t != CommentTool::Eraser) {
            app.quick_tool = QuickTool::Comment(style_tool);
        }
    };

    if (action.is_some() || center_clicked || close)
        && let Some(v) = app.views.get_mut(view_index)
    {
        v.comments.cancelled_drag = true;
    }
    if center_clicked {
        if menu.mode == RadialMode::Tools {
            finish(app);
        } else {
            set_mode(app, menu.mode.back());
        }
        return;
    }
    let Some(action) = action else {
        if close {
            finish(app);
        }
        return;
    };
    match action {
        SlotAction::PenMode(mode) => {
            app.comment_prefs.activate_pen_mode(mode);
            app.quick_tool = QuickTool::Comment(CommentTool::Ink);
            app.comment_prefs.group_tool[CommentTool::Ink.group()] = CommentTool::Ink;
            finish(app);
        }
        SlotAction::Lasso => {
            app.quick_tool = QuickTool::Lasso;
            finish(app);
        }
        SlotAction::Tool(tool) => {
            app.quick_tool = QuickTool::Comment(tool);
            app.comment_prefs.group_tool[tool.group()] = tool;
            finish(app);
        }
        SlotAction::SelectTool => {
            app.quick_tool = QuickTool::Select;
            finish(app);
        }
        SlotAction::Undo => {
            app.execute("edit.undo");
        }
        SlotAction::Redo => {
            app.execute("edit.redo");
        }
        SlotAction::SwitchMode(mode) => set_mode(app, mode),
        SlotAction::SetColor(color) => {
            app.comment_prefs.set_color(style_tool, color);
            style_tool_on(app);
            apply_style_to_selected(app, style_tool);
            finish(app);
        }
        SlotAction::SelectPaletteSlot(slot) => {
            let slot = slot.min(palette.len().saturating_sub(1));
            app.comment_prefs.selected_palette_slot = slot;
            if let Some(c) = palette.get(slot) {
                app.comment_prefs.set_color(style_tool, *c);
            }
            set_mode(app, RadialMode::CustomColor);
        }
        SlotAction::SetWidth(w) => {
            app.comment_prefs.set_width(style_tool, w);
            style_tool_on(app);
            apply_style_to_selected(app, style_tool);
            finish(app);
        }
        SlotAction::SetOpacity(o) => {
            app.comment_prefs.set_opacity(style_tool, o);
            style_tool_on(app);
            apply_style_to_selected(app, style_tool);
            finish(app);
        }
        SlotAction::SetFontSize(size) => {
            app.comment_prefs.text_box_font_size = size;
            if let Some((index, id)) = app.active_ids()
                && let Some((page, annotation)) = app.views.get(index).and_then(|view| view.comments.selected)
                && app.session.get(id).is_some_and(|doc| doc.allows_annotation())
            {
                app.apply_edit(pdfcraft_engine::Edit::FontSizeAnnotation { page, index: annotation, size });
            }
            finish(app);
        }
        SlotAction::ToggleStrokeEraser => {
            app.comment_prefs.stroke_eraser = !app.comment_prefs.stroke_eraser;
            app.quick_tool = QuickTool::Comment(CommentTool::Eraser);
            finish(app);
        }
    }
}

/// A slot's content: an icon, a colour swatch or a size dot, centred on `pos`.
fn paint_slot(ui: &egui::Ui, painter: &egui::Painter, slot: &Slot, pos: Pos2, scale: f32, t: f32, hovered: bool) {
    let lift = if hovered { 1.12 } else { 1.0 };
    if let Some(w) = slot.line_width {
        let c = slot.color.unwrap_or([1.0, 1.0, 1.0]);
        let half = 13.0 * scale * lift;
        let pts: Vec<Pos2> = (0..=12)
            .map(|i| {
                let u = i as f32 / 12.0;
                pos + vec2((u - 0.5) * 2.0 * half, (u * std::f32::consts::TAU).sin() * 3.0 * scale)
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new((w * 1.2).clamp(1.0, 14.0) * scale, color32(c).gamma_multiply(t * slot.alpha))));
    } else if let Some(r) = slot.dot_radius {
        let c = slot.color.unwrap_or([1.0, 1.0, 1.0]);
        painter.circle_filled(pos, r * scale * lift, color32(c).gamma_multiply(t * slot.alpha));
        painter.circle_stroke(pos, r * scale * lift, Stroke::new(1.0, Color32::from_white_alpha((120.0 * t) as u8)));
    } else if let Some(c) = slot.color {
        painter.circle_filled(pos, 10.0 * scale * lift, color32(c).gamma_multiply(t));
        painter.circle_stroke(
            pos,
            10.0 * scale * lift,
            Stroke::new(if slot.active { 2.0 } else { 1.0 }, Color32::from_white_alpha((if slot.active { 230.0 } else { 70.0 } * t) as u8)),
        );
    } else if let Some(icon) = slot.icon {
        let tint = if slot.active { Color32::WHITE.gamma_multiply(t) } else { Color32::from_gray(225).gamma_multiply(t * slot.alpha) };
        let size = 20.0 * scale * lift;
        icons::paint(ui, Rect::from_center_size(pos, Vec2::splat(size)), icon, size, tint);
    }
}

fn apply_style_to_selected(app: &mut PdfCraftApp, tool: CommentTool) {
    let style = app.comment_prefs.style(tool);
    if let Some((index, id)) = app.active_ids()
        && let Some(view) = app.views.get(index)
        && let Some((page, annotation)) = view.comments.selected
        && app.session.get(id).is_some_and(|doc| doc.allows_annotation())
    {
        let also = view.comments.also.clone();
        if also.is_empty() {
            app.apply_edit(pdfcraft_engine::Edit::StyleAnnotation {
                page,
                index: annotation,
                color: Some(style.color),
                opacity: Some(style.opacity),
                width: tool.has_width().then_some(style.width),
                endings: None,
            });
        } else {
            let mut edits: Vec<pdfcraft_engine::Edit> = also
                .into_iter()
                .chain(std::iter::once(annotation))
                .map(|i| pdfcraft_engine::Edit::StyleAnnotation {
                    page,
                    index: i,
                    color: Some(style.color),
                    opacity: Some(style.opacity),
                    width: tool.has_width().then_some(style.width),
                    endings: None,
                })
                .collect();
            edits.dedup_by_key(|e| match e {
                pdfcraft_engine::Edit::StyleAnnotation { index, .. } => *index,
                _ => usize::MAX,
            });
            app.apply_edit(pdfcraft_engine::Edit::Batch { label: "Style".into(), edits });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wedges_start_at_the_top_and_go_clockwise() {
        assert_eq!(wedge_at(vec2(0.0, -1.0), 4), Some(0));
        assert_eq!(wedge_at(vec2(1.0, 0.0), 4), Some(1));
        assert_eq!(wedge_at(vec2(0.0, 1.0), 4), Some(2));
        assert_eq!(wedge_at(vec2(-1.0, 0.0), 4), Some(3));
        // Just left of the top still belongs to wedge 0, not the last one.
        assert_eq!(wedge_at(vec2(-0.1, -1.0), 4), Some(0));
        for n in [1, 3, 8, 13] {
            for k in 0..n {
                let a = wedge_angle(k, n);
                assert_eq!(wedge_at(vec2(a.cos(), a.sin()), n), Some(k), "{k}/{n}");
            }
        }
    }

    #[test]
    fn hit_testing_is_total() {
        assert_eq!(wedge_at(vec2(1.0, 0.0), 0), None);
        assert_eq!(wedge_at(vec2(f32::NAN, 0.0), 4), None);
        let c = egui::pos2(200.0, 200.0);
        assert_eq!(hit_test(c, c, 1.0, 8, 13), Some(Hit::Center));
        assert_eq!(hit_test(c + vec2(0.0, -50.0), c, 1.0, 8, 13), Some(Hit::Inner(0)));
        assert_eq!(hit_test(c + vec2(0.0, -100.0), c, 1.0, 8, 13), Some(Hit::Outer(0)));
        assert_eq!(hit_test(c + vec2(0.0, -50.0), c, 1.0, 0, 6), Some(Hit::Outer(0)), "no inner ring: the outer slots reach inwards");
        assert_eq!(hit_test(c + vec2(0.0, -300.0), c, 1.0, 8, 13), None);
        assert_eq!(hit_test(egui::pos2(f32::NAN, 0.0), c, 1.0, 8, 13), None);
    }

    #[test]
    fn the_wheel_stays_inside_the_view() {
        let avail = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(800.0, 600.0));
        let p = clamp_center(egui::pos2(5.0, 5.0), avail);
        assert!(p.x - R_OUTER_OUT >= 0.0 && p.y - R_OUTER_OUT - 30.0 >= 0.0, "{p:?}");
        let p = clamp_center(egui::pos2(795.0, 595.0), avail);
        assert!(p.x + R_OUTER_OUT <= 800.0 && p.y + R_OUTER_OUT <= 600.0, "{p:?}");
        // A view smaller than the wheel centres it instead of panicking in clamp.
        let tiny = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(100.0, 100.0));
        assert_eq!(clamp_center(egui::pos2(10.0, 10.0), tiny), egui::pos2(50.0, 50.0));
    }
}
