use crate::sections::{ExecutableFile, FileNode};
use egui::*;
use std::vec;

const HOVER_COLOR: Rgba = Rgba::from_rgb(0.8, 0.8, 0.8);
type BytesCount = i64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum SortBy {
    #[default]
    Actual,
    GroupedForSpaceUsageAnalysis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Sorting {
    pub sort_by: SortBy,
}

impl Sorting {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("ordering:");

            for &sort_by in &[SortBy::Actual, SortBy::GroupedForSpaceUsageAnalysis] {
                let selected = self.sort_by == sort_by;

                let label = format!("{sort_by:?}");

                if ui.add(egui::RadioButton::new(selected, label)).clicked() {
                    self.sort_by = sort_by;
                }
            }
        });
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Options {
    // --------------------
    // View:
    /// Controls zoom
    pub canvas_width_bytes: f32,

    /// How much we have panned sideways:
    pub sideways_pan_in_points: f32,

    // --------------------
    // Visuals:
    /// Events shorter than this many points aren't painted
    pub cull_width: f32,
    /// Draw each item with at least this width (only makes sense if [`Self::cull_width`] is 0)
    pub min_width: f32,

    pub rect_height: f32,
    pub spacing: f32,
    pub rounding: f32,

    pub frame_list_height: f32,
    /// Distance between subsequent frames in the frame view.
    pub frame_width: f32,

    pub sorting: Sorting,

    pub to_scale: bool,

    /// Set when user clicks a scope.
    /// First part is `now()`, second is range.
    #[cfg_attr(feature = "serde", serde(skip))]
    zoom_to_relative_bytes_range: Option<(f64, (BytesCount, BytesCount))>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            canvas_width_bytes: 0.0,
            sideways_pan_in_points: 0.0,

            // cull_width: 0.5, // save some CPU?
            cull_width: 0.0, // no culling
            min_width: 1.0,

            rect_height: 16.0,
            spacing: 4.0,
            rounding: 4.0,

            frame_list_height: 48.0,
            frame_width: 10.0,

            sorting: Default::default(),
            to_scale: true,

            zoom_to_relative_bytes_range: None,
        }
    }
}

/// Context for painting a frame.
struct Info {
    ctx: egui::Context,
    /// Bounding box of canvas in points:
    canvas: Rect,
    /// Interaction with the profiler canvas
    response: Response,
    painter: egui::Painter,
    text_height: f32,
    /// Time of first event
    start_bytes: BytesCount,
    /// Time of last event
    stop_bytes: BytesCount,

    font_id: FontId,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PaintResult {
    Culled,
    Hovered,
    Normal,
}

impl Info {
    fn point_from_bytes(&self, options: &Options, ns: BytesCount) -> f32 {
        self.canvas.min.x
            + options.sideways_pan_in_points
            + self.canvas.width() * ((ns - self.start_bytes) as f32) / options.canvas_width_bytes
    }
}

/// Show the Inspector.
pub fn ui(ui: &mut egui::Ui, options: &mut Options, files: &mut [ExecutableFile]) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().widgets.inactive.text_color(), "❓")
                    .on_hover_text(
                        "Drag to pan.\n\
            Zoom: Ctrl/cmd + scroll, or drag with secondary mouse button.\n\
            Click on a scope to zoom to it.\n\
            Double-click to reset view.",
                    );

                ui.separator();

                ui.checkbox(&mut options.to_scale, "Draw to scale");

                options.sorting.ui(ui);
            });
        });
    });

    ui.separator();

    Frame::dark_canvas(ui.style()).show(ui, |ui| {
        let available_height = ui.max_rect().bottom() - ui.min_rect().bottom();
        ScrollArea::vertical().show(ui, |ui| {
            let mut canvas = ui.available_rect_before_wrap();
            canvas.max.y = f32::INFINITY;
            let response = ui.interact(canvas, ui.id(), Sense::click_and_drag());

            let min_bytes = 0;
            let max_bytes = files
                .iter()
                .map(|file| file.root.bytes_end)
                .max()
                .unwrap_or(100);

            let info = Info {
                ctx: ui.ctx().clone(),
                canvas,
                response,
                painter: ui.painter_at(canvas),
                text_height: 15.0, // TODO
                start_bytes: min_bytes,
                stop_bytes: max_bytes,
                font_id: TextStyle::Body.resolve(ui.style()),
            };

            interact_with_canvas(options, &info.response, &info);

            let where_to_put_timeline = info.painter.add(Shape::Noop);

            let max_y = ui_canvas(options, &info, (min_bytes, max_bytes), files);

            let mut used_rect = canvas;
            used_rect.max.y = max_y;

            // Fill out space that we don't use so that the `ScrollArea` doesn't collapse in height:
            used_rect.max.y = used_rect.max.y.max(used_rect.min.y + available_height);

            let timeline = paint_timeline(&info, used_rect, options, min_bytes);
            info.painter
                .set(where_to_put_timeline, Shape::Vec(timeline));

            ui.allocate_rect(used_rect, Sense::hover());
        });
    });
}

fn ui_canvas(
    options: &mut Options,
    info: &Info,
    (min_bytes, max_bytes): (BytesCount, BytesCount),
    files: &mut [ExecutableFile],
) -> f32 {
    if options.canvas_width_bytes <= 0.0 {
        options.canvas_width_bytes = (max_bytes - min_bytes) as f32;
        options.zoom_to_relative_bytes_range = None;
    }

    // We paint the binaries top-down
    let mut cursor_y = info.canvas.top();
    cursor_y += info.text_height; // Leave room for time labels

    for file in files {
        // Visual separator between binaries:
        cursor_y += 2.0;
        let line_y = cursor_y;
        cursor_y += 2.0;

        let text_pos = pos2(info.canvas.min.x, cursor_y);

        paint_binary_info(info, file, text_pos);

        // draw on top of binary info background:
        info.painter.line_segment(
            [
                pos2(info.canvas.min.x, line_y),
                pos2(info.canvas.max.x, line_y),
            ],
            Stroke::new(1.0, Rgba::from_white_alpha(0.5)),
        );

        cursor_y += info.text_height;

        if !file.inspector_collapsed {
            paint_scope(
                info,
                options,
                0,
                cursor_y,
                &file.root,
                file.root.bytes_start,
                file.root.bytes_end,
            );

            let max_depth = 6;
            cursor_y += max_depth as f32 * (options.rect_height + options.spacing);
        }
        cursor_y += info.text_height; // Extra spacing between binaries
    }

    cursor_y
}

fn interact_with_canvas(options: &mut Options, response: &Response, info: &Info) {
    if response.drag_delta().x != 0.0 {
        options.sideways_pan_in_points += response.drag_delta().x;
        options.zoom_to_relative_bytes_range = None;
    }

    if response.hovered() {
        // Sideways pan with e.g. a touch pad:
        if info.ctx.input(|i| i.smooth_scroll_delta.x != 0.0) {
            options.sideways_pan_in_points += info.ctx.input(|i| i.smooth_scroll_delta.x);
            options.zoom_to_relative_bytes_range = None;
        }

        let mut zoom_factor = info.ctx.input(|i| i.zoom_delta_2d().x);

        if response.dragged_by(PointerButton::Secondary) {
            zoom_factor *= (response.drag_delta().y * 0.01).exp();
        }

        if zoom_factor != 1.0 {
            options.canvas_width_bytes /= zoom_factor;

            if let Some(mouse_pos) = response.hover_pos() {
                let zoom_center = mouse_pos.x - info.canvas.min.x;
                options.sideways_pan_in_points =
                    (options.sideways_pan_in_points - zoom_center) * zoom_factor + zoom_center;
            }
            options.zoom_to_relative_bytes_range = None;
        }
    }

    if response.double_clicked() {
        // Reset view
        options.zoom_to_relative_bytes_range = Some((
            info.ctx.input(|i| i.time),
            (0, info.stop_bytes - info.start_bytes),
        ));
    }

    if let Some((start_time, (start_bytes, end_bytes))) = options.zoom_to_relative_bytes_range {
        const ZOOM_DURATION: f32 = 0.75;
        let t = (info.ctx.input(|i| i.time - start_time) as f32 / ZOOM_DURATION).min(1.0);

        let canvas_width = response.rect.width();

        let target_canvas_width_bytes = (end_bytes - start_bytes) as f32;
        let target_pan_in_points = -canvas_width * start_bytes as f32 / target_canvas_width_bytes;

        options.canvas_width_bytes = lerp(
            options.canvas_width_bytes.recip()..=target_canvas_width_bytes.recip(),
            t,
        )
        .recip();
        options.sideways_pan_in_points =
            lerp(options.sideways_pan_in_points..=target_pan_in_points, t);

        if t >= 1.0 {
            options.zoom_to_relative_bytes_range = None;
        }

        info.ctx.request_repaint();
    }
}

fn paint_timeline(
    info: &Info,
    canvas: Rect,
    options: &Options,
    start_bytes: BytesCount,
) -> Vec<egui::Shape> {
    let mut shapes = vec![];

    if options.canvas_width_bytes <= 0.0 {
        return shapes;
    }

    let alpha_multiplier = 0.3;

    // We show all measurements relative to start_bytes

    let max_lines = canvas.width() / 4.0;
    let mut grid_spacing_bytes = 1;
    while options.canvas_width_bytes / (grid_spacing_bytes as f32) > max_lines {
        grid_spacing_bytes *= 10;
    }

    // We fade in lines as we zoom in:
    let num_tiny_lines = options.canvas_width_bytes / (grid_spacing_bytes as f32);
    let zoom_factor = remap_clamp(num_tiny_lines, (0.1 * max_lines)..=max_lines, 1.0..=0.0);
    let zoom_factor = zoom_factor * zoom_factor;
    let big_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.5..=1.0);
    let medium_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.1..=0.5);
    let tiny_alpha = remap_clamp(zoom_factor, 0.0..=1.0, 0.0..=0.1);

    let mut grid_bytes = 0;

    loop {
        let line_x = info.point_from_bytes(options, start_bytes + grid_bytes);
        if line_x > canvas.max.x {
            break;
        }

        if canvas.min.x <= line_x {
            let big_line = grid_bytes % (grid_spacing_bytes * 100) == 0;
            let medium_line = grid_bytes % (grid_spacing_bytes * 10) == 0;

            let line_alpha = if big_line {
                big_alpha
            } else if medium_line {
                medium_alpha
            } else {
                tiny_alpha
            };

            shapes.push(egui::Shape::line_segment(
                [pos2(line_x, canvas.min.y), pos2(line_x, canvas.max.y)],
                Stroke::new(1.0, Rgba::from_white_alpha(line_alpha * alpha_multiplier)),
            ));

            let text_alpha = if big_line {
                medium_alpha
            } else if medium_line {
                tiny_alpha
            } else {
                0.0
            };

            if text_alpha > 0.0 {
                let text = grid_text(grid_bytes);
                let text_x = line_x + 4.0;
                let text_color = Rgba::from_white_alpha((text_alpha * 2.0).min(1.0)).into();

                info.painter.fonts(|f| {
                    // Text at top:
                    shapes.push(egui::Shape::text(
                        f,
                        pos2(text_x, canvas.min.y),
                        Align2::LEFT_TOP,
                        &text,
                        info.font_id.clone(),
                        text_color,
                    ));
                });

                info.painter.fonts(|f| {
                    // Text at bottom:
                    shapes.push(egui::Shape::text(
                        f,
                        pos2(text_x, canvas.max.y - info.text_height),
                        Align2::LEFT_TOP,
                        &text,
                        info.font_id.clone(),
                        text_color,
                    ));
                });
            }
        }

        grid_bytes += grid_spacing_bytes;
    }

    shapes
}

fn grid_text(bytes: i64) -> String {
    if bytes >= 1_000_000 {
        let mb = bytes as f32 / 1_000_000f32;
        format!("{mb} MB")
    } else if bytes >= 1_000 {
        let kb = bytes as f32 / 1_000f32;
        format!("{kb} KB")
    } else {
        format!("{bytes:.3} bytes")
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_record(
    info: &Info,
    options: &mut Options,
    top_y: f32,
    section: &FileNode,
    unscaled_start: i64,
    unscaled_end: i64,
) -> PaintResult {
    let bytes_start = if options.to_scale {
        section.bytes_start
    } else {
        unscaled_start
    };
    let bytes_end = if options.to_scale {
        section.bytes_end
    } else {
        unscaled_end
    };
    let start_x = info.point_from_bytes(options, bytes_start);
    let stop_x = info.point_from_bytes(options, bytes_end);
    if info.canvas.max.x < start_x
        || stop_x < info.canvas.min.x
        || stop_x - start_x < options.cull_width
    {
        return PaintResult::Culled;
    }

    let bottom_y = top_y + options.rect_height;

    let rect = Rect::from_min_max(pos2(start_x, top_y), pos2(stop_x, bottom_y));

    let is_hovered = if let Some(mouse_pos) = info.response.hover_pos() {
        rect.contains(mouse_pos)
    } else {
        false
    };

    if is_hovered && info.response.clicked() {
        options.zoom_to_relative_bytes_range = Some((
            info.ctx.input(|i| i.time),
            (
                section.bytes_start - info.start_bytes,
                section.bytes_end - info.start_bytes,
            ),
        ));
    }

    let rect_color = if is_hovered {
        HOVER_COLOR
    } else {
        color_from_size(section.bytes_end - section.bytes_start)
    };

    let min_width = options.min_width;

    if rect.width() <= min_width {
        // faster to draw it as a thin line
        info.painter.line_segment(
            [rect.center_top(), rect.center_bottom()],
            egui::Stroke::new(min_width, rect_color),
        );
    } else {
        info.painter.rect_filled(rect, options.rounding, rect_color);
    }

    let wide_enough_for_text = stop_x - start_x > 32.0;
    if wide_enough_for_text {
        let painter = info.painter.with_clip_rect(rect.intersect(info.canvas));

        let text = &section.name;
        let pos = pos2(
            start_x + 4.0,
            top_y + 0.5 * (options.rect_height - info.text_height),
        );
        let pos = painter.round_pos_to_pixels(pos);
        const TEXT_COLOR: Color32 = Color32::BLACK;
        painter.text(
            pos,
            Align2::LEFT_TOP,
            text,
            info.font_id.clone(),
            TEXT_COLOR,
        );
    }

    if is_hovered {
        PaintResult::Hovered
    } else {
        PaintResult::Normal
    }
}

// TODO: would make more sense to color by section type
fn color_from_size(bytes: BytesCount) -> Rgba {
    let kb = bytes as f32 / 1000.0;
    // Brighter = larger
    // So we start with dark colors (blue) and later bright colors (green).
    let b = remap_clamp(kb, 0.0..=5.0, 1.0..=0.3);
    let r = remap_clamp(kb, 0.0..=10.0, 0.5..=0.8);
    let g = remap_clamp(kb, 10.0..=33.0, 0.1..=0.8);
    let a = 0.9;
    Rgba::from_rgb(r, g, b) * a
}

fn paint_scope(
    info: &Info,
    options: &mut Options,
    depth: usize,
    min_y: f32,
    section: &FileNode,
    unscaled_start: i64,
    unscaled_end: i64,
) -> PaintResult {
    let top_y = min_y + (depth as f32) * (options.rect_height + options.spacing);

    let result = paint_record(info, options, top_y, section, unscaled_start, unscaled_end);

    if result != PaintResult::Culled {
        for (i, child) in section.children.iter().enumerate() {
            let width = (unscaled_end - unscaled_start) / section.children.len() as i64;
            paint_scope(
                info,
                options,
                depth + 1,
                min_y,
                child,
                section.bytes_start + i as i64 * width,
                section.bytes_start + (i as i64 + 1) * width,
            );
        }

        if result == PaintResult::Hovered {
            egui::show_tooltip_at_pointer(&info.ctx, Id::new("inspector_tooltip"), |ui| {
                paint_section_details(ui, section);
            });
        }
    }
    result
}

fn paint_section_details(ui: &mut Ui, section: &FileNode) {
    egui::Grid::new("section_details_tooltip")
        .num_columns(2)
        .show(ui, |ui| {
            // show name because sometimes the name is truncated because the section is small
            ui.monospace("name");
            ui.monospace(&section.name);
            ui.end_row();

            ui.monospace("file start");
            ui.monospace(format!("0x{:x}", section.bytes_start));
            ui.end_row();

            ui.monospace("len");
            ui.monospace(format!("0x{:x}", section.len()));
            ui.end_row();

            for (name, value) in &section.notes {
                ui.monospace(name);
                ui.monospace(value);
                ui.end_row();
            }
        });
}

fn paint_binary_info(info: &Info, file: &mut ExecutableFile, pos: Pos2) {
    let collapsed_symbol = if file.inspector_collapsed {
        "⏵"
    } else {
        "⏷"
    };

    let galley = info.ctx.fonts(|f| {
        f.layout_no_wrap(
            format!("{} {}", collapsed_symbol, file.name.clone()),
            info.font_id.clone(),
            egui::Color32::PLACEHOLDER,
        )
    });

    let rect = Rect::from_min_size(pos, galley.size());

    let is_hovered = if let Some(mouse_pos) = info.response.hover_pos() {
        rect.contains(mouse_pos)
    } else {
        false
    };

    let text_color = if is_hovered {
        Color32::WHITE
    } else {
        Color32::from_white_alpha(229)
    };
    let back_color = if is_hovered {
        Color32::from_black_alpha(100)
    } else {
        Color32::BLACK
    };

    info.painter.rect_filled(rect.expand(2.0), 0.0, back_color);
    info.painter.galley(rect.min, galley, text_color);

    if is_hovered && info.response.clicked() {
        file.inspector_collapsed = !file.inspector_collapsed;
    }
}
