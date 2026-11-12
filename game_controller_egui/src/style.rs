//! This module defines the look of the application: colors, fonts, and the painting of the custom
//! widgets (buttons, text) that the main window is built from.

use std::sync::Arc;

use eframe::egui::{
    self,
    text::{LayoutJob, TextFormat},
    Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Galley,
    Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
};

use game_controller_core::types::Color;

/// The Tailwind colors used by the original web UI.
pub mod tw {
    use eframe::egui::Color32;

    pub const WHITE: Color32 = Color32::WHITE;
    pub const BLACK: Color32 = Color32::BLACK;
    pub const GRAY_100: Color32 = Color32::from_rgb(0xf3, 0xf4, 0xf6);
    pub const GRAY_200: Color32 = Color32::from_rgb(0xe5, 0xe7, 0xeb);
    pub const GRAY_300: Color32 = Color32::from_rgb(0xd1, 0xd5, 0xdc);
    pub const GRAY_500: Color32 = Color32::from_rgb(0x6a, 0x72, 0x82);
    pub const GRAY_600: Color32 = Color32::from_rgb(0x4a, 0x55, 0x65);
    pub const GRAY_700: Color32 = Color32::from_rgb(0x36, 0x41, 0x53);
    pub const GRAY_900: Color32 = Color32::from_rgb(0x10, 0x18, 0x28);
    pub const RED_100: Color32 = Color32::from_rgb(0xff, 0xe2, 0xe2);
    pub const RED_600: Color32 = Color32::from_rgb(0xe7, 0x00, 0x0b);
    pub const BLUE_100: Color32 = Color32::from_rgb(0xdb, 0xea, 0xfe);
    pub const BLUE_600: Color32 = Color32::from_rgb(0x15, 0x5d, 0xfc);
    pub const YELLOW_100: Color32 = Color32::from_rgb(0xfe, 0xf9, 0xc2);
    pub const YELLOW_400: Color32 = Color32::from_rgb(0xfa, 0xcc, 0x15);
    pub const GREEN_100: Color32 = Color32::from_rgb(0xdc, 0xfc, 0xe7);
    pub const GREEN_600: Color32 = Color32::from_rgb(0x00, 0xa6, 0x3e);
    pub const ORANGE_100: Color32 = Color32::from_rgb(0xff, 0xed, 0xd4);
    pub const ORANGE_400: Color32 = Color32::from_rgb(0xff, 0x89, 0x04);
    pub const PURPLE_100: Color32 = Color32::from_rgb(0xf3, 0xe8, 0xff);
    pub const PURPLE_600: Color32 = Color32::from_rgb(0x98, 0x10, 0xfa);
    pub const AMBER_100: Color32 = Color32::from_rgb(0xfe, 0xf3, 0xc6);
    pub const AMBER_800: Color32 = Color32::from_rgb(0x97, 0x3c, 0x00);
    pub const FUCHSIA_400: Color32 = Color32::from_rgb(0xed, 0x6a, 0xff);
}

/// The color in which flashing elements alternate (yellow-400).
pub const FLASH: Color32 = tw::YELLOW_400;

/// The background color of a player button for a jersey color.
pub fn jersey_background(color: Color) -> Color32 {
    match color {
        Color::Red => tw::RED_100,
        Color::Blue => tw::BLUE_100,
        Color::Yellow => tw::YELLOW_100,
        Color::Black | Color::White => tw::WHITE,
        Color::Green => tw::GREEN_100,
        Color::Orange => tw::ORANGE_100,
        Color::Purple => tw::PURPLE_100,
        Color::Brown => tw::AMBER_100,
        Color::Gray => tw::GRAY_200,
    }
}

/// The text color of a team name for a jersey color.
pub fn jersey_text(color: Color) -> Color32 {
    match color {
        Color::Red => tw::RED_600,
        Color::Blue => tw::BLUE_600,
        Color::Yellow => tw::YELLOW_400,
        Color::Black | Color::White => tw::BLACK,
        Color::Green => tw::GREEN_600,
        Color::Orange => tw::ORANGE_400,
        Color::Purple => tw::PURPLE_600,
        Color::Brown => tw::AMBER_800,
        Color::Gray => tw::GRAY_600,
    }
}

/// The human-readable name of a jersey color.
pub fn color_name(color: Color) -> &'static str {
    match color {
        Color::Blue => "Blue",
        Color::Red => "Red",
        Color::Yellow => "Yellow",
        Color::Black => "Black",
        Color::White => "White",
        Color::Green => "Green",
        Color::Orange => "Orange",
        Color::Purple => "Purple",
        Color::Brown => "Brown",
        Color::Gray => "Gray",
    }
}

/// The font weights that are available.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
    Bold,
}

impl Weight {
    fn family_name(self) -> &'static str {
        match self {
            Weight::Regular => "Inter-400",
            Weight::Medium => "Inter-500",
            Weight::SemiBold => "Inter-600",
            Weight::Bold => "Inter-700",
        }
    }
}

/// This function returns a font with a given size (in points) and weight.
pub fn font(size: f32, weight: Weight) -> FontId {
    match weight {
        Weight::Regular => FontId::proportional(size),
        _ => FontId::new(size, FontFamily::Name(weight.family_name().into())),
    }
}

/// This function installs the bundled fonts (Inter in four weights) and the light look.
pub fn setup(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let weights: [(Weight, &'static [u8]); 4] = [
        (Weight::Regular, include_bytes!("../fonts/Inter-400.ttf")),
        (Weight::Medium, include_bytes!("../fonts/Inter-500.ttf")),
        (Weight::SemiBold, include_bytes!("../fonts/Inter-600.ttf")),
        (Weight::Bold, include_bytes!("../fonts/Inter-700.ttf")),
    ];
    // The default fonts stay as fallbacks (e.g. for symbols that Inter doesn't have).
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    for (weight, data) in weights {
        let name = weight.family_name().to_owned();
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_static(data)));
        let mut family = vec![name];
        family.extend(fallbacks.iter().cloned());
        if weight == Weight::Regular {
            fonts.families.insert(FontFamily::Proportional, family);
        } else {
            fonts
                .families
                .insert(FontFamily::Name(weight.family_name().into()), family);
        }
    }
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Light);
    ctx.all_styles_mut(|style| {
        let visuals = &mut style.visuals;
        visuals.panel_fill = tw::WHITE;
        visuals.window_fill = tw::WHITE;
        visuals.extreme_bg_color = tw::WHITE;
        visuals.override_text_color = Some(tw::BLACK);
        visuals.selection.bg_fill = tw::GRAY_300;
        visuals.selection.stroke = Stroke::new(1.0, tw::BLACK);
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            widget.corner_radius = CornerRadius::same(6);
            widget.bg_stroke = Stroke::new(1.0, tw::GRAY_600);
            widget.fg_stroke = Stroke::new(1.0, tw::BLACK);
            widget.weak_bg_fill = tw::WHITE;
            widget.bg_fill = tw::WHITE;
            widget.expansion = 0.0;
        }
        visuals.widgets.hovered.weak_bg_fill = tw::GRAY_100;
        visuals.widgets.hovered.bg_fill = tw::GRAY_100;
        visuals.widgets.active.weak_bg_fill = tw::GRAY_300;
        visuals.widgets.active.bg_fill = tw::GRAY_300;
        visuals.widgets.open.weak_bg_fill = tw::GRAY_100;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, tw::GRAY_600);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, tw::BLACK);
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
        visuals.menu_corner_radius = CornerRadius::same(6);
        visuals.window_stroke = Stroke::new(1.0, tw::GRAY_600);
        style.spacing.interact_size.y = 28.0;
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        style.spacing.icon_width = 16.0;
        style.spacing.icon_width_inner = 10.0;
        style.spacing.icon_spacing = 8.0;
        style.interaction.selectable_labels = false;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(16.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, font(20.0, Weight::SemiBold));
    });
}

/// The corner radius of buttons (rounded-md).
pub const RADIUS: f32 = 6.0;

/// This function lays out text, optionally wrapped and centered.
pub fn galley(
    painter: &Painter,
    text: &str,
    font: FontId,
    color: Color32,
    wrap_width: f32,
    halign: Align,
) -> Arc<Galley> {
    // Like the web UI, lines are 1.5 times as high as the font size.
    let line_height = Some(font.size * 1.5);
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color,
            line_height,
            ..Default::default()
        },
    );
    job.wrap.max_width = wrap_width;
    job.halign = halign;
    painter.layout_job(job)
}

/// This function paints a galley so that its bounding box is anchored at a point.
pub fn paint_galley(painter: &Painter, galley: Arc<Galley>, anchor: Align2, pos: Pos2) {
    let rect = anchor.anchor_size(pos, galley.size());
    let offset = rect.min - galley.rect.min;
    painter.galley(offset.to_pos2(), galley, tw::BLACK);
}

/// This function paints text centered in a rectangle (wrapping it if necessary). If a single word
/// is wider than the rectangle, the font is made smaller so that words are not broken.
pub fn paint_text_centered(
    painter: &Painter,
    rect: Rect,
    text: &str,
    mut font: FontId,
    color: Color32,
) {
    let widest_word = text
        .split_whitespace()
        .map(|word| {
            painter
                .layout_no_wrap(word.to_owned(), font.clone(), color)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    if widest_word > rect.width() && widest_word > 0.0 {
        // A small margin keeps the space after the word from being wrapped onto its own line.
        font.size *= (0.97 * rect.width() / widest_word).max(0.6);
    }
    let galley = galley(painter, text, font, color, rect.width(), Align::Center);
    paint_galley(painter, galley, Align2::CENTER_CENTER, rect.center());
}

/// This function paints text with tabular (fixed-width) digits, which keeps timers from jittering
/// when their digits change. Inter's digits are proportional and egui can't select the `tnum`
/// feature, so the characters are placed one by one.
pub fn paint_tabular(
    painter: &Painter,
    text: &str,
    font: FontId,
    color: Color32,
    anchor: Align2,
    pos: Pos2,
) -> Rect {
    let digit_width = painter
        .layout_no_wrap("0".into(), font.clone(), color)
        .size()
        .x;
    let glyphs: Vec<_> = text
        .chars()
        .map(|c| painter.layout_no_wrap(c.to_string(), font.clone(), color))
        .collect();
    let advances: Vec<f32> = text
        .chars()
        .zip(&glyphs)
        .map(|(c, g)| {
            if c.is_ascii_digit() {
                digit_width
            } else {
                g.size().x
            }
        })
        .collect();
    let height = glyphs.iter().map(|g| g.size().y).fold(0.0, f32::max);
    let rect = anchor.anchor_size(pos, Vec2::new(advances.iter().sum(), height));
    let mut x = rect.left();
    for (glyph, advance) in glyphs.into_iter().zip(advances) {
        let glyph_x = x + (advance - glyph.size().x) / 2.0;
        painter.galley(Pos2::new(glyph_x, rect.top()), glyph, color);
        x += advance;
    }
    rect
}

/// This function exposes painted text to accessibility tools (screen readers, UI automation), which
/// otherwise only see widgets. `name` is the accessible name and `value` the displayed text; for
/// [egui::Role::Label], egui uses the name as the text, so `value` should be empty.
pub fn accessible_text(
    ui: &Ui,
    rect: Rect,
    id: egui::Id,
    role: egui::Role,
    name: &str,
    value: &str,
) {
    let response = ui.interact(rect, id, Sense::hover());
    let (name, value) = (name.to_owned(), value.to_owned());
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(role, true, &name);
        if !value.is_empty() {
            info.current_text_value = Some(value.clone());
        }
        info
    });
    hide_if_clipped(ui, &response, rect);
}

/// This function marks a widget as hidden for accessibility tools if it lies completely outside
/// the visible area (e.g. a player button scrolled out of view). egui doesn't do this itself.
fn hide_if_clipped(ui: &Ui, response: &Response, rect: Rect) {
    if !ui.is_rect_visible(rect) {
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_hidden());
    }
}

/// The visual state of a button.
#[derive(Clone, Copy, Default)]
pub struct ButtonState {
    /// The button is highlighted (e.g. a selected toggle).
    pub active: bool,
    /// The button can be clicked.
    pub legal: bool,
}

/// This function allocates an interactive button at a fixed rectangle and paints its frame. The
/// content must be painted by the caller. Illegal buttons are not clickable.
pub fn button_frame(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    label: &str,
    state: ButtonState,
    fill: Option<Color32>,
) -> Response {
    let sense = if state.legal {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    let label = label.to_owned();
    let legal = state.legal;
    let selected = state.active;
    response.widget_info(|| {
        let mut info = WidgetInfo::labeled(egui::Role::Button, legal, &label);
        info.selected = Some(selected);
        info
    });
    hide_if_clipped(ui, &response, rect);
    if ui.is_rect_visible(rect) {
        let fill = fill.unwrap_or(if state.active {
            tw::GRAY_300
        } else if state.legal {
            tw::WHITE
        } else {
            tw::GRAY_100
        });
        let fill = if state.legal && response.is_pointer_button_down_on() {
            darken(fill, 0.1)
        } else if state.legal && response.hovered() {
            darken(fill, 0.04)
        } else {
            fill
        };
        ui.painter().rect(
            rect,
            RADIUS,
            fill,
            Stroke::new(1.0, tw::GRAY_600),
            StrokeKind::Inside,
        );
        if state.legal && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    response
}

/// This function draws a standard text button and returns whether it was clicked.
pub fn text_button(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    label: &str,
    state: ButtonState,
    font_size: f32,
) -> bool {
    let response = button_frame(ui, rect, id, label, state, None);
    let color = if state.legal || state.active {
        tw::BLACK
    } else {
        tw::GRAY_500
    };
    let inner = rect.shrink2(Vec2::new(font_size * 0.5, font_size * 0.25));
    paint_text_centered(
        ui.painter(),
        inner,
        label,
        font(font_size, Weight::Regular),
        color,
    );
    response.clicked()
}

fn darken(color: Color32, amount: f32) -> Color32 {
    let f = 1.0 - amount;
    Color32::from_rgb(
        (color.r() as f32 * f) as u8,
        (color.g() as f32 * f) as u8,
        (color.b() as f32 * f) as u8,
    )
}

/// The connection status icons (after Heroicons, outline style).
pub mod icons {
    use eframe::egui::{epaint::PathShape, Color32, Painter, Pos2, Rect, Stroke, Vec2};

    fn map(rect: Rect, x: f32, y: f32) -> Pos2 {
        rect.min + Vec2::new(x, y) * (rect.width() / 24.0)
    }

    fn stroke(rect: Rect, color: Color32) -> Stroke {
        Stroke::new(1.5 * rect.width() / 24.0, color)
    }

    pub fn check(painter: &Painter, rect: Rect, color: Color32) {
        let points = vec![
            map(rect, 4.5, 12.75),
            map(rect, 10.5, 18.75),
            map(rect, 19.5, 5.25),
        ];
        painter.add(PathShape::line(points, stroke(rect, color)));
    }

    pub fn x_mark(painter: &Painter, rect: Rect, color: Color32) {
        painter.line_segment(
            [map(rect, 6.0, 18.0), map(rect, 18.0, 6.0)],
            stroke(rect, color),
        );
        painter.line_segment(
            [map(rect, 6.0, 6.0), map(rect, 18.0, 18.0)],
            stroke(rect, color),
        );
    }

    /// Two opposed horizontal arrows (⇄).
    pub fn arrows_right_left(painter: &Painter, rect: Rect, color: Color32) {
        let s = Stroke::new(rect.width() / 24.0 * 1.2, color);
        let segments = [
            // The upper arrow points right.
            ((3.0, 8.0), (21.0, 8.0)),
            ((21.0, 8.0), (16.5, 3.5)),
            // The lower arrow points left.
            ((21.0, 16.0), (3.0, 16.0)),
            ((3.0, 16.0), (7.5, 20.5)),
        ];
        for ((x0, y0), (x1, y1)) in segments {
            painter.line_segment([map(rect, x0, y0), map(rect, x1, y1)], s);
        }
    }

    pub fn exclamation_triangle(painter: &Painter, rect: Rect, color: Color32) {
        let points = vec![
            map(rect, 12.0, 3.0),
            map(rect, 21.6, 19.5),
            map(rect, 2.4, 19.5),
        ];
        let mut shape = PathShape::closed_line(points, stroke(rect, color));
        shape.stroke = stroke(rect, color).into();
        painter.add(shape);
        painter.line_segment(
            [map(rect, 12.0, 9.0), map(rect, 12.0, 12.75)],
            stroke(rect, color),
        );
        painter.circle_filled(map(rect, 12.0, 15.75), rect.width() / 24.0, color);
    }
}
