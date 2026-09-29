//! Procedural item art. Icons are painted with the egui painter (no image
//! assets), matching the game's procedurally generated block textures. Each
//! icon is a small stylised silhouette keyed off the item's kind, drawn to fill
//! the cell so it scales with multi-cell items and the drag preview alike.

use egui::{Color32, Pos2, Shape, Stroke};

use crate::inventory::items::{BackpackKind, BarterKind, MedKind, RigKind};
use crate::inventory::{Item, ItemKind};
use crate::weapons::ammo::AmmoClass;
use crate::weapons::armor::ArmorKind;
use crate::weapons::{AttachmentId, ReceiverId, Slot};

// --- palette -----------------------------------------------------------------

const STEEL: (u8, u8, u8) = (156, 162, 170);
const STEEL_D: (u8, u8, u8) = (96, 101, 110);
const GUN: (u8, u8, u8) = (48, 50, 55);
const GUN_L: (u8, u8, u8) = (74, 77, 84);
const WOOD: (u8, u8, u8) = (150, 98, 52);
const WOOD_D: (u8, u8, u8) = (104, 66, 34);
const BRASS: (u8, u8, u8) = (204, 166, 78);
const BRASS_D: (u8, u8, u8) = (150, 118, 50);
const OD: (u8, u8, u8) = (98, 106, 72);
const OD_D: (u8, u8, u8) = (66, 72, 46);
const LENS: (u8, u8, u8) = (86, 150, 196);
const GOLD: (u8, u8, u8) = (214, 176, 66);
const RED: (u8, u8, u8) = (188, 66, 58);
const CLOTH: (u8, u8, u8) = (70, 78, 60);
const CLOTH_D: (u8, u8, u8) = (48, 54, 42);
const OUTLINE: (u8, u8, u8) = (22, 23, 25);

/// Drawing helper: authors icons in a unit box (0..1, 0..1) mapped onto the
/// item rect, applying the shared alpha and scaling line widths to the cell.
struct Pen<'a> {
    p: &'a egui::Painter,
    r: egui::Rect,
    a: f32,
    s: f32,
}

impl<'a> Pen<'a> {
    fn new(p: &'a egui::Painter, rect: egui::Rect, alpha: f32) -> Self {
        let s = rect.width().min(rect.height()) / 44.0;
        // Leave a little breathing room around the art.
        Self {
            p,
            r: rect.shrink(rect.width().min(rect.height()) * 0.12),
            a: alpha,
            s,
        }
    }

    fn at(&self, u: f32, v: f32) -> Pos2 {
        Pos2::new(self.r.min.x + u * self.r.width(), self.r.min.y + v * self.r.height())
    }

    fn col(&self, c: (u8, u8, u8)) -> Color32 {
        Color32::from_rgba_unmultiplied(c.0, c.1, c.2, (self.a * 255.0) as u8)
    }

    fn w(&self, base: f32) -> f32 {
        (base * self.s).max(0.6)
    }

    fn stroke(&self, base: f32, c: (u8, u8, u8)) -> Stroke {
        Stroke::new(self.w(base), self.col(c))
    }

    /// Rounded rectangle in unit coords with a dark outline.
    fn slab(&self, u0: f32, v0: f32, u1: f32, v1: f32, rad: f32, fill: (u8, u8, u8)) {
        let rect = egui::Rect::from_two_pos(self.at(u0, v0), self.at(u1, v1));
        let rad = rad * self.r.width().min(self.r.height());
        self.p.rect_filled(rect, rad, self.col(fill));
        self.p
            .rect_stroke(rect, rad, self.stroke(1.0, OUTLINE), egui::StrokeKind::Inside);
    }

    /// Filled convex polygon (unit coords) with a dark outline.
    fn poly(&self, pts: &[(f32, f32)], fill: (u8, u8, u8)) {
        let pts: Vec<Pos2> = pts.iter().map(|&(u, v)| self.at(u, v)).collect();
        self.p
            .add(Shape::convex_polygon(pts, self.col(fill), self.stroke(1.0, OUTLINE)));
    }

    /// Circle centred at (u,v); radius is a fraction of the smaller side.
    fn disc(&self, u: f32, v: f32, rad: f32, fill: (u8, u8, u8)) {
        let r = rad * self.r.width().min(self.r.height());
        self.p
            .circle(self.at(u, v), r, self.col(fill), self.stroke(1.0, OUTLINE));
    }

    fn line(&self, a: (f32, f32), b: (f32, f32), width: f32, c: (u8, u8, u8)) {
        self.p
            .line_segment([self.at(a.0, a.1), self.at(b.0, b.1)], self.stroke(width, c));
    }
}

/// Paint the item's icon inside `rect`.
pub fn paint(p: &egui::Painter, rect: egui::Rect, item: &Item, alpha: f32) {
    let pen = Pen::new(p, rect, alpha);
    match item.kind {
        ItemKind::Weapon(r) => weapon(&pen, r),
        ItemKind::Attachment(a) => attachment(&pen, a),
        ItemKind::Ammo(a) => ammo(&pen, a.def().class),
        ItemKind::Armor(a) => armor(&pen, a),
        ItemKind::Backpack(b) => backpack(&pen, b),
        ItemKind::Rig(r) => rig(&pen, r),
        ItemKind::Med(m) => med(&pen, m),
        ItemKind::Barter(b) => barter(&pen, b),
        ItemKind::Roubles => roubles(&pen),
    }
}

// --- weapons -----------------------------------------------------------------

fn weapon(pen: &Pen, r: ReceiverId) {
    match r {
        ReceiverId::Grach => pistol(pen),
        ReceiverId::Vityaz => long_gun(pen, GUN, GUN_L, false),
        ReceiverId::Ak74n | ReceiverId::Akm => long_gun(pen, WOOD, WOOD_D, true),
    }
}

fn pistol(pen: &Pen) {
    // Slide across the top, grip angling down, trigger guard, muzzle at right.
    pen.slab(0.06, 0.30, 0.94, 0.52, 0.06, GUN_L);
    pen.slab(0.86, 0.34, 0.98, 0.48, 0.04, STEEL_D); // muzzle
    pen.poly(&[(0.20, 0.50), (0.44, 0.50), (0.34, 0.96), (0.16, 0.96)], GUN); // grip
    pen.poly(&[(0.44, 0.52), (0.60, 0.52), (0.52, 0.74), (0.44, 0.74)], GUN); // trigger guard
    pen.line((0.12, 0.36), (0.80, 0.36), 1.0, STEEL); // slide highlight
}

/// Rifle / SMG silhouette: stock, receiver, barrel, pistol grip, magazine, sight.
fn long_gun(pen: &Pen, furniture: (u8, u8, u8), furniture_d: (u8, u8, u8), curved_mag: bool) {
    // Barrel and receiver run left→right.
    pen.slab(0.60, 0.40, 0.96, 0.47, 0.02, STEEL_D); // barrel
    pen.slab(0.20, 0.34, 0.66, 0.54, 0.04, GUN); // receiver
    pen.slab(0.04, 0.36, 0.24, 0.52, 0.04, furniture); // stock
    pen.poly(&[(0.34, 0.54), (0.46, 0.54), (0.42, 0.80), (0.32, 0.80)], GUN); // pistol grip
                                                                              // Magazine.
    if curved_mag {
        pen.poly(&[(0.48, 0.54), (0.62, 0.54), (0.70, 0.94), (0.58, 0.96)], furniture_d);
    } else {
        pen.slab(0.50, 0.54, 0.62, 0.92, 0.03, GUN_L);
    }
    // Sight / rail on top.
    pen.slab(0.30, 0.24, 0.40, 0.34, 0.02, GUN_L);
    pen.slab(0.62, 0.28, 0.68, 0.40, 0.02, STEEL_D); // front sight post
}

// --- attachments -------------------------------------------------------------

fn attachment(pen: &Pen, a: AttachmentId) {
    let name = a.def().name;
    match a.def().slot {
        Slot::Barrel => {
            pen.slab(0.08, 0.42, 0.92, 0.58, 0.05, STEEL);
            pen.slab(0.80, 0.40, 0.92, 0.60, 0.04, STEEL_D);
            pen.line((0.14, 0.46), (0.76, 0.46), 0.8, (200, 205, 212));
        }
        Slot::Muzzle => {
            let suppressor = name.contains("Suppressor");
            if suppressor {
                pen.slab(0.16, 0.34, 0.90, 0.66, 0.10, GUN);
                pen.line((0.24, 0.42), (0.82, 0.42), 0.8, GUN_L);
            } else {
                // Muzzle brake with vent slots.
                pen.slab(0.28, 0.34, 0.86, 0.66, 0.08, STEEL_D);
                pen.slab(0.10, 0.44, 0.30, 0.56, 0.03, STEEL);
                for i in 0..3 {
                    let x = 0.42 + i as f32 * 0.16;
                    pen.line((x, 0.34), (x, 0.66), 1.2, OUTLINE);
                }
            }
        }
        Slot::Stock => {
            let wood = name.contains("wood") || name.contains("Wooden");
            let col = if wood { WOOD } else { GUN_L };
            pen.slab(0.62, 0.30, 0.94, 0.70, 0.06, col);
            pen.slab(0.10, 0.42, 0.66, 0.58, 0.04, if wood { WOOD_D } else { GUN });
            pen.poly(&[(0.62, 0.58), (0.78, 0.58), (0.72, 0.86), (0.62, 0.80)], col);
        }
        Slot::Sight => {
            let scope = name.contains("Scope") || name.contains("PSO");
            if scope {
                pen.slab(0.10, 0.40, 0.90, 0.58, 0.08, GUN);
                pen.disc(0.16, 0.49, 0.14, LENS);
                pen.disc(0.84, 0.49, 0.14, LENS);
                pen.slab(0.34, 0.58, 0.42, 0.72, 0.02, GUN); // mount
                pen.slab(0.58, 0.58, 0.66, 0.72, 0.02, GUN);
            } else {
                // Red dot / holo: housing with a glowing dot.
                pen.slab(0.24, 0.30, 0.76, 0.70, 0.08, GUN);
                pen.slab(0.30, 0.36, 0.70, 0.60, 0.06, LENS);
                pen.disc(0.50, 0.48, 0.06, RED);
                pen.slab(0.34, 0.70, 0.66, 0.84, 0.03, GUN_L); // base
            }
        }
        Slot::Magazine => {
            let drum = name.contains("Drum") || name.contains("drum");
            if drum {
                pen.disc(0.50, 0.58, 0.34, GUN_L);
                pen.disc(0.50, 0.58, 0.10, GUN);
                pen.slab(0.42, 0.16, 0.58, 0.34, 0.03, GUN_L);
            } else {
                let curved = name.contains("AK") || name.contains("74") || name.contains("AKM");
                let col = if name.contains("Wood") { WOOD } else { GUN_L };
                if curved {
                    pen.poly(&[(0.34, 0.14), (0.56, 0.14), (0.72, 0.90), (0.50, 0.94)], col);
                } else {
                    pen.slab(0.36, 0.14, 0.60, 0.92, 0.04, col);
                }
                // Feed lips + rounds hint.
                pen.slab(0.34, 0.10, 0.60, 0.20, 0.02, STEEL_D);
                pen.disc(0.44, 0.15, 0.05, BRASS);
            }
        }
        Slot::Grip => {
            pen.slab(0.30, 0.20, 0.70, 0.38, 0.05, GUN); // rail clamp
            pen.slab(0.40, 0.38, 0.60, 0.92, 0.06, GUN_L); // vertical grip body
        }
    }
}

// --- ammo --------------------------------------------------------------------

fn ammo(pen: &Pen, class: AmmoClass) {
    // A single cartridge stood upright: brass case, bullet with a tip colour
    // that reads the ammo class (FMJ / hollow-point / armour-piercing).
    pen.slab(0.36, 0.42, 0.64, 0.92, 0.04, BRASS); // case
    pen.slab(0.34, 0.86, 0.66, 0.96, 0.02, BRASS_D); // rim
    pen.line((0.42, 0.48), (0.42, 0.88), 0.8, (232, 200, 120)); // shine
    let tip = match class {
        AmmoClass::Fmj => (188, 150, 70),
        AmmoClass::HollowPoint => (170, 176, 184),
        AmmoClass::ArmorPiercing => (40, 42, 46),
    };
    // Bullet: shoulder then pointed tip.
    pen.poly(
        &[(0.38, 0.42), (0.62, 0.42), (0.58, 0.24), (0.50, 0.10), (0.42, 0.24)],
        tip,
    );
    if matches!(class, AmmoClass::ArmorPiercing) {
        pen.line((0.40, 0.22), (0.60, 0.22), 1.4, RED); // painted AP band
    }
}

// --- armor -------------------------------------------------------------------

fn armor(pen: &Pen, a: ArmorKind) {
    if a.def().helmet {
        // Dome with a rim.
        pen.poly(
            &[
                (0.20, 0.66),
                (0.24, 0.34),
                (0.42, 0.20),
                (0.58, 0.20),
                (0.76, 0.34),
                (0.80, 0.66),
            ],
            STEEL_D,
        );
        pen.slab(0.16, 0.64, 0.84, 0.76, 0.04, STEEL); // brim
        pen.line((0.30, 0.30), (0.50, 0.24), 1.0, STEEL); // highlight
    } else {
        // Plate-carrier vest: torso panel with shoulder straps.
        pen.poly(
            &[
                (0.24, 0.24),
                (0.76, 0.24),
                (0.82, 0.44),
                (0.72, 0.90),
                (0.28, 0.90),
                (0.18, 0.44),
            ],
            OD,
        );
        pen.slab(0.30, 0.14, 0.42, 0.28, 0.03, OD_D); // straps
        pen.slab(0.58, 0.14, 0.70, 0.28, 0.03, OD_D);
        pen.slab(0.38, 0.40, 0.62, 0.72, 0.04, OD_D); // armour plate
    }
}

// --- containers --------------------------------------------------------------

fn backpack(pen: &Pen, _b: BackpackKind) {
    pen.slab(0.18, 0.16, 0.82, 0.94, 0.10, CLOTH); // body
    pen.slab(0.30, 0.30, 0.70, 0.62, 0.06, CLOTH_D); // front pocket
    pen.slab(0.40, 0.06, 0.60, 0.20, 0.06, CLOTH_D); // top handle
    pen.line((0.30, 0.30), (0.30, 0.94), 1.0, CLOTH_D); // side seam
    pen.line((0.70, 0.30), (0.70, 0.94), 1.0, CLOTH_D);
}

fn rig(pen: &Pen, _r: RigKind) {
    pen.slab(0.20, 0.18, 0.80, 0.92, 0.06, CLOTH); // vest body
                                                   // A row of mag pouches.
    for i in 0..3 {
        let x0 = 0.24 + i as f32 * 0.18;
        pen.slab(x0, 0.46, x0 + 0.14, 0.82, 0.03, CLOTH_D);
    }
    pen.slab(0.30, 0.10, 0.44, 0.20, 0.03, CLOTH_D); // straps
    pen.slab(0.56, 0.10, 0.70, 0.20, 0.03, CLOTH_D);
}

// --- meds --------------------------------------------------------------------

fn med(pen: &Pen, m: MedKind) {
    match m {
        MedKind::Surv12 => {
            // Field surgical kit: pouch with a scalpel motif.
            pen.slab(0.10, 0.28, 0.90, 0.74, 0.08, (60, 66, 74));
            pen.line((0.20, 0.51), (0.80, 0.51), 1.4, STEEL); // scalpel blade
            pen.poly(&[(0.78, 0.44), (0.86, 0.51), (0.78, 0.58)], STEEL_D); // handle end
        }
        _ => {
            // First-aid box with a red cross.
            pen.slab(0.20, 0.16, 0.80, 0.88, 0.10, (238, 236, 228));
            pen.slab(0.20, 0.16, 0.80, 0.30, 0.04, (210, 208, 200)); // lid
            pen.slab(0.44, 0.36, 0.56, 0.78, 0.02, RED); // cross vertical
            pen.slab(0.30, 0.50, 0.70, 0.62, 0.02, RED); // cross horizontal
        }
    }
}

// --- barter & valuables ------------------------------------------------------

fn barter(pen: &Pen, b: BarterKind) {
    match b {
        BarterKind::Bitcoin => {
            pen.disc(0.50, 0.50, 0.40, GOLD);
            pen.disc(0.50, 0.50, 0.32, (232, 196, 90));
            pen.line((0.50, 0.28), (0.50, 0.72), 1.6, (120, 92, 20)); // ₿ stem
            pen.line((0.42, 0.34), (0.60, 0.34), 1.6, (120, 92, 20));
            pen.line((0.42, 0.50), (0.60, 0.50), 1.6, (120, 92, 20));
            pen.line((0.42, 0.66), (0.60, 0.66), 1.6, (120, 92, 20));
        }
        BarterKind::GoldChain => {
            for i in 0..4 {
                let x = 0.24 + i as f32 * 0.17;
                pen.disc(x, 0.40 + (i % 2) as f32 * 0.16, 0.10, GOLD);
            }
        }
        BarterKind::Toolset => {
            // Crossed wrench and screwdriver.
            pen.line((0.20, 0.80), (0.72, 0.28), 3.0, STEEL);
            pen.disc(0.74, 0.26, 0.10, STEEL_D);
            pen.line((0.80, 0.80), (0.34, 0.30), 2.4, (176, 120, 60)); // driver handle
            pen.line((0.34, 0.30), (0.24, 0.20), 1.6, STEEL); // driver tip
        }
        BarterKind::Battery => {
            pen.slab(0.30, 0.16, 0.70, 0.90, 0.06, OD);
            pen.slab(0.40, 0.08, 0.60, 0.16, 0.02, STEEL_D); // terminal
            pen.line((0.40, 0.60), (0.60, 0.60), 1.4, GOLD); // + label band
        }
        BarterKind::FuelCanister => {
            pen.slab(0.22, 0.24, 0.78, 0.92, 0.08, RED);
            pen.slab(0.40, 0.10, 0.60, 0.24, 0.03, (150, 50, 44)); // cap
            pen.poly(&[(0.30, 0.30), (0.44, 0.30), (0.30, 0.44)], (150, 50, 44));
            // corner rib
        }
        BarterKind::CircuitBoard => {
            pen.slab(0.16, 0.20, 0.84, 0.86, 0.04, (46, 96, 60));
            pen.line((0.24, 0.34), (0.72, 0.34), 1.0, GOLD);
            pen.line((0.24, 0.56), (0.60, 0.56), 1.0, GOLD);
            pen.disc(0.70, 0.66, 0.06, STEEL_D);
            pen.slab(0.30, 0.44, 0.44, 0.52, 0.01, (30, 32, 34)); // chip
        }
        BarterKind::Matches => {
            pen.slab(0.22, 0.34, 0.78, 0.90, 0.04, (170, 120, 60));
            pen.slab(0.22, 0.34, 0.78, 0.50, 0.02, RED); // strike strip
            pen.line((0.60, 0.10), (0.60, 0.34), 1.6, (232, 224, 200)); // stick
            pen.disc(0.60, 0.10, 0.05, RED); // match head
        }
        BarterKind::DuctTape => {
            pen.disc(0.50, 0.50, 0.40, (60, 62, 66));
            pen.disc(0.50, 0.50, 0.16, (28, 29, 31)); // hole
            pen.disc(0.50, 0.50, 0.28, (86, 88, 92));
        }
        BarterKind::Wires => {
            pen.line((0.20, 0.30), (0.80, 0.30), 2.0, RED);
            pen.line((0.20, 0.50), (0.80, 0.50), 2.0, (200, 180, 60));
            pen.line((0.20, 0.70), (0.80, 0.70), 2.0, (80, 130, 200));
        }
        BarterKind::GunpowderEagle | BarterKind::GunpowderKite => {
            // A can / jar of powder.
            pen.slab(0.30, 0.24, 0.70, 0.92, 0.06, (54, 56, 60));
            pen.slab(0.34, 0.12, 0.66, 0.26, 0.03, (40, 42, 46)); // lid
            pen.slab(0.36, 0.44, 0.64, 0.72, 0.02, (150, 60, 52)); // label
        }
        BarterKind::WeaponParts => {
            pen.disc(0.36, 0.44, 0.20, STEEL_D); // gear
            pen.disc(0.36, 0.44, 0.08, (40, 42, 46));
            pen.slab(0.56, 0.30, 0.86, 0.40, 0.02, STEEL); // bar
            pen.slab(0.60, 0.56, 0.80, 0.84, 0.03, GUN_L); // block
        }
        BarterKind::MetalScrap => {
            pen.poly(
                &[
                    (0.16, 0.62),
                    (0.40, 0.26),
                    (0.62, 0.52),
                    (0.86, 0.30),
                    (0.80, 0.84),
                    (0.22, 0.86),
                ],
                STEEL_D,
            );
            pen.line((0.30, 0.70), (0.60, 0.60), 1.0, STEEL);
        }
        BarterKind::Screws => fasteners(pen, false),
        BarterKind::Bolts => fasteners(pen, true),
        BarterKind::Nuts => {
            for (u, v) in [(0.36, 0.40), (0.62, 0.58)] {
                hex(pen, u, v, 0.20, STEEL_D);
                pen.disc(u, v, 0.07, (34, 36, 38));
            }
        }
    }
}

fn fasteners(pen: &Pen, bolt: bool) {
    for (u, v) in [(0.36, 0.30), (0.60, 0.66)] {
        if bolt {
            hex(pen, u, v - 0.02, 0.12, STEEL_D); // bolt head
        } else {
            pen.disc(u, v, 0.10, STEEL_D); // screw head
        }
        pen.line((u, v), (u + 0.02, v + 0.30), 2.4, STEEL); // shank
    }
}

fn hex(pen: &Pen, cu: f32, cv: f32, rad: f32, fill: (u8, u8, u8)) {
    let pts: Vec<(f32, f32)> = (0..6)
        .map(|i| {
            let a = std::f32::consts::PI / 3.0 * i as f32 + std::f32::consts::FRAC_PI_6;
            (cu + rad * a.cos(), cv + rad * a.sin())
        })
        .collect();
    pen.poly(&pts, fill);
}

fn roubles(pen: &Pen) {
    // A small stack of banknotes.
    pen.slab(0.12, 0.40, 0.88, 0.80, 0.05, (74, 110, 76));
    pen.slab(0.16, 0.30, 0.84, 0.70, 0.05, (92, 132, 92));
    pen.slab(0.20, 0.20, 0.80, 0.60, 0.05, (110, 150, 108));
    pen.disc(0.50, 0.40, 0.12, (198, 214, 190)); // seal
    pen.line((0.46, 0.34), (0.46, 0.46), 1.4, (60, 88, 60)); // ₽ mark
    pen.line((0.46, 0.34), (0.54, 0.34), 1.4, (60, 88, 60));
    pen.line((0.42, 0.40), (0.52, 0.40), 1.4, (60, 88, 60));
}
