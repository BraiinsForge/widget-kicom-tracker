use std::cell::RefCell;

use bmc_wasm_sdk::*;

use crate::feed::{Bet, Person, date};
use crate::model::{Rotation, finished, navigate, visible};

const FEED: &str = "https://www.aox.cz/kicomtracker/api/v1/wagers.json";
const CACHE: &str = "kicom.snapshot.v1";
const ACCENT: Color = Color::from_hex(0x6E_E7_B7);
const INK: Color = Color::from_hex(0x0B_12_18);
const PANEL: Color = Color::from_hex(0x15_23_2B);
const MUTED: Color = Color::from_hex(0x93_AA_B6);
const LABEL: u32 = 18;
const LABEL_H: f32 = 22.0;
// From a section label's top to its text.
const LABEL_GAP: f32 = 28.0;
const MIN_PROSE: u16 = 18;
const YES: Color = ACCENT;
const YES_PANEL: Color = Color::from_hex(0x12_3A_2E);
const NO: Color = Color::from_hex(0xFC_A5_A5);
const NO_PANEL: Color = Color::from_hex(0x3D_1A_1E);
const KICOM_SIDE: Color = Color::from_hex(0x60_A5_FA);
const COUNTER_SIDE: Color = Color::from_hex(0xFB_BF_24);
const STRIPE: f32 = 4.0;
const MIN_NAME_WIDTH: f32 = 80.0;
// Line width a name box needs, summed over its lines, to show a readable name.
const MIN_NAME_LINE: f32 = 70.0;
const MIN_TEXT: u16 = 14;
const MAX_LINES: usize = 2;
// A label with one line of prose under it.
const MIN_SECTION: f32 = LABEL_GAP + MIN_PROSE as f32 * 1.2;
// The full layout's bet state, at the right end of the title row.
const STATUS_W: f32 = 420.0;

#[derive(Default)]
struct State {
    bets: Vec<Bet>,
    loaded: bool,
    error: bool,
    selected: Option<String>,
    rotation: Rotation,
    touched: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static POLL: RefCell<Option<PollHandle>> = const { RefCell::new(None) };
    static BUTTON_SKINS: RefCell<[Option<ButtonSkin>; 2]> = const { RefCell::new([None, None]) };
}

fn interval() -> u32 {
    let minutes = params::current().get_i32("refresh_minutes").unwrap_or(15);
    (minutes.clamp(5, 60) as u32) * 60_000
}

fn retention() -> i32 {
    params::current()
        .get_i32("retention_days")
        .unwrap_or(3)
        .clamp(0, 30)
}

fn parse_snapshot(body: &[u8], allow_legacy: bool) -> Option<Vec<Bet>> {
    let (mut bets, _) = crate::feed::parse(body, allow_legacy)?;
    // Persist retention state only after the entire snapshot has been validated.
    for bet in &mut bets {
        let key = format!("kicom.closed.{}", bet.id);
        if !finished(&bet.status) {
            kv::delete(&key);
            bet.closed = None;
        } else if bet.closed.is_none() {
            bet.closed = Some(
                kv::get_string(&key)
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or_else(|| {
                        let now = SystemTime::now().unix_secs;
                        kv::set(&key, now.to_string().as_bytes());
                        now
                    }),
            );
        }
    }
    Some(bets)
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    if let Some(body) = kv::get(CACHE).filter(|b| b.len() <= 512_000)
        && let Some(bets) = parse_snapshot(&body, true)
    {
        STATE.with_borrow_mut(|s| {
            s.bets = bets;
            s.loaded = true;
        });
    }
    let handle = register_poll(
        |_| Some(FetchSpec::get(FEED)),
        on_reply,
        PollConfig {
            interval_ms: Some(interval()),
            ..Default::default()
        },
    );
    POLL.set(Some(handle));
}

fn on_reply(handle: &PollHandle, response: &FetchResponse) {
    let parsed = if (200..300).contains(&response.status) && response.body().len() <= 512_000 {
        parse_snapshot(response.body(), false)
    } else {
        None
    };
    STATE.with_borrow_mut(|s| {
        if let Some(bets) = parsed {
            kv::set(CACHE, response.body());
            s.bets = bets;
            s.loaded = true;
            s.error = false;
        } else {
            s.error = true;
            handle.mark_stale();
        }
    });
    request_frame();
}

#[unsafe(no_mangle)]
pub extern "C" fn on_params_update() {
    POLL.with_borrow(|p| {
        if let Some(handle) = p {
            handle.set_interval(interval());
            handle.invalidate();
        }
    });
    request_frame();
}

#[unsafe(no_mangle)]
pub extern "C" fn on_system_update() {
    request_frame();
}

fn label(bet: &Bet, now: i64) -> &'static str {
    match bet.status.as_str() {
        "won" => "VYHRÁNO",
        "lost" => "PROHRÁNO",
        "draw" => "REMÍZA",
        "cancelled" => "ZRUŠENO",
        "open" if date(&bet.deadline).is_some_and(|at| now >= at + 86_400) => "ČEKÁ NA VYHODNOCENÍ",
        "open" => "AKTIVNÍ",
        _ => "ČEKÁ NA VYHODNOCENÍ",
    }
}

fn display_date(value: &str) -> String {
    match (value.get(8..10), value.get(5..7), value.get(..4)) {
        (Some(d), Some(m), Some(y)) => format!("{d}.{m}.{y}"),
        _ => value.to_string(),
    }
}

fn rotation_ms() -> u32 {
    params::current()
        .get_i32("rotation_seconds")
        .unwrap_or(15)
        .clamp(5, 120) as u32
        * 1_000
}

#[unsafe(no_mangle)]
pub extern "C" fn on_touch() {
    STATE.with_borrow_mut(|s| s.touched = true);
    request_frame();
}

fn rounded(draws: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let r = 12.0_f32.min(h / 2.0).min(w / 2.0);
    draws.push(Draw::rect(x + r, y, w - 2.0 * r, h, color));
    draws.push(Draw::rect(x, y + r, w, h - 2.0 * r, color));
    for cx in [x + r, x + w - r] {
        for cy in [y + r, y + h - r] {
            draws.push(Draw::circle(cx, cy, r, color));
        }
    }
}

fn fit(draws: &mut Vec<Draw>, rect: [f32; 4], value: &str, size: u32, color: Color) {
    draws.push(Draw::autofit_text(
        rect[0],
        rect[1],
        rect[2],
        rect[3],
        value,
        style!(size: size, color: color, max_width: rect[2].max(1.0) as u32, line_height: 1.2),
    ));
}

fn centered(draws: &mut Vec<Draw>, rect: [f32; 4], value: &str, size: u32, color: Color) {
    draws.push(Draw::autofit_text(
        rect[0],
        rect[1],
        rect[2],
        rect[3],
        value,
        style!(size: size, color: color, max_width: rect[2].max(1.0) as u32, line_height: 1.2,
            align: TextAlign::Center, valign: VerticalAlign::Center),
    ));
}

// Canvas autofit wraps past its box once text no longer fits at `min`,
// so the text gets a canvas of its own that clips the overflow.
fn clipped(
    clips: &mut Vec<Node>,
    rect: [f32; 4],
    value: &str,
    style: impl Into<TextStyle>,
    min: u16,
    max: u16,
) {
    clips.push(canvas(
        props!(width: rect[2], height: rect[3], inset_left: rect[0], inset_top: rect[1]),
        [Draw::autofit_text_ranged(
            0.0,
            0.0,
            rect[2],
            rect[3],
            value,
            style,
            AutoFit::Shrink,
            min,
            max,
        )],
    ));
}

// The floor rises until the lines `min` allows fill the box, so an overflow is cut between lines.
fn line(clips: &mut Vec<Node>, rect: [f32; 4], value: &str, min: u16, size: u16, color: Color) {
    if value.is_empty() {
        return;
    }
    let rows = (rect[3] / (f32::from(min) * 1.2))
        .floor()
        .clamp(1.0, MAX_LINES as f32);
    let floor = ((rect[3] / (rows * 1.2)).floor() as u16).clamp(min, size);
    clipped(
        clips,
        rect,
        value,
        style!(size: u32::from(size), color: color, max_width: rect[2].max(1.0) as u32,
            line_height: 1.2, valign: VerticalAlign::Center),
        floor,
        size,
    );
}

// SDK nine-patch skins preserve corner radii when buttons stretch to any width.
// Registration occurs in render scope, once per style; the host switches pressed assets.
fn button_skin(primary: bool) -> Option<ButtonSkin> {
    BUTTON_SKINS.with_borrow_mut(|skins| {
        let index = usize::from(primary);
        if let Some(skin) = skins[index] {
            return Some(skin);
        }
        let (normal_bytes, pressed_bytes): (&[u8], &[u8]) = if primary {
            (
                include_bytes!("../assets/button-mint.png"),
                include_bytes!("../assets/button-mint-pressed.png"),
            )
        } else {
            (
                include_bytes!("../assets/button-dark.png"),
                include_bytes!("../assets/button-dark-pressed.png"),
            )
        };
        let normal = register_bitmap(&format!("kicom.button.{index}.normal"), normal_bytes)?;
        let pressed = register_bitmap(&format!("kicom.button.{index}.pressed"), pressed_bytes)?;
        let skin = ButtonSkin {
            normal: NinePatch::from_id(normal, 12, 12, 12, 12),
            pressed: Some(NinePatch::from_id(pressed, 12, 12, 12, 12)),
            text_color: if primary { INK } else { GRAY_10 },
            pressed_text_color: if primary { INK } else { GRAY_10 },
            opaque: false,
        };
        skins[index] = Some(skin);
        Some(skin)
    })
}

fn control(key: &str, title: &str, width: f32, primary: bool) -> Node {
    // The pinned SDK left-aligns text-only button labels. Keep its button and
    // pressed skins, but put a passive, explicitly centered label over it.
    let size = if width < 100.0 {
        ButtonSize::Normal
    } else {
        ButtonSize::Large
    };
    let height = size.height();
    let button = make_button(
        key.to_owned(),
        String::new(),
        if primary {
            ButtonStyle::Primary
        } else {
            ButtonStyle::Secondary
        },
        size,
        None,
        false,
        button_skin(primary),
    )
    .stretch();
    let label = canvas(
        props!(width: width, height: height, inset_left: 0.0, inset_top: 0.0),
        [Draw::autofit_text(
            10.0,
            0.0,
            width - 20.0,
            height,
            title,
            style!(size: 20, color: if primary { INK } else { GRAY_10 },
                max_width: (width - 20.0) as u32,
                align: TextAlign::Center, valign: VerticalAlign::Center),
        )],
    );
    col(props!(width: width, height: 56.0), [button, label])
}

fn avatar(draws: &mut Vec<Draw>, person: &Person, x: f32, y: f32, size: f32) {
    if let Some(bitmap) = crate::portraits::get(&person.avatar) {
        draws.push(Draw::bitmap_id(x, y, size, size, Some(bitmap)));
    } else {
        let initials: String = person
            .name
            .split_whitespace()
            .take(2)
            .filter_map(|s| s.chars().next())
            .collect();
        rounded(draws, x, y, size, size, INK);
        centered(draws, [x, y, size, size], &initials, 24, ACCENT);
    }
}

#[derive(Clone, Copy)]
enum Side {
    Kicom,
    Counter,
}

fn participant(
    draws: &mut Vec<Draw>,
    clips: &mut Vec<Node>,
    person: &Person,
    side: Side,
    rect: [f32; 4],
) {
    let [x, y, w, h] = rect;
    let inset = if h >= 60.0 { 10.0 } else { 4.0 };
    let tall = h >= 60.0;
    // A yes/no side reads at a glance in colour; any other position stays prose.
    let (ink, panel) = match person.position.as_str() {
        "ANO" => (YES, Some(YES_PANEL)),
        "NE" => (NO, Some(NO_PANEL)),
        _ => (MUTED, None),
    };
    // A free-text side too narrow to read becomes a stripe in its side's colour.
    let narrow = w - (h - inset * 2.0).min(w * 0.35) - inset * 3.0 < MIN_NAME_WIDTH;
    let striped = panel.is_none() && !person.position.is_empty() && narrow;
    let (x, w) = if striped {
        let color = match side {
            Side::Kicom => KICOM_SIDE,
            Side::Counter => COUNTER_SIDE,
        };
        rounded(draws, x, y, w, h, color);
        (x + STRIPE, w - STRIPE)
    } else {
        (x, w)
    };
    rounded(draws, x, y, w, h, PANEL);
    let photo = (h - inset * 2.0).min(w * 0.35);
    let tw = w - photo - inset * 3.0;
    let pad = if tall { 6.0 } else { inset };
    let position_h = if tall { 24.0 } else { (h - pad * 2.0) / 2.0 };
    let name_h = h - pad * 2.0 - position_h;
    let name_lines = (name_h / (f32::from(MIN_TEXT) * 1.2))
        .floor()
        .clamp(1.0, MAX_LINES as f32);
    if tw * name_lines < MIN_NAME_LINE {
        // No room for a readable name: the avatar alone, with a yes/no side as a badge beside it
        // in a short tile and under it in a tall one.
        let Some(panel) = panel else {
            let photo = (w - inset * 2.0).min(h - inset * 2.0);
            avatar(
                draws,
                person,
                x + (w - photo) / 2.0,
                y + (h - photo) / 2.0,
                photo,
            );
            return;
        };
        let gap = 4.0;
        let badge = if tall {
            let badge_h = 18.0;
            let photo = (w - inset * 2.0).min(h - inset * 2.0 - badge_h - gap);
            let top = y + (h - photo - gap - badge_h) / 2.0;
            avatar(draws, person, x + (w - photo) / 2.0, top, photo);
            [x + inset, top + photo + gap, w - inset * 2.0, badge_h]
        } else {
            let photo = h - inset * 2.0;
            let badge_w = (w - inset * 2.0 - photo - gap).min(46.0);
            let left = x + (w - photo - gap - badge_w) / 2.0;
            avatar(draws, person, left, y + inset, photo);
            [left + photo + gap, y + (h - 20.0) / 2.0, badge_w, 20.0]
        };
        rounded(draws, badge[0], badge[1], badge[2], badge[3], panel);
        centered(draws, badge, &person.position, 14, ink);
        return;
    }
    avatar(draws, person, x + inset, y + inset, photo);
    let tx = x + photo + inset * 2.0;
    let name_size = if tall { 24 } else { 18 };
    let badge_w = if tall { 64.0 } else { 46.0 };
    let badge = panel.filter(|_| tw - badge_w - inset >= MIN_NAME_WIDTH);
    if let Some(panel) = badge {
        let bh = (h - inset * 2.0).min(36.0);
        let bx = x + w - inset - badge_w;
        let by = y + (h - bh) / 2.0;
        rounded(draws, bx, by, badge_w, bh, panel);
        centered(
            draws,
            [bx, by, badge_w, bh],
            &person.position,
            if tall { 20 } else { 16 },
            ink,
        );
    }
    if badge.is_some() || person.position.is_empty() {
        let tw = if badge.is_some() {
            tw - badge_w - inset
        } else {
            tw
        };
        line(
            clips,
            [tx, y + inset, tw, h - inset * 2.0],
            &person.name,
            MIN_TEXT,
            name_size,
            GRAY_10,
        );
    } else {
        // A striped name keeps the half-height box: a full-height one would let
        // autofit pick a bigger size that breaks a word.
        let top = if striped {
            y + (h - name_h) / 2.0
        } else {
            y + pad
        };
        line(
            clips,
            [tx, top, tw, name_h],
            &person.name,
            MIN_TEXT,
            name_size,
            GRAY_10,
        );
        if striped {
            return;
        }
        line(
            clips,
            [tx, y + pad + name_h, tw, position_h],
            &person.position,
            MIN_TEXT,
            if tall { 20 } else { 16 },
            ink,
        );
    }
}

// Prose grows to `max` when it is short and is cut at the last whole line at `MIN_PROSE`.
fn summary(clips: &mut Vec<Node>, rect: [f32; 4], value: &str, color: Color, max: u16) {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() || rect[3] < f32::from(MIN_PROSE) * 1.2 {
        return;
    }
    let style = style!(size: u32::from(max), color: color, max_width: rect[2].max(1.0) as u32,
        line_height: 1.2);
    let rows = (rect[3] / (f32::from(MIN_PROSE) * 1.2)).floor();
    clipped(
        clips,
        [rect[0], rect[1], rect[2], rows * f32::from(MIN_PROSE) * 1.2],
        &value,
        style,
        MIN_PROSE,
        max,
    );
}

fn overview(draws: &mut Vec<Draw>, clips: &mut Vec<Node>, b: &Bet, w: f32, h: f32, wide: bool) {
    if wide {
        line(
            clips,
            [0.0, 0.0, w - STATUS_W - 16.0, 44.0],
            &b.title,
            18,
            38,
            GRAY_10,
        );
        let gutter = 52.0;
        let side = (w - gutter) / 2.0;
        let right = side + gutter;
        let prose = 30;
        if let Some(kicom) = b.people.first() {
            participant(draws, clips, kicom, Side::Kicom, [0.0, 52.0, side, 64.0]);
        }
        let opponents = &b.people[1..];
        let count = opponents.len().clamp(1, 4);
        let tile = (side - 8.0 * (count - 1) as f32) / count as f32;
        for (i, person) in opponents.iter().take(count).enumerate() {
            participant(
                draws,
                clips,
                person,
                Side::Counter,
                [right + i as f32 * (tile + 8.0), 52.0, tile, 64.0],
            );
        }
        centered(draws, [side, 52.0, gutter, 64.0], "VS", 20, ACCENT);
        let left = if b.outcome.is_empty() && b.metrics.is_empty() {
            w
        } else {
            draws.push(Draw::rect(w / 2.0, 126.0, 1.0, h - 126.0, PANEL));
            side
        };
        let first_h = 96.0;
        let second_label = 124.0 + LABEL_GAP + first_h + 6.0;
        let second_top = second_label + LABEL_GAP;
        fit(
            draws,
            [0.0, 124.0, left, LABEL_H],
            "PODMÍNKY SÁZKY",
            LABEL,
            ACCENT,
        );
        summary(
            clips,
            [0.0, 124.0 + LABEL_GAP, left, first_h],
            &b.subject,
            GRAY_10,
            prose,
        );
        fit(
            draws,
            [0.0, second_label, left, LABEL_H],
            "O CO SE HRAJE",
            LABEL,
            ACCENT,
        );
        summary(
            clips,
            [0.0, second_top, left, h - second_top],
            &b.stake,
            GRAY_10,
            prose,
        );
        if !b.outcome.is_empty() {
            fit(draws, [right, 124.0, side, LABEL_H], "STAV", LABEL, ACCENT);
            let outcome_h = if b.metrics.is_empty() {
                h - 124.0 - LABEL_GAP
            } else {
                first_h
            };
            summary(
                clips,
                [right, 124.0 + LABEL_GAP, side, outcome_h],
                &b.outcome,
                GRAY_10,
                prose,
            );
        }
        if !b.metrics.is_empty() {
            fit(
                draws,
                [right, second_label, side, LABEL_H],
                "UKAZATELE",
                LABEL,
                ACCENT,
            );
            summary(
                clips,
                [right, second_top, side, h - second_top],
                &b.metrics.join(" · "),
                GRAY_10,
                prose,
            );
        }
    } else {
        // Compact displays prioritize participant identity.
        let short = h < 240.0;
        line(clips, [0.0, 0.0, w, 34.0], &b.title, 14, 28, GRAY_10);
        let y = 64.0;
        let tile_h = if short { 44.0 } else { 60.0 };
        // With no room for a section below them, the tiles take the free height.
        let tile_h = if h - (y + tile_h + 12.0) < MIN_SECTION {
            (h - y - 8.0).clamp(tile_h, 80.0)
        } else {
            tile_h
        };
        let side = (w - 16.0) / 2.0;
        if let Some(kicom) = b.people.first() {
            participant(draws, clips, kicom, Side::Kicom, [0.0, y, side, tile_h]);
        }
        let opponents = &b.people[1..];
        let count = opponents.len().clamp(1, 2);
        let tile = (side - 4.0 * (count - 1) as f32) / count as f32;
        for (i, person) in opponents.iter().take(count).enumerate() {
            participant(
                draws,
                clips,
                person,
                Side::Counter,
                [side + 16.0 + i as f32 * (tile + 4.0), y, tile, tile_h],
            );
        }
        let top = y + tile_h + 12.0;
        let available = h - top;
        let right = side + 16.0;
        let prose = 26;
        if available >= 120.0 {
            let section_h = (available - LABEL_GAP * 2.0 - 8.0) / 2.0;
            let left = if b.outcome.is_empty() && b.metrics.is_empty() {
                w
            } else {
                side
            };
            fit(
                draws,
                [0.0, top, left, LABEL_H],
                "PODMÍNKY SÁZKY",
                LABEL,
                ACCENT,
            );
            summary(
                clips,
                [0.0, top + LABEL_GAP, left, section_h],
                &b.subject,
                GRAY_10,
                prose,
            );
            let lower = top + LABEL_GAP + 8.0 + section_h;
            fit(
                draws,
                [0.0, lower, left, LABEL_H],
                "O CO SE HRAJE",
                LABEL,
                ACCENT,
            );
            summary(
                clips,
                [0.0, lower + LABEL_GAP, left, section_h],
                &b.stake,
                GRAY_10,
                prose,
            );
            if !b.outcome.is_empty() {
                fit(draws, [right, top, side, LABEL_H], "STAV", LABEL, ACCENT);
                summary(
                    clips,
                    [
                        right,
                        top + LABEL_GAP,
                        side,
                        if b.metrics.is_empty() {
                            available - LABEL_GAP
                        } else {
                            section_h
                        },
                    ],
                    &b.outcome,
                    GRAY_10,
                    prose,
                );
            }
            if !b.metrics.is_empty() {
                fit(
                    draws,
                    [right, lower, side, LABEL_H],
                    "UKAZATELE",
                    LABEL,
                    ACCENT,
                );
                summary(
                    clips,
                    [right, lower + LABEL_GAP, side, section_h],
                    &b.metrics.join(" · "),
                    GRAY_10,
                    prose,
                );
            }
        } else if available >= MIN_SECTION {
            // Too short for four sections, so the terms and the stake share one row.
            fit(
                draws,
                [0.0, top, side, LABEL_H],
                "PODMÍNKY SÁZKY",
                LABEL,
                ACCENT,
            );
            summary(
                clips,
                [0.0, top + LABEL_GAP, side, available - LABEL_GAP],
                &b.subject,
                GRAY_10,
                prose,
            );
            fit(
                draws,
                [right, top, side, LABEL_H],
                "O CO SE HRAJE",
                LABEL,
                ACCENT,
            );
            summary(
                clips,
                [right, top + LABEL_GAP, side, available - LABEL_GAP],
                &b.stake,
                GRAY_10,
                prose,
            );
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn render(delta_ms: u32) {
    let size = widget_size();
    let w = size.width as f32;
    let h = size.height as f32;
    let pad = if w >= 800.0 { 20.0 } else { 12.0 };
    let body_h = h - pad * 2.0 - 68.0;
    let cw = w - pad * 2.0;
    let now = SystemTime::now().unix_secs;
    let period = rotation_ms();
    let result = STATE.with_borrow_mut(|s| {
        let ids: Vec<String> = s
            .bets
            .iter()
            .filter(|b| visible(&b.status, b.closed, now, retention()))
            .map(|b| b.id.clone())
            .collect();
        let mut index = s
            .selected
            .as_ref()
            .and_then(|id| ids.iter().position(|v| v == id))
            .unwrap_or(0);
        let current = ids.get(index).cloned();
        if s.selected != current {
            s.selected = current;
            s.rotation.reset();
        }
        let touched = std::mem::take(&mut s.touched);
        if s.rotation
            .tick(delta_ms, period, ids.len() > 1 && !touched)
        {
            index = navigate(index, ids.len(), true);
            s.selected = ids.get(index).cloned();
        }
        let bet = s
            .selected
            .as_ref()
            .and_then(|id| s.bets.iter().find(|b| &b.id == id));
        let wide = cw >= 740.0 && body_h >= 370.0;
        let mut draws = Vec::new();
        let mut clips = Vec::new();
        let count = if ids.is_empty() {
            "—".into()
        } else {
            format!("{:02} / {:02}", index + 1, ids.len())
        };
        if let Some(b) = bet {
            overview(&mut draws, &mut clips, b, cw, body_h, wide);
            let status = format!("{} · Termín {}", label(b, now), display_date(&b.deadline));
            if wide {
                draws.push(Draw::autofit_text(cw - STATUS_W, 0.0, STATUS_W, 44.0, &status,
                    style!(size: 28, color: MUTED, max_width: STATUS_W as u32, line_height: 1.2,
                        align: TextAlign::Right, valign: VerticalAlign::Center)));
            } else {
                fit(&mut draws, [0.0, 34.0, cw, 24.0], &status, 20, MUTED);
            }
        } else {
            let message = if s.loaded {
                "Žádné aktuální sázky"
            } else if s.error {
                "Zdroj je nedostupný. Zkoušíme znovu…"
            } else {
                "Načítání sázek…"
            };
            fit(
                &mut draws,
                [0.0, 48.0, cw, body_h - 82.0],
                message,
                32,
                GRAY_10,
            );
        }
        // Reserve a compact attribution block; navigation fills the remaining width.
        let center_w = (cw * 0.3).clamp(145.0, 360.0);
        let bw = (cw - center_w - 16.0) / 2.0;
        // Below 12 px the host wraps instead of shrinking, so narrow footers drop the name.
        let credit = if center_w < 170.0 {
            "data by AOX.cz"
        } else {
            "KICOM TRACKER data by AOX.cz"
        };
        let footer = vec![Draw::autofit_text(0.0, 28.0, center_w, 22.0, credit, style!(size: 18, color: MUTED, max_width: center_w as u32, align: TextAlign::Center)),
        Draw::autofit_text(0.0, 4.0, center_w, 20.0, &count, style!(size: 17, color: GRAY_10, max_width: center_w as u32, align: TextAlign::Center))];
        render_ui(
            size.width,
            size.height,
            col(
                props!(padding: pad, gap: 12.0, background: INK),
                [
                    col(
                        props!(width: cw, height: body_h),
                        std::iter::once(canvas(props!(width: cw, height: body_h), draws)).chain(clips),
                    ),
                    row(
                        props!(gap: 8.0),
                        [
                            control(
                                "prev",
                                if cw < 500.0 { "Zpět" } else { "Předchozí" },
                                bw,
                                false,
                            ),
                            canvas(props!(width: center_w, height: 56.0), footer),
                            control("next", "Další", bw, true),
                        ],
                    ),
                ],
            ),
        )
    });
    STATE.with_borrow_mut(|s| {
        let ids: Vec<String> = s
            .bets
            .iter()
            .filter(|b| visible(&b.status, b.closed, now, retention()))
            .map(|b| b.id.clone())
            .collect();
        let index = s
            .selected
            .as_ref()
            .and_then(|id| ids.iter().position(|v| v == id))
            .unwrap_or(0);
        for (key, forward) in [("prev", false), ("next", true)] {
            if result.clicks.contains_key(key) {
                s.selected = ids.get(navigate(index, ids.len(), forward)).cloned();
                s.rotation.reset();
                request_frame();
            }
        }
    });
    request_frame_after(1_000);
}
