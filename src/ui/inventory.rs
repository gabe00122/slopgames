//! Grid inventory UI with drag & drop (Tarkov style).

use egui::{Align2, Color32, FontId, Key, LayerId, Order, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

use super::style;
use crate::inventory::{Category, EquipSlot, Equipment, Grid, GridRef, Item, ItemKind};
use crate::weapons::AmmoType;

pub const CELL: f32 = 44.0;

/// Where an item currently lives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    Grid { grid: GridRef, x: u8, y: u8, rotated: bool },
    Slot(EquipSlot),
}

struct Dragged {
    item: Item,
    from: Origin,
    rotated: bool,
    /// Pointer offset from the item's top-left corner, in pixels.
    grab: Vec2,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Discard,
    Split,
    Unload,
    Load(AmmoType),
    Modify,
    Use,
    Empty,
    Equip,
}

#[derive(Default)]
struct Frame {
    grid_rects: Vec<(GridRef, Rect)>,
    slot_rects: Vec<(EquipSlot, Rect)>,
    start_drag: Option<(Origin, u64, Vec2)>,
    quick: Option<(Origin, u64)>,
    equip: Option<(Origin, u64)>,
    menu: Option<(Origin, u64, MenuAction)>,
}

/// Actions the inventory can't perform on its own.
pub enum InvAction {
    Modify(Origin, u64),
    UseMed(GridRef, u64),
}

/// Everything the inventory UI may read and modify this frame.
pub struct InvCtx<'a> {
    pub equipment: &'a mut Equipment,
    pub stash: Option<&'a mut Grid>,
    pub loot: Option<&'a mut Grid>,
    pub loot_name: String,
    pub in_raid: bool,
}

impl InvCtx<'_> {
    pub fn grid(&self, g: GridRef) -> Option<&Grid> {
        match g {
            GridRef::Stash => self.stash.as_deref(),
            GridRef::Loot => self.loot.as_deref(),
            other => self.equipment.grid(other),
        }
    }

    pub fn grid_mut(&mut self, g: GridRef) -> Option<&mut Grid> {
        match g {
            GridRef::Stash => self.stash.as_deref_mut(),
            GridRef::Loot => self.loot.as_deref_mut(),
            other => self.equipment.grid_mut(other),
        }
    }

    fn item(&self, origin: Origin, uid: u64) -> Option<&Item> {
        match origin {
            Origin::Grid { grid, .. } => self.grid(grid)?.get(uid).map(|p| &p.item),
            Origin::Slot(s) => self.equipment.slot(s).as_ref().filter(|i| i.uid == uid),
        }
    }

    fn item_mut(&mut self, origin: Origin, uid: u64) -> Option<&mut Item> {
        match origin {
            Origin::Grid { grid, .. } => self.grid_mut(grid)?.get_mut(uid).map(|p| &mut p.item),
            Origin::Slot(s) => self.equipment.slot_mut(s).as_mut().filter(|i| i.uid == uid),
        }
    }

    pub fn take(&mut self, origin: Origin, uid: u64) -> Option<Item> {
        match origin {
            Origin::Grid { grid, .. } => self.grid_mut(grid)?.remove(uid).map(|p| p.item),
            Origin::Slot(s) => {
                let slot = self.equipment.slot_mut(s);
                if slot.as_ref().map(|i| i.uid) == Some(uid) {
                    slot.take()
                } else {
                    None
                }
            }
        }
    }

    /// Put an item back where it came from; falls back to any free space.
    pub fn put_back(&mut self, item: Item, origin: Origin) -> Result<(), Item> {
        let item = match origin {
            Origin::Grid { grid, x, y, rotated } => match self.grid_mut(grid) {
                Some(g) => match g.place(item, x, y, rotated) {
                    Ok(()) => return Ok(()),
                    Err(it) => it,
                },
                None => item,
            },
            Origin::Slot(s) => {
                let slot = self.equipment.slot_mut(s);
                if slot.is_none() && s.accepts(item.kind) {
                    *slot = Some(item);
                    return Ok(());
                }
                item
            }
        };
        self.insert_anywhere(item, &[GridRef::Stash, GridRef::Backpack, GridRef::Rig, GridRef::Pockets, GridRef::Loot])
    }

    fn insert_anywhere(&mut self, mut item: Item, order: &[GridRef]) -> Result<(), Item> {
        for g in order {
            if let Some(grid) = self.grid_mut(*g) {
                match grid.insert(item) {
                    Ok(()) => return Ok(()),
                    Err(it) => item = it,
                }
            }
        }
        Err(item)
    }

    /// Ammo of a type available to load: stash (out of raid) or pouches.
    fn ammo_available(&self, a: AmmoType) -> u32 {
        let kind = ItemKind::Ammo(a);
        self.stash.as_deref().map(|s| s.count_kind(kind)).unwrap_or(0) + self.equipment.count_kind(kind)
    }

    fn take_ammo(&mut self, a: AmmoType, n: u32) -> u32 {
        let kind = ItemKind::Ammo(a);
        let mut got = 0;
        if let Some(s) = self.stash.as_deref_mut() {
            got += s.take_kind(kind, n);
        }
        if got < n {
            got += self.equipment.take_kind(kind, n - got);
        }
        got
    }

    fn return_ammo(&mut self, a: AmmoType, mut n: u32) -> bool {
        let order: &[GridRef] = if self.stash.is_some() {
            &[GridRef::Stash, GridRef::Rig, GridRef::Pockets, GridRef::Pouch, GridRef::Backpack]
        } else {
            &[GridRef::Rig, GridRef::Pockets, GridRef::Pouch, GridRef::Backpack, GridRef::Loot]
        };
        while n > 0 {
            let chunk = n.min(60);
            n -= chunk;
            if self.insert_anywhere(Item::stack(ItemKind::Ammo(a), chunk), order).is_err() {
                return false;
            }
        }
        true
    }
}

fn category_color(c: Category) -> Color32 {
    match c {
        Category::Weapon => Color32::from_rgb(52, 58, 52),
        Category::Mod => Color32::from_rgb(44, 52, 66),
        Category::Ammo => Color32::from_rgb(78, 66, 34),
        Category::Gear => Color32::from_rgb(54, 64, 44),
        Category::Container => Color32::from_rgb(66, 56, 40),
        Category::Med => Color32::from_rgb(92, 38, 38),
        Category::Barter => Color32::from_rgb(52, 52, 54),
        Category::Valuable => Color32::from_rgb(104, 86, 28),
        Category::Money => Color32::from_rgb(44, 76, 46),
    }
}

/// Bottom-right status text for an item (stack count, rounds, HP...).
fn item_badge(item: &Item) -> Option<String> {
    if let Some(w) = &item.weapon {
        return Some(format!("{}/{}", w.rounds, w.capacity()));
    }
    match item.kind {
        ItemKind::Med(_) => item.uses.map(|u| u.to_string()),
        ItemKind::Armor(a) => item
            .durability
            .map(|d| format!("{:.0}/{:.0}", d, a.def().max_durability)),
        ItemKind::Backpack(_) | ItemKind::Rig(_) => item
            .contents
            .as_ref()
            .filter(|g| !g.is_empty())
            .map(|g| format!("{} items", g.items.len())),
        k if k.stackable() => Some(if item.count >= 10_000 {
            format!("{}k", item.count / 1000)
        } else {
            item.count.to_string()
        }),
        _ => None,
    }
}

fn paint_item(p: &egui::Painter, rect: Rect, item: &Item, hovered: bool, alpha: f32) {
    let base = category_color(item.kind.category());
    let fill = if hovered { base.gamma_multiply(1.35) } else { base };
    p.rect_filled(rect, 3.0, fill.gamma_multiply(alpha));
    p.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, Color32::from_white_alpha((70.0 * alpha) as u8)),
        StrokeKind::Inside,
    );
    let text_col = Color32::from_rgba_unmultiplied(235, 232, 215, (255.0 * alpha) as u8);
    let font = if rect.width() < 60.0 { 10.0 } else { 11.5 };
    let galley = p.layout(
        item.kind.short(),
        FontId::proportional(font),
        text_col,
        (rect.width() - 6.0).max(10.0),
    );
    p.galley(rect.min + egui::vec2(3.0, 2.0), galley, text_col);
    if let Some(b) = item_badge(item) {
        p.text(
            rect.right_bottom() - egui::vec2(3.0, 2.0),
            Align2::RIGHT_BOTTOM,
            b,
            FontId::monospace(10.5),
            Color32::from_rgba_unmultiplied(250, 240, 190, (255.0 * alpha) as u8),
        );
    }
    if let (ItemKind::Armor(a), Some(d)) = (item.kind, item.durability) {
        let frac = (d / a.def().max_durability).clamp(0.0, 1.0);
        let bar = Rect::from_min_size(rect.left_bottom() + egui::vec2(3.0, -16.0), egui::vec2((rect.width() - 6.0) * frac, 3.0));
        p.rect_filled(bar, 1.0, style::hp_color(frac));
    }
}

fn tooltip(ui: &mut egui::Ui, item: &Item) {
    ui.label(RichText::new(item.name()).strong().color(style::ACCENT));
    ui.label(item.kind.description());
    if let Some(w) = &item.weapon {
        let s = w.stats();
        ui.label(format!(
            "Ergonomics {:.0} · recoil {:.2}°/{:.2}° · {}x zoom",
            s.ergonomics, s.vertical_recoil, s.horizontal_recoil, s.zoom
        ));
        let loaded = w.loaded.filter(|_| w.rounds > 0).map(|a| a.def().name).unwrap_or("empty");
        ui.label(format!("Magazine: {}/{} ({})", w.rounds, w.capacity(), loaded));
        if !s.operable {
            ui.label(RichText::new("Inoperable: missing barrel").color(style::BAD));
        }
    }
    if item.kind.stackable() {
        ui.label(format!("Stack: {} / {}", item.count, item.kind.max_stack()));
    }
    let (w, h) = item.size();
    ui.label(RichText::new(format!("Size {}x{} · value {} RUB", w, h, item.value())).color(style::TEXT_DIM));
    ui.label(
        RichText::new("Drag to move · R rotates · Ctrl+click quick-move · Right-click for actions")
            .size(10.5)
            .color(style::TEXT_DIM),
    );
}

#[derive(Default)]
pub struct InventoryUi {
    drag: Option<Dragged>,
    frame: Frame,
    status: Option<(String, bool)>,
}

impl InventoryUi {
    pub fn status(&self) -> Option<&(String, bool)> {
        self.status.as_ref()
    }

    fn set_status(&mut self, msg: impl Into<String>, error: bool) {
        self.status = Some((msg.into(), error));
    }

    /// Return any dragged item to where it came from (e.g. when closing the screen).
    pub fn cancel(&mut self, ctx: &mut InvCtx) {
        if let Some(d) = self.drag.take() {
            if let Err(item) = ctx.put_back(d.item, d.from) {
                // Last resort: never destroy items silently.
                let _ = ctx.equipment.pockets.insert(item);
            }
        }
    }

    fn interactions(&mut self, ui: &egui::Ui, resp: egui::Response, origin: Origin, item: &Item, ctx: &InvCtx) {
        if self.drag.is_some() {
            return;
        }
        if resp.drag_started() {
            let grab = ui
                .ctx()
                .input(|i| i.pointer.press_origin())
                .map(|p| p - resp.rect.min)
                .unwrap_or(Vec2::splat(CELL * 0.5));
            self.frame.start_drag = Some((origin, item.uid, grab));
            return;
        }
        let ctrl = ui.ctx().input(|i| i.modifiers.command || i.modifiers.ctrl);
        if resp.clicked() && ctrl {
            self.frame.quick = Some((origin, item.uid));
        } else if resp.double_clicked() {
            self.frame.equip = Some((origin, item.uid));
        }
        let resp = resp.on_hover_ui(|ui| tooltip(ui, item));
        let uid = item.uid;
        let mut chosen: Option<MenuAction> = None;
        resp.context_menu(|ui| {
            ui.label(RichText::new(item.name()).color(style::ACCENT));
            ui.separator();
            if let Some(w) = &item.weapon {
                if !ctx.in_raid && ui.button("Modify").clicked() {
                    chosen = Some(MenuAction::Modify);
                }
                let cal = w.caliber();
                ui.menu_button("Load ammo", |ui| {
                    let mut any = false;
                    for a in cal.ammo_types() {
                        let n = ctx.ammo_available(*a);
                        if n > 0 {
                            any = true;
                            if ui.button(format!("{} (x{})", a.def().name, n)).clicked() {
                                chosen = Some(MenuAction::Load(*a));
                            }
                        }
                    }
                    if !any {
                        ui.label(RichText::new(format!("No {} ammo", cal.name())).color(style::TEXT_DIM));
                    }
                });
                if w.rounds > 0 && ui.button("Unload ammo").clicked() {
                    chosen = Some(MenuAction::Unload);
                }
            }
            if matches!(item.kind, ItemKind::Med(_)) && ctx.in_raid && ui.button("Use").clicked() {
                chosen = Some(MenuAction::Use);
            }
            if item.kind.stackable() && item.count > 1 && ui.button("Split stack").clicked() {
                chosen = Some(MenuAction::Split);
            }
            if matches!(origin, Origin::Grid { .. }) {
                if item.contents.as_ref().is_some_and(|g| !g.is_empty()) && ui.button("Empty contents").clicked() {
                    chosen = Some(MenuAction::Empty);
                }
                if EquipSlot::ALL.iter().any(|s| s.accepts(item.kind)) && ui.button("Equip").clicked() {
                    chosen = Some(MenuAction::Equip);
                }
            }
            let discard = if ctx.loot.is_some() { "Drop into container" } else { "Discard" };
            if ui.button(RichText::new(discard).color(style::BAD)).clicked() {
                chosen = Some(MenuAction::Discard);
            }
        });
        if let Some(a) = chosen {
            self.frame.menu = Some((origin, uid, a));
        }
    }

    /// Draw a container grid.
    pub fn grid(&mut self, ui: &mut egui::Ui, ctx: &InvCtx, g: GridRef) {
        let Some(grid) = ctx.grid(g) else {
            ui.label(RichText::new("—").color(style::TEXT_DIM));
            return;
        };
        let size = egui::vec2(grid.w as f32 * CELL, grid.h as f32 * CELL);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect.expand(1.0));
        painter.rect_filled(rect, 2.0, Color32::from_rgb(16, 18, 17));
        for y in 0..grid.h {
            for x in 0..grid.w {
                let c = Rect::from_min_size(rect.min + egui::vec2(x as f32 * CELL, y as f32 * CELL), Vec2::splat(CELL));
                painter.rect_stroke(c, 0.0, Stroke::new(1.0, Color32::from_rgb(40, 43, 40)), StrokeKind::Inside);
            }
        }
        self.frame.grid_rects.push((g, rect));
        for p in &grid.items {
            let (w, h) = p.size();
            let r = Rect::from_min_size(
                rect.min + egui::vec2(p.x as f32 * CELL, p.y as f32 * CELL),
                egui::vec2(w as f32 * CELL, h as f32 * CELL),
            )
            .shrink(1.5);
            let id = egui::Id::new(("inv_item", p.item.uid));
            let resp = ui.interact(r, id, Sense::click_and_drag());
            paint_item(&painter, r, &p.item, resp.hovered(), 1.0);
            let origin = Origin::Grid {
                grid: g,
                x: p.x,
                y: p.y,
                rotated: p.rotated,
            };
            self.interactions(ui, resp, origin, &p.item, ctx);
        }
    }

    /// Draw an equipment slot of the given size (in cells).
    pub fn slot(&mut self, ui: &mut egui::Ui, ctx: &InvCtx, s: EquipSlot, cells: (u8, u8)) {
        ui.vertical(|ui| {
            ui.label(RichText::new(s.name().to_uppercase()).size(10.5).color(style::TEXT_DIM));
            let size = egui::vec2(cells.0 as f32 * CELL, cells.1 as f32 * CELL);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let painter = ui.painter_at(rect.expand(2.0));
            painter.rect_filled(rect, 3.0, Color32::from_rgb(20, 22, 21));
            let accepts = self.drag.as_ref().map(|d| s.accepts(d.item.kind));
            let border = match accepts {
                Some(true) if ctx.equipment.slot(s).is_none() => style::GOOD,
                _ => Color32::from_rgb(58, 60, 54),
            };
            painter.rect_stroke(rect, 3.0, Stroke::new(1.5, border), StrokeKind::Inside);
            self.frame.slot_rects.push((s, rect));
            match ctx.equipment.slot(s) {
                Some(item) => {
                    let r = rect.shrink(3.0);
                    let resp = ui.interact(r, egui::Id::new(("inv_slot", s as u8)), Sense::click_and_drag());
                    paint_item(&painter, r, item, resp.hovered(), 1.0);
                    if let Some(w) = &item.weapon {
                        painter.text(
                            r.left_bottom() + egui::vec2(4.0, -3.0),
                            Align2::LEFT_BOTTOM,
                            w.name(),
                            FontId::proportional(10.5),
                            style::TEXT_DIM,
                        );
                    }
                    self.interactions(ui, resp, Origin::Slot(s), item, ctx);
                }
                None => {
                    painter.text(rect.center(), Align2::CENTER_CENTER, "empty", FontId::proportional(12.0), Color32::from_rgb(80, 80, 74));
                }
            }
        });
    }

    /// Resolve drags, drops and actions. Call once after drawing all grids/slots.
    pub fn end(&mut self, egui_ctx: &egui::Context, ctx: &mut InvCtx) -> Vec<InvAction> {
        let frame = std::mem::take(&mut self.frame);
        let mut actions = Vec::new();

        if self.drag.is_none() {
            if let Some((origin, uid, grab)) = frame.start_drag {
                if let Some(item) = ctx.take(origin, uid) {
                    let rotated = matches!(origin, Origin::Grid { rotated: true, .. });
                    self.drag = Some(Dragged {
                        item,
                        from: origin,
                        rotated,
                        grab,
                    });
                }
            }
        }

        if self.drag.is_some() {
            self.update_drag(egui_ctx, ctx, &frame);
        }

        if let Some((origin, uid)) = frame.quick {
            self.quick_move(ctx, origin, uid);
        }
        if let Some((origin, uid)) = frame.equip {
            self.equip(ctx, origin, uid);
        }
        if let Some((origin, uid, action)) = frame.menu {
            self.menu_action(ctx, origin, uid, action, &mut actions);
        }
        actions
    }

    fn update_drag(&mut self, egui_ctx: &egui::Context, ctx: &mut InvCtx, frame: &Frame) {
        let (pointer, released, rotate) = egui_ctx.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.primary_released() || !i.pointer.primary_down(),
                i.key_pressed(Key::R),
            )
        });
        let Some(d) = self.drag.as_mut() else { return };
        if rotate && d.item.size().0 != d.item.size().1 {
            d.rotated = !d.rotated;
            d.grab = Vec2::new(d.grab.y, d.grab.x);
        }
        let (w, h) = d.item.size();
        let (w, h) = if d.rotated { (h, w) } else { (w, h) };
        let size_px = egui::vec2(w as f32 * CELL, h as f32 * CELL);
        let grab = d.grab.min(size_px - Vec2::splat(4.0)).max(Vec2::splat(4.0));
        let Some(pointer) = pointer else { return };
        let top_left = pointer - grab;

        // Preview target placement.
        let painter = egui_ctx.layer_painter(LayerId::new(Order::Tooltip, egui::Id::new("inv_drag")));
        let mut target: Option<(GridRef, i32, i32, bool)> = None;
        for (g, rect) in &frame.grid_rects {
            if rect.contains(pointer) {
                let cx = ((top_left.x - rect.min.x) / CELL).round() as i32;
                let cy = ((top_left.y - rect.min.y) / CELL).round() as i32;
                let ok = ctx.grid(*g).is_some_and(|grid| grid.can_place((w, h), cx, cy, None));
                let hl = Rect::from_min_size(rect.min + egui::vec2(cx as f32 * CELL, cy as f32 * CELL), size_px);
                let col = if ok { style::GOOD } else { style::BAD };
                painter.rect_filled(hl, 2.0, col.gamma_multiply(0.25));
                target = Some((*g, cx, cy, ok));
                break;
            }
        }
        paint_item(&painter, Rect::from_min_size(top_left, size_px).shrink(1.5), &d.item, true, 0.85);

        if !released {
            return;
        }
        let Some(d) = self.drag.take() else { return };
        let mut item = d.item;

        // Equipment slots.
        for (s, rect) in &frame.slot_rects {
            if rect.contains(pointer) {
                if s.accepts(item.kind) && ctx.equipment.slot(*s).is_none() {
                    *ctx.equipment.slot_mut(*s) = Some(item);
                    self.status = None;
                    return;
                }
                self.set_status(
                    if ctx.equipment.slot(*s).is_some() {
                        "Slot is occupied".to_string()
                    } else {
                        format!("{} can't go in {}", item.name(), s.name())
                    },
                    true,
                );
                if let Err(it) = ctx.put_back(item, d.from) {
                    let _ = ctx.equipment.pockets.insert(it);
                }
                return;
            }
        }

        // Grids.
        if let Some((g, cx, cy, ok)) = target {
            let rect = frame.grid_rects.iter().find(|(gg, _)| *gg == g).map(|(_, r)| *r).unwrap_or(Rect::NOTHING);
            let under = ((pointer - rect.min) / CELL).floor();
            if let Some(grid) = ctx.grid_mut(g) {
                // Merge onto a matching stack under the cursor.
                if item.kind.stackable() {
                    let under_uid = grid.item_at(under.x as u8, under.y as u8).map(|p| p.item.uid);
                    if let Some(uid) = under_uid {
                        if let Some(p) = grid.get_mut(uid) {
                            if p.item.kind == item.kind {
                                let max = item.kind.max_stack();
                                let add = (max - p.item.count).min(item.count);
                                p.item.count += add;
                                item.count -= add;
                                if item.count == 0 {
                                    self.status = None;
                                    return;
                                }
                            }
                        }
                    }
                }
                if ok {
                    match grid.place(item, cx as u8, cy as u8, d.rotated) {
                        Ok(()) => {
                            self.status = None;
                            return;
                        }
                        Err(it) => item = it,
                    }
                } else {
                    self.set_status("Not enough space there", true);
                }
            }
        }
        if let Err(it) = ctx.put_back(item, d.from) {
            let _ = ctx.equipment.pockets.insert(it);
        }
    }

    fn quick_move(&mut self, ctx: &mut InvCtx, origin: Origin, uid: u64) {
        let from_grid = match origin {
            Origin::Grid { grid, .. } => Some(grid),
            Origin::Slot(_) => None,
        };
        let Some(item) = ctx.take(origin, uid) else { return };
        let player = [GridRef::Backpack, GridRef::Rig, GridRef::Pockets, GridRef::Pouch];
        let targets: Vec<GridRef> = match from_grid {
            Some(GridRef::Loot) | Some(GridRef::Stash) => player.to_vec(),
            _ => {
                if ctx.loot.is_some() {
                    vec![GridRef::Loot]
                } else if ctx.stash.is_some() {
                    vec![GridRef::Stash]
                } else {
                    player.iter().copied().filter(|g| Some(*g) != from_grid).collect()
                }
            }
        };
        // Equippable items from loot/stash go to an empty slot first.
        let mut item = item;
        if matches!(from_grid, Some(GridRef::Loot) | Some(GridRef::Stash)) {
            if let Some(s) = EquipSlot::ALL
                .iter()
                .find(|s| s.accepts(item.kind) && ctx.equipment.slot(**s).is_none())
            {
                *ctx.equipment.slot_mut(*s) = Some(item);
                return;
            }
        }
        match ctx.insert_anywhere(item, &targets) {
            Ok(()) => self.status = None,
            Err(it) => {
                self.set_status("No space to move that item", true);
                item = it;
                if let Err(it) = ctx.put_back(item, origin) {
                    let _ = ctx.equipment.pockets.insert(it);
                }
            }
        }
    }

    fn equip(&mut self, ctx: &mut InvCtx, origin: Origin, uid: u64) {
        let Some(kind) = ctx.item(origin, uid).map(|i| i.kind) else { return };
        let Some(slot) = EquipSlot::ALL.iter().copied().find(|s| s.accepts(kind)) else { return };
        if matches!(origin, Origin::Slot(_)) {
            return;
        }
        if ctx.equipment.slot(slot).is_some() {
            self.set_status(format!("{} slot is occupied", slot.name()), true);
            return;
        }
        if let Some(item) = ctx.take(origin, uid) {
            *ctx.equipment.slot_mut(slot) = Some(item);
        }
    }

    fn menu_action(&mut self, ctx: &mut InvCtx, origin: Origin, uid: u64, action: MenuAction, out: &mut Vec<InvAction>) {
        match action {
            MenuAction::Modify => out.push(InvAction::Modify(origin, uid)),
            MenuAction::Use => {
                if let Origin::Grid { grid, .. } = origin {
                    out.push(InvAction::UseMed(grid, uid));
                } else {
                    self.set_status("Move the med into your rig or pockets to use it", true);
                }
            }
            MenuAction::Equip => self.equip(ctx, origin, uid),
            MenuAction::Discard => {
                if let Some(item) = ctx.take(origin, uid) {
                    if let Some(loot) = ctx.loot.as_deref_mut() {
                        if let Err(it) = loot.insert(item) {
                            self.set_status("Container is full", true);
                            let _ = ctx.put_back(it, origin);
                        }
                    } else {
                        self.set_status(format!("Discarded {}", item.name()), false);
                    }
                }
            }
            MenuAction::Split => {
                let Origin::Grid { grid, .. } = origin else { return };
                let Some(item) = ctx.item_mut(origin, uid) else { return };
                let half = item.count / 2;
                if half == 0 {
                    return;
                }
                let kind = item.kind;
                item.count -= half;
                let new = Item::stack(kind, half);
                let placed = ctx
                    .grid_mut(grid)
                    .and_then(|g| g.find_spot(new.size(), None).map(|(x, y, r)| (g, x, y, r)))
                    .map(|(g, x, y, r)| g.place(new, x, y, r).is_ok())
                    .unwrap_or(false);
                if !placed {
                    if let Some(item) = ctx.item_mut(origin, uid) {
                        item.count += half;
                    }
                    self.set_status("No space to split the stack", true);
                }
            }
            MenuAction::Unload => {
                let Some(w) = ctx.item_mut(origin, uid).and_then(|i| i.weapon.as_mut()) else { return };
                let (Some(ammo), n) = (w.loaded, w.rounds) else { return };
                w.rounds = 0;
                if !ctx.return_ammo(ammo, n) {
                    self.set_status("Not enough space for all the rounds", true);
                }
            }
            MenuAction::Load(ammo) => {
                let Some(w) = ctx.item_mut(origin, uid).and_then(|i| i.weapon.as_mut()) else { return };
                let cap = w.capacity();
                let mut returned = None;
                if w.loaded != Some(ammo) && w.rounds > 0 {
                    returned = w.loaded.map(|a| (a, w.rounds));
                    w.rounds = 0;
                }
                let have = w.rounds;
                let got = ctx.take_ammo(ammo, cap.saturating_sub(have));
                if let Some(w) = ctx.item_mut(origin, uid).and_then(|i| i.weapon.as_mut()) {
                    w.rounds = have + got;
                    w.loaded = Some(ammo);
                }
                if let Some((a, n)) = returned {
                    ctx.return_ammo(a, n);
                }
                self.set_status(format!("Loaded {} x{}", ammo.def().short, got), false);
            }
            MenuAction::Empty => {
                let Origin::Grid { grid, .. } = origin else { return };
                let contents = ctx
                    .item_mut(origin, uid)
                    .and_then(|i| i.contents.as_mut())
                    .map(|g| std::mem::take(&mut g.items))
                    .unwrap_or_default();
                let mut left = Vec::new();
                for p in contents {
                    let dest = ctx.grid_mut(grid).map(|g| g.insert(p.item));
                    match dest {
                        Some(Ok(())) => {}
                        Some(Err(it)) => left.push(it),
                        None => {}
                    }
                }
                if !left.is_empty() {
                    self.set_status("Not everything fit", true);
                    if let Some(g) = ctx.item_mut(origin, uid).and_then(|i| i.contents.as_mut()) {
                        for it in left {
                            let _ = g.insert(it);
                        }
                    }
                }
            }
        }
    }
}

/// Standard equipment + carried containers column used by the stash and raid screens.
pub fn equipment_column(ui: &mut egui::Ui, inv: &mut InventoryUi, ctx: &InvCtx) {
    let section = |ui: &mut egui::Ui, text: &str| {
        ui.add_space(4.0);
        ui.label(RichText::new(text).size(12.0).color(style::ACCENT));
    };
    section(ui, "EQUIPMENT");
    ui.horizontal(|ui| {
        inv.slot(ui, ctx, EquipSlot::Helmet, (2, 2));
        inv.slot(ui, ctx, EquipSlot::Armor, (3, 3));
        inv.slot(ui, ctx, EquipSlot::Rig, (3, 3));
        inv.slot(ui, ctx, EquipSlot::Backpack, (3, 3));
    });
    ui.horizontal(|ui| {
        inv.slot(ui, ctx, EquipSlot::Primary, (6, 2));
        inv.slot(ui, ctx, EquipSlot::Holster, (3, 2));
    });
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            section(ui, "POCKETS");
            inv.grid(ui, ctx, GridRef::Pockets);
        });
        ui.vertical(|ui| {
            section(ui, "POUCH");
            inv.grid(ui, ctx, GridRef::Pouch);
        });
    });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            let name = ctx.equipment.rig.as_ref().map(|i| i.name()).unwrap_or("no rig");
            section(ui, &format!("RIG · {name}"));
            inv.grid(ui, ctx, GridRef::Rig);
        });
        ui.vertical(|ui| {
            let name = ctx.equipment.backpack.as_ref().map(|i| i.name()).unwrap_or("no backpack");
            section(ui, &format!("BACKPACK · {name}"));
            inv.grid(ui, ctx, GridRef::Backpack);
        });
    });
    if let Some((msg, err)) = inv.status() {
        ui.add_space(4.0);
        ui.label(RichText::new(msg).color(if *err { style::BAD } else { style::GOOD }));
    }
}

pub fn value_label(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    format!("{out} RUB")
}
