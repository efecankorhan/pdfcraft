//! Apple-like curved arc radial menu for graphic tablets, stylus, and handwriting tools.
//!
//! Activated on canvas secondary click / stylus barrel click. Offers quick vector ink,
//! highlighter, eraser, shapes, undo/redo, color swatches, and stroke thickness.

use egui::{Color32, Mesh, Pos2, Rect, Stroke, Vec2, vec2};
use pdfcraft_engine::Rgb;

use crate::comments::{self, CommentTool, color32};
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
    Thickness,
    FontSize,
    Shapes,
    More,
}

struct Slot {
    icon: Option<&'static str>,
    color: Option<Rgb>,
    dot_radius: Option<f32>,
    alpha: f32,
    label: &'static str,
    active: bool,
    action: SlotAction,
}

impl Slot {
    fn icon(icon: &'static str, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: Some(icon), color: None, dot_radius: None, alpha: 1.0, label, active, action }
    }

    fn swatch(color: Rgb, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: None, color: Some(color), dot_radius: None, alpha: 1.0, label, active, action }
    }

    fn dot(radius: f32, color: Rgb, alpha: f32, label: &'static str, active: bool, action: SlotAction) -> Self {
        Self { icon: None, color: Some(color), dot_radius: Some(radius), alpha, label, active, action }
    }
}

enum SlotAction {
    Pen,
    FreehandHighlighter,
    Tool(CommentTool),
    SelectTool,
    Undo,
    Redo,
    SwitchMode(RadialMode),
    SetColor(Rgb),
    SetWidth(f64),
    SetOpacity(f64),
    SetFontSize(f64),
    ToggleStrokeEraser,
}

pub(crate) fn show(app: &mut PdfCraftApp, view_index: usize, ui: &mut egui::Ui, avail: Rect) {
    let (radial, view_id) = match app.views.get(view_index) {
        Some(v) => (v.comments.radial_menu.clone(), v.id.0),
        None => return,
    };
    let is_open = radial.is_some();

    // Smooth Apple-style cubic easing for radial menu appearance.
    let anim_id = egui::Id::new(("radial-menu-anim", view_id));
    let t = ui.ctx().animate_bool_with_time_and_easing(anim_id, is_open, 0.20, egui::emath::easing::cubic_out);

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

    let center = menu.screen_pos;

    // Determine optimal arc angle orientation based on viewport boundaries.
    let flips_down = center.y - 140.0 < avail.top();
    let (start_deg, end_deg) = if flips_down {
        (15.0f32, 165.0f32)
    } else {
        (-165.0f32, 15.0f32)
    };

    let start_rad = start_deg.to_radians();
    let end_rad = end_deg.to_radians();

    let scale = 0.78 + 0.22 * t;
    let r_in = 78.0 * scale;
    let r_out = 124.0 * scale;
    let r_mid = (r_in + r_out) * 0.5;
    let half_thick = (r_out - r_in) * 0.5;

    // Tool targeted by style adjustments (Colors, Size & Opacity).
    // When Eraser or Select is active, styling targets Ink so changes apply to Pen/Highlighter.
    let style_tool = match app.quick_tool {
        QuickTool::Comment(t) if t != CommentTool::Eraser => t,
        _ => CommentTool::Ink,
    };
    let current_style = app.comment_prefs.style(style_tool);
    let is_highlighter = app.comment_prefs.is_freehand_highlighter();
    let text_mode = matches!(app.quick_tool, QuickTool::Comment(CommentTool::TextBox | CommentTool::Callout));

    let slots = match menu.mode {
        RadialMode::Tools => {
            let can_undo = app.active_ids().and_then(|(_, id)| app.session.get(id)).and_then(|d| d.can_undo()).is_some();
            let can_redo = app.active_ids().and_then(|(_, id)| app.session.get(id)).and_then(|d| d.can_redo()).is_some();
            let mut undo_slot = Slot::icon("undo-2", "Undo", false, SlotAction::Undo);
            if !can_undo {
                undo_slot.alpha = 0.35;
            }
            let mut redo_slot = Slot::icon("redo-2", "Redo", false, SlotAction::Redo);
            if !can_redo {
                redo_slot.alpha = 0.35;
            }
            vec![
                undo_slot,
                redo_slot,
                Slot::icon(
                    "eraser",
                    "Eraser",
                    app.quick_tool == QuickTool::Comment(CommentTool::Eraser),
                    SlotAction::Tool(CommentTool::Eraser),
                ),
            Slot::icon(
                "pencil",
                "Pen",
                app.quick_tool == QuickTool::Comment(CommentTool::Ink) && !is_highlighter,
                SlotAction::Pen,
            ),
            Slot::icon(
                "highlighter",
                "Highlighter",
                app.quick_tool == QuickTool::Comment(CommentTool::Highlight)
                    || (app.quick_tool == QuickTool::Comment(CommentTool::Ink) && is_highlighter),
                SlotAction::FreehandHighlighter,
            ),
            Slot::icon(
                "type",
                "Text Box",
                app.quick_tool == QuickTool::Comment(CommentTool::TextBox),
                SlotAction::Tool(CommentTool::TextBox),
            ),
            Slot::icon("square", "Shapes…", false, SlotAction::SwitchMode(RadialMode::Shapes)),
            Slot::swatch(current_style.color, "Colors…", false, SlotAction::SwitchMode(RadialMode::Colors)),
            Slot::icon(
                if text_mode { "type" } else { "sliders-horizontal" },
                if text_mode { "Font Size (Punto)…" } else { "Size & Opacity…" },
                false,
                SlotAction::SwitchMode(if text_mode { RadialMode::FontSize } else { RadialMode::Thickness }),
            ),
            Slot::icon("ellipsis", "More…", false, SlotAction::SwitchMode(RadialMode::More)),
        ]},
        RadialMode::Colors => vec![
            Slot::icon("chevron-left", "Back", false, SlotAction::SwitchMode(RadialMode::Tools)),
            Slot::swatch([0.0, 0.0, 0.0], "Black", comments::same_colour(&current_style.color, &[0.0, 0.0, 0.0]), SlotAction::SetColor([0.0, 0.0, 0.0])),
            Slot::swatch([0.0, 0.47, 0.84], "Blue", comments::same_colour(&current_style.color, &[0.0, 0.47, 0.84]), SlotAction::SetColor([0.0, 0.47, 0.84])),
            Slot::swatch([0.36, 0.75, 0.98], "Cyan", comments::same_colour(&current_style.color, &[0.36, 0.75, 0.98]), SlotAction::SetColor([0.36, 0.75, 0.98])),
            Slot::swatch([0.18, 0.62, 0.36], "Green", comments::same_colour(&current_style.color, &[0.18, 0.62, 0.36]), SlotAction::SetColor([0.18, 0.62, 0.36])),
            Slot::swatch([1.0, 0.94, 0.0], "Yellow", comments::same_colour(&current_style.color, &[1.0, 0.94, 0.0]), SlotAction::SetColor([1.0, 0.94, 0.0])),
            Slot::swatch([1.0, 0.54, 0.0], "Orange", comments::same_colour(&current_style.color, &[1.0, 0.54, 0.0]), SlotAction::SetColor([1.0, 0.54, 0.0])),
            Slot::swatch([0.89, 0.13, 0.13], "Red", comments::same_colour(&current_style.color, &[0.89, 0.13, 0.13]), SlotAction::SetColor([0.89, 0.13, 0.13])),
            Slot::swatch([0.54, 0.25, 0.82], "Purple", comments::same_colour(&current_style.color, &[0.54, 0.25, 0.82]), SlotAction::SetColor([0.54, 0.25, 0.82])),
        ],
        RadialMode::Thickness => vec![
            Slot::icon("chevron-left", "Back", false, SlotAction::SwitchMode(RadialMode::Tools)),
            Slot::dot(2.5, current_style.color, 1.0, "Fine (1.5 pt)", (current_style.width - 1.5).abs() < 0.5, SlotAction::SetWidth(1.5)),
            Slot::dot(4.2, current_style.color, 1.0, "Normal (3.0 pt)", (current_style.width - 3.0).abs() < 0.5, SlotAction::SetWidth(3.0)),
            Slot::dot(6.2, current_style.color, 1.0, "Thick (6.0 pt)", (current_style.width - 6.0).abs() < 0.5, SlotAction::SetWidth(6.0)),
            Slot::dot(9.0, current_style.color, 1.0, "Marker (12 pt)", (current_style.width - 12.0).abs() < 1.5, SlotAction::SetWidth(12.0)),
            Slot::dot(6.5, current_style.color, 1.0, "Opaque (100%)", current_style.opacity >= 0.9, SlotAction::SetOpacity(1.0)),
            Slot::dot(6.5, current_style.color, 0.7, "Semi-Transparent (70%)", (current_style.opacity - 0.7).abs() < 0.15, SlotAction::SetOpacity(0.7)),
            Slot::dot(6.5, current_style.color, 0.4, "Highlighter (40%)", current_style.opacity <= 0.5, SlotAction::SetOpacity(0.4)),
            Slot::icon("type", "Font Size (Punto)…", false, SlotAction::SwitchMode(RadialMode::FontSize)),
        ],
        RadialMode::FontSize => {
            let cur = app.comment_prefs.text_box_font_size;
            vec![
                Slot::icon("chevron-left", "Back", false, SlotAction::SwitchMode(RadialMode::Tools)),
                Slot::icon("type", "9 pt", (cur - 9.0).abs() < 0.5, SlotAction::SetFontSize(9.0)),
                Slot::icon("type", "12 pt", (cur - 12.0).abs() < 0.5, SlotAction::SetFontSize(12.0)),
                Slot::icon("type", "14 pt", (cur - 14.0).abs() < 0.5, SlotAction::SetFontSize(14.0)),
                Slot::icon("type", "18 pt", (cur - 18.0).abs() < 0.5, SlotAction::SetFontSize(18.0)),
                Slot::icon("type", "24 pt", (cur - 24.0).abs() < 0.5, SlotAction::SetFontSize(24.0)),
                Slot::icon("type", "36 pt", (cur - 36.0).abs() < 0.5, SlotAction::SetFontSize(36.0)),
                Slot::icon("sliders-horizontal", "Stroke & Opacity…", false, SlotAction::SwitchMode(RadialMode::Thickness)),
            ]
        }
        RadialMode::Shapes => vec![
            Slot::icon("chevron-left", "Back", false, SlotAction::SwitchMode(RadialMode::Tools)),
            Slot::icon("square", "Rectangle", app.quick_tool == QuickTool::Comment(CommentTool::Rectangle), SlotAction::Tool(CommentTool::Rectangle)),
            Slot::icon("circle", "Oval", app.quick_tool == QuickTool::Comment(CommentTool::Oval), SlotAction::Tool(CommentTool::Oval)),
            Slot::icon("minus", "Line", app.quick_tool == QuickTool::Comment(CommentTool::Line), SlotAction::Tool(CommentTool::Line)),
            Slot::icon("move-right", "Arrow", app.quick_tool == QuickTool::Comment(CommentTool::Arrow), SlotAction::Tool(CommentTool::Arrow)),
            Slot::icon("pentagon", "Polygon", app.quick_tool == QuickTool::Comment(CommentTool::Polygon), SlotAction::Tool(CommentTool::Polygon)),
            Slot::icon("cloud", "Cloud", app.quick_tool == QuickTool::Comment(CommentTool::Cloud), SlotAction::Tool(CommentTool::Cloud)),
        ],
        RadialMode::More => vec![
            Slot::icon("chevron-left", "Back", false, SlotAction::SwitchMode(RadialMode::Tools)),
            Slot::icon("mouse-pointer", "Select Tool", app.quick_tool == QuickTool::Select, SlotAction::SelectTool),
            Slot::icon("message-square-plus", "Sticky Note", app.quick_tool == QuickTool::Comment(CommentTool::Note), SlotAction::Tool(CommentTool::Note)),
            Slot::icon("message-square-quote", "Callout", app.quick_tool == QuickTool::Comment(CommentTool::Callout), SlotAction::Tool(CommentTool::Callout)),
            Slot::icon("type", "Font Size (Punto)…", false, SlotAction::SwitchMode(RadialMode::FontSize)),
            Slot::icon("highlighter", "Text Highlight", app.quick_tool == QuickTool::Comment(CommentTool::Highlight), SlotAction::Tool(CommentTool::Highlight)),
            Slot::icon("underline", "Underline", app.quick_tool == QuickTool::Comment(CommentTool::Underline), SlotAction::Tool(CommentTool::Underline)),
            Slot::icon("strikethrough", "Strikethrough", app.quick_tool == QuickTool::Comment(CommentTool::StrikeOut), SlotAction::Tool(CommentTool::StrikeOut)),
            Slot::icon(
                "eraser",
                if app.comment_prefs.stroke_eraser { "Whole-Stroke Eraser (On)" } else { "Partial Eraser (On)" },
                app.comment_prefs.stroke_eraser,
                SlotAction::ToggleStrokeEraser,
            ),
        ],
    };

    egui::Area::new(egui::Id::new(("radial-menu-area", view_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .constrain(false)
        .interactable(false)
        .show(ui.ctx(), |ui| {
            let painter = ui.painter();

            // 1. Paint Smooth Arc Ribbon Background (Continuous rounded contour, no square shadow, Apple obsidian dark glass)
            let mut bg_mesh = Mesh::default();
            let arc_steps = 40;
            let cap_steps = 12;
            let bg_color = Color32::from_rgba_unmultiplied(22, 22, 24, (246.0 * t) as u8);
            let border_color = Color32::from_white_alpha((50.0 * t) as u8);

            let start_cap_center = center + vec2(start_rad.cos(), start_rad.sin()) * r_mid;
            let end_cap_center = center + vec2(end_rad.cos(), end_rad.sin()) * r_mid;

            let mut outer_pts = Vec::with_capacity(arc_steps + 1);
            let mut inner_pts = Vec::with_capacity(arc_steps + 1);
            let mut contour = Vec::with_capacity(arc_steps * 2 + cap_steps * 2);

            for k in 0..=arc_steps {
                let frac = k as f32 / arc_steps as f32;
                let angle = start_rad + (end_rad - start_rad) * frac;
                let dir = vec2(angle.cos(), angle.sin());
                let p_out = center + dir * r_out;
                let p_in = center + dir * r_in;
                outer_pts.push(p_out);
                inner_pts.push(p_in);
                contour.push(p_out);
            }

            // End cap rounded semicircle outward bulge
            for s in 1..cap_steps {
                let a = end_rad + std::f32::consts::PI * (s as f32 / cap_steps as f32);
                contour.push(end_cap_center + vec2(a.cos(), a.sin()) * half_thick);
            }

            // Inner arc returning to start
            for k in (0..=arc_steps).rev() {
                contour.push(inner_pts[k]);
            }

            // Start cap rounded semicircle outward bulge
            for s in 1..cap_steps {
                let a = start_rad + std::f32::consts::PI + std::f32::consts::PI * (s as f32 / cap_steps as f32);
                contour.push(start_cap_center + vec2(a.cos(), a.sin()) * half_thick);
            }

            // Connect arc quads
            for k in 0..arc_steps {
                let base = bg_mesh.vertices.len() as u32;
                bg_mesh.colored_vertex(outer_pts[k], bg_color);
                bg_mesh.colored_vertex(outer_pts[k + 1], bg_color);
                bg_mesh.colored_vertex(inner_pts[k + 1], bg_color);
                bg_mesh.colored_vertex(inner_pts[k], bg_color);
                bg_mesh.add_triangle(base, base + 1, base + 2);
                bg_mesh.add_triangle(base, base + 2, base + 3);
            }

            // Triangulate end cap
            let end_idx = bg_mesh.vertices.len() as u32;
            bg_mesh.colored_vertex(end_cap_center, bg_color);
            let p_out_end = *outer_pts.last().unwrap_or(&center);
            let p_in_end = *inner_pts.last().unwrap_or(&center);
            bg_mesh.colored_vertex(p_out_end, bg_color);
            for s in 1..cap_steps {
                let a = end_rad + std::f32::consts::PI * (s as f32 / cap_steps as f32);
                bg_mesh.colored_vertex(end_cap_center + vec2(a.cos(), a.sin()) * half_thick, bg_color);
            }
            bg_mesh.colored_vertex(p_in_end, bg_color);
            for s in 0..cap_steps {
                bg_mesh.add_triangle(end_idx, end_idx + 1 + s as u32, end_idx + 2 + s as u32);
            }

            // Triangulate start cap
            let start_idx = bg_mesh.vertices.len() as u32;
            bg_mesh.colored_vertex(start_cap_center, bg_color);
            let p_in_start = inner_pts[0];
            let p_out_start = outer_pts[0];
            bg_mesh.colored_vertex(p_in_start, bg_color);
            for s in 1..cap_steps {
                let a = start_rad + std::f32::consts::PI + std::f32::consts::PI * (s as f32 / cap_steps as f32);
                bg_mesh.colored_vertex(start_cap_center + vec2(a.cos(), a.sin()) * half_thick, bg_color);
            }
            bg_mesh.colored_vertex(p_out_start, bg_color);
            for s in 0..cap_steps {
                bg_mesh.add_triangle(start_idx, start_idx + 1 + s as u32, start_idx + 2 + s as u32);
            }

            // Paint background mesh & smooth continuous closed border (no square shadow)
            painter.add(egui::Shape::mesh(bg_mesh));
            painter.add(egui::Shape::closed_line(contour, Stroke::new(1.0, border_color)));

            // 2. Slots & Buttons Along the Arc (single-nearest-slot hit test so neighbors never overlap)
            let num_slots = slots.len();
            let pointer = ui.input(|inp| inp.pointer.latest_pos().or_else(|| inp.pointer.hover_pos()));
            let btn_radius = 15.0 * scale;

            let slot_positions: Vec<Pos2> = (0..num_slots)
                .map(|idx| {
                    let frac = (idx as f32 + 0.5) / num_slots as f32;
                    let angle = start_rad + (end_rad - start_rad) * frac;
                    center + vec2(angle.cos(), angle.sin()) * r_mid
                })
                .collect();

            let hovered_idx = pointer.and_then(|p| {
                let d_center = p.distance(center);
                if d_center < r_in - 12.0 || d_center > r_out + 12.0 {
                    return None;
                }
                slot_positions
                    .iter()
                    .enumerate()
                    .map(|(idx, pos)| (idx, p.distance(*pos)))
                    .filter(|(_, dist)| *dist <= half_thick + 4.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(idx, _)| idx)
            });

            let now = ui.input(|i| i.time);
            let ready = now - menu.opened_at > 0.08;
            let clicked = ready
                && ui.input(|inp| {
                    inp.pointer.primary_clicked() || inp.pointer.button_released(egui::PointerButton::Primary)
                });

            let triggered_action = if clicked { hovered_idx } else { None };
            let mut hovered_label = None;

            for (idx, slot) in slots.iter().enumerate() {
                let slot_pos = slot_positions[idx];
                let is_hovered = hovered_idx == Some(idx);

                if is_hovered {
                    hovered_label = Some(slot.label);
                    painter.circle_filled(slot_pos, btn_radius, Color32::from_white_alpha((44.0 * t) as u8));
                }

                if slot.active {
                    painter.circle_stroke(slot_pos, btn_radius + 1.0, Stroke::new(2.0, Color32::from_rgb(10, 132, 255).gamma_multiply(t)));
                }

                if let Some(r) = slot.dot_radius {
                    let c = slot.color.unwrap_or([1.0, 1.0, 1.0]);
                    let fill = color32(c).gamma_multiply(t * slot.alpha);
                    painter.circle_filled(slot_pos, r * scale, fill);
                    painter.circle_stroke(slot_pos, r * scale, Stroke::new(1.0, Color32::from_white_alpha((130.0 * t) as u8)));
                } else if let Some(c) = slot.color {
                    painter.circle_filled(slot_pos, 9.0 * scale, color32(c).gamma_multiply(t));
                    painter.circle_stroke(slot_pos, 9.0 * scale, Stroke::new(1.0, Color32::from_white_alpha((80.0 * t) as u8)));
                } else if let Some(icon_name) = slot.icon {
                    let icon_color = if slot.active { Color32::from_rgb(10, 132, 255) } else { Color32::from_gray(235).gamma_multiply(t * slot.alpha) };
                    icons::paint(ui, Rect::from_center_size(slot_pos, Vec2::splat(18.0 * scale)), icon_name, 18.0 * scale, icon_color);
                }
            }

            // Show hovered label above/below the arc
            if let Some(label) = hovered_label {
                let label_pos = center + vec2(0.0, if flips_down { r_out + 18.0 } else { -(r_out + 18.0) });
                let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), crate::theme::semibold(11.5), Color32::WHITE));
                let badge_rect = Rect::from_center_size(label_pos, galley.size() + vec2(14.0, 8.0));
                painter.rect_filled(badge_rect, egui::CornerRadius::same(10), Color32::from_rgba_unmultiplied(26, 26, 28, (245.0 * t) as u8));
                painter.rect_stroke(badge_rect, egui::CornerRadius::same(10), Stroke::new(1.0, border_color), egui::StrokeKind::Outside);
                painter.galley(badge_rect.center() - galley.size() * 0.5, galley, Color32::WHITE);
            }

            // 3. Handle Click / Dismissal
            if let Some(idx) = triggered_action {
                if let Some(slot) = slots.get(idx) {
                    match slot.action {
                        SlotAction::Pen => {
                            app.comment_prefs.activate_pen();
                            app.quick_tool = QuickTool::Comment(CommentTool::Ink);
                            app.comment_prefs.group_tool[CommentTool::Ink.group()] = CommentTool::Ink;
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::FreehandHighlighter => {
                            app.comment_prefs.activate_freehand_highlighter();
                            app.quick_tool = QuickTool::Comment(CommentTool::Ink);
                            app.comment_prefs.group_tool[CommentTool::Ink.group()] = CommentTool::Ink;
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::Tool(tool) => {
                            app.quick_tool = QuickTool::Comment(tool);
                            app.comment_prefs.group_tool[tool.group()] = tool;
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::SelectTool => {
                            app.quick_tool = QuickTool::Select;
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::Undo => {
                            app.execute("edit.undo");
                        }
                        SlotAction::Redo => {
                            app.execute("edit.redo");
                        }
                        SlotAction::SwitchMode(new_mode) => {
                            if let Some(v) = app.views.get_mut(view_index)
                                && let Some(r) = v.comments.radial_menu.as_mut()
                            {
                                r.mode = new_mode;
                            }
                        }
                        SlotAction::SetColor(color) => {
                            app.comment_prefs.set_color(style_tool, color);
                            if !matches!(app.quick_tool, QuickTool::Comment(t) if t != CommentTool::Eraser) {
                                app.quick_tool = QuickTool::Comment(style_tool);
                            }
                            apply_style_to_selected(app, style_tool);
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::SetWidth(w) => {
                            app.comment_prefs.set_width(style_tool, w);
                            if !matches!(app.quick_tool, QuickTool::Comment(t) if t != CommentTool::Eraser) {
                                app.quick_tool = QuickTool::Comment(style_tool);
                            }
                            apply_style_to_selected(app, style_tool);
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::SetOpacity(o) => {
                            app.comment_prefs.set_opacity(style_tool, o);
                            if !matches!(app.quick_tool, QuickTool::Comment(t) if t != CommentTool::Eraser) {
                                app.quick_tool = QuickTool::Comment(style_tool);
                            }
                            apply_style_to_selected(app, style_tool);
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::SetFontSize(size) => {
                            app.comment_prefs.text_box_font_size = size;
                            if let Some((index, id)) = app.active_ids()
                                && let Some((page, annotation)) = app.views.get(index).and_then(|view| view.comments.selected)
                                && app.session.get(id).is_some_and(|doc| doc.allows_annotation())
                            {
                                app.apply_edit(pdfcraft_engine::Edit::FontSizeAnnotation {
                                    page,
                                    index: annotation,
                                    size,
                                });
                            }
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                        SlotAction::ToggleStrokeEraser => {
                            app.comment_prefs.stroke_eraser = !app.comment_prefs.stroke_eraser;
                            app.quick_tool = QuickTool::Comment(CommentTool::Eraser);
                            if let Some(v) = app.views.get_mut(view_index) {
                                v.comments.radial_menu = None;
                            }
                        }
                    }
                }
            } else {
                // Dismiss on second right-click, any primary click/press outside a slot, or Escape key
                let escape = ui.input(|inp| inp.key_pressed(egui::Key::Escape));
                let secondary_close = ready && ui.input(|inp| inp.pointer.secondary_clicked());
                let primary_outside = ready
                    && hovered_idx.is_none()
                    && ui.input(|inp| inp.pointer.primary_pressed() || inp.pointer.primary_clicked());

                if (escape || secondary_close || primary_outside)
                    && let Some(v) = app.views.get_mut(view_index)
                {
                    v.comments.radial_menu = None;
                }
            }
        });
}

fn apply_style_to_selected(app: &mut PdfCraftApp, tool: CommentTool) {
    let style = app.comment_prefs.style(tool);
    if let Some((index, id)) = app.active_ids()
        && let Some((page, annotation)) = app.views.get(index).and_then(|view| view.comments.selected)
        && app.session.get(id).is_some_and(|doc| doc.allows_annotation())
    {
        app.apply_edit(pdfcraft_engine::Edit::StyleAnnotation {
            page,
            index: annotation,
            color: Some(style.color),
            opacity: Some(style.opacity),
            width: tool.has_width().then_some(style.width),
            endings: None,
        });
    }
}
