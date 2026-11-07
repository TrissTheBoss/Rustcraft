//! Minimal native Windows presentation layer for the dependency-free prototype.
#![cfg(windows)]

use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::time::Instant;

use crate::blocks::Aabb;
use crate::blocks::BlockRegistry;
use crate::drops::ItemDrops;
#[cfg(windows)]
use crate::gpu::GpuRenderer;
use crate::grid::PLAYER_EYE_OFFSET;
use crate::interaction::{
    break_block, place_full_block, raycast_blocks, PROTOTYPE_INTERACTION_REACH,
};
use crate::inventory::Inventory;
use crate::logger::Logger;
use crate::movement::{GameMode, MovementInput, PlayerController};
use crate::render::{
    greedy_mesh_chunk, load_png, Camera, CpuRenderer, DirectionalLight, InputKey, InputState,
    Material, MaterialRegistry, Mesh, Renderer, Vec3,
};
use crate::terrain::{TerrainGenerator, ROCK, SOIL, SURFACE};
use crate::world::{ChunkPos, CHUNK_SIZE};
use crate::world_store::{
    PlayerSnapshot, StoredDifficulty, StoredGameMode, StoredWorld, WorldMetadata, WorldStore,
};

type Hinstance = *mut c_void;
type Hwnd = *mut c_void;
type Hdc = *mut c_void;
type Lresult = isize;
type Wparam = usize;
type Lparam = isize;

const WM_DESTROY: u32 = 0x0002;
const WM_COMMAND: u32 = 0x0111;
const WM_CLOSE: u32 = 0x0010;
const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_TIMER: u32 = 0x0113;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_MOUSEMOVE: u32 = 0x0200;
const CS_HREDRAW: u32 = 0x0002;
const CS_VREDRAW: u32 = 0x0001;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const SW_SHOW: i32 = 5;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC0020;
const TIMER_ID: usize = 1;
const RENDER_TIMER_ID: usize = 2;
const MENU_SAVE_WORLD: usize = 1001;
const MENU_CREATE_WORLD: usize = 1002;
const MENU_QUIT: usize = 1003;
const MENU_WORLD_BASE: usize = 2000;
const MENU_EDIT_WORLD_BASE: usize = 2500;
const MENU_COPY_WORLD_BASE: usize = 3000;
const MENU_DELETE_WORLD_BASE: usize = 4000;
const MENU_RESTORE_BACKUP_BASE: usize = 5000;
const MENU_RESTORE_BACKUP_CAPACITY: usize = 2500;
const FORM_WORLD_NAME: usize = 1101;
const FORM_WORLD_SEED: usize = 1102;
const FORM_WORLD_MODE: usize = 1103;
const FORM_WORLD_ERROR: usize = 1104;
const FORM_WORLD_CREATE: usize = 1105;
const FORM_WORLD_CANCEL: usize = 1106;
const FORM_WORLD_DIFFICULTY: usize = 1107;
const FORM_WORLD_DIFFICULTY_LOCKED: usize = 1108;
const DIALOG_IDOK: usize = 1;
const DIALOG_IDCANCEL: usize = 2;
const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const EM_SETLIMITTEXT: u32 = 0x00C5;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const BST_CHECKED: isize = 1;
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const SS_WHITERECT: u32 = 0x0006;
const BS_DEFPUSHBUTTON: u32 = 0x0001;
const BS_PUSHBUTTON: u32 = 0x0000;
const IDYES: i32 = 6;
const MB_YESNO: u32 = 0x0004;
const MB_ICONWARNING: u32 = 0x0030;
const MB_DEFBUTTON2: u32 = 0x0100;
const TICK_INTERVAL_MS: u32 = 50;
const VK_ESCAPE: u32 = 0x1B;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_RBUTTONDOWN: u32 = 0x0204;
// The CPU renderer is supersampled 2x in each axis below. Keep the presented
// target at a 16:9 960x540 cap so the anti-aliased path remains responsive.
const MAX_RENDER_WIDTH: u32 = 960;
const MAX_RENDER_HEIGHT: u32 = 540;
const RENDER_SCALE: u32 = 2;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct PaintStruct {
    hdc: Hdc,
    erase: i32,
    rect: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult>,
    extra_class: i32,
    extra_window: i32,
    instance: Hinstance,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu: *const u16,
    name: *const u16,
}
#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pels: i32,
    y_pels: i32,
    used: u32,
    important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [[u8; 4]; 1],
}

#[link(name = "user32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: *mut c_void,
        instance: Hinstance,
        param: *mut c_void,
    ) -> Hwnd;
    fn ShowWindow(window: Hwnd, command: i32) -> i32;
    fn UpdateWindow(window: Hwnd) -> i32;
    fn GetMessageW(message: *mut Message, window: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> Lresult;
    fn BeginPaint(window: Hwnd, paint: *mut PaintStruct) -> Hdc;
    fn EndPaint(window: Hwnd, paint: *const PaintStruct) -> i32;
    fn DefWindowProcW(window: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
    fn PostQuitMessage(code: i32);
    fn InvalidateRect(window: Hwnd, rect: *const Rect, erase: i32) -> i32;
    fn SetTimer(window: Hwnd, id: usize, interval: u32, callback: *mut c_void) -> usize;
    fn SetWindowTextW(window: Hwnd, title: *const u16) -> i32;
    fn CreateMenu() -> *mut c_void;
    fn CreatePopupMenu() -> *mut c_void;
    fn AppendMenuW(menu: *mut c_void, flags: u32, item: usize, text: *const u16) -> i32;
    fn SetMenu(window: Hwnd, menu: *mut c_void) -> i32;
    fn GetMenu(window: Hwnd) -> *mut c_void;
    fn DestroyMenu(menu: *mut c_void) -> i32;
    fn PostMessageW(window: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> i32;
    fn MessageBoxW(window: Hwnd, text: *const u16, caption: *const u16, flags: u32) -> i32;
    fn SendMessageW(window: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
    fn GetWindowTextLengthW(window: Hwnd) -> i32;
    fn GetWindowTextW(window: Hwnd, text: *mut u16, max_chars: i32) -> i32;
    fn SetFocus(window: Hwnd) -> Hwnd;
    fn IsDialogMessageW(window: Hwnd, message: *const Message) -> i32;
    fn EnableWindow(window: Hwnd, enable: i32) -> i32;
    fn DrawMenuBar(window: Hwnd) -> i32;
    fn DestroyWindow(window: Hwnd) -> i32;
    fn ClipCursor(rect: *const Rect) -> i32;
    fn ShowCursor(show: i32) -> i32;
    fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn ClientToScreen(window: Hwnd, point: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn StretchDIBits(
        hdc: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src_x: i32,
        src_y: i32,
        src_width: i32,
        src_height: i32,
        bits: *const c_void,
        info: *const BitmapInfo,
        usage: u32,
        rop: u32,
    ) -> i32;
}
#[repr(C)]
struct Message {
    hwnd: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
    time: u32,
    point: Point,
    private: u32,
}

static mut FRAME: Option<(Vec<u8>, BitmapInfo)> = None;
static mut APP: Option<AppState> = None;
static mut SCENE: Option<Scene> = None;
static mut WORLD_FORM: Option<WorldCreationForm> = None;

struct Scene {
    world: crate::world::World,
    meshes: Vec<Mesh>,
    mesh_positions: Vec<ChunkPos>,
    materials: MaterialRegistry,
    block_textures: [Option<u16>; 4],
    grass_top_texture: Option<u16>,
    dirty: bool,
}

struct AppState {
    camera: Camera,
    input: InputState,
    yaw: f32,
    pitch: f32,
    cursor_locked: bool,
    width: i32,
    height: i32,
    render_width: u32,
    render_height: u32,
    logger: Logger,
    player: PlayerController,
    gpu: Option<GpuRenderer>,
    last_physics: Instant,
    selected_block: crate::world::BlockId,
    inventory: Inventory,
    drops: ItemDrops,
    last_jump_press: Option<Instant>,
    jump_was_pressed: bool,
    toggle_flight_pending: bool,
    pending_block_actions: VecDeque<bool>,
    world_store: Option<WorldStore>,
    world_metadata: Option<WorldMetadata>,
    menu_world_ids: Vec<String>,
    menu_restore_actions: Vec<(String, String)>,
    dialog_open: bool,
    last_save: Instant,
}

struct WorldCreationForm {
    controls: Vec<Hwnd>,
    world_id: Option<String>,
    name_edit: Hwnd,
    seed_edit: Hwnd,
    mode_combo: Hwnd,
    difficulty_combo: Hwnd,
    difficulty_locked: Hwnd,
    error_label: Hwnd,
}

const MF_STRING: u32 = 0x0000;
const MF_SEPARATOR: u32 = 0x0800;
const MF_POPUP: u32 = 0x0010;
const MF_GRAYED: u32 = 0x0001;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(static_mut_refs)]
unsafe extern "system" fn window_proc(
    window: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        WM_COMMAND => {
            let command = wparam & 0xffff;
            if WORLD_FORM.is_some() && command == DIALOG_IDCANCEL {
                close_world_creation_form(window);
                return 0;
            }
            if WORLD_FORM.is_some() && command == DIALOG_IDOK {
                submit_world_creation_form(window);
                return 0;
            }
            if command == FORM_WORLD_CANCEL {
                close_world_creation_form(window);
                return 0;
            }
            if command == FORM_WORLD_CREATE {
                submit_world_creation_form(window);
                return 0;
            }
            if WORLD_FORM.is_some() && command == FORM_WORLD_DIFFICULTY_LOCKED {
                if let Some(form) = WORLD_FORM.as_ref() {
                    let checked = SendMessageW(form.difficulty_locked, BM_GETCHECK, 0, 0);
                    EnableWindow(form.difficulty_combo, (checked != BST_CHECKED) as i32);
                }
                return 0;
            }
            if command == MENU_CREATE_WORLD {
                open_world_creation_form(window);
                return 0;
            }
            if (MENU_RESTORE_BACKUP_BASE..MENU_RESTORE_BACKUP_BASE + MENU_RESTORE_BACKUP_CAPACITY)
                .contains(&command)
            {
                let index = command - MENU_RESTORE_BACKUP_BASE;
                if let Some((id, backup)) = APP
                    .as_ref()
                    .and_then(|app| app.menu_restore_actions.get(index).cloned())
                {
                    restore_backup_from_menu(window, &id, &backup);
                }
                return 0;
            }
            if (MENU_DELETE_WORLD_BASE..MENU_DELETE_WORLD_BASE + 500).contains(&command) {
                let index = command - MENU_DELETE_WORLD_BASE;
                if let Some(id) = APP
                    .as_ref()
                    .and_then(|app| app.menu_world_ids.get(index).cloned())
                {
                    delete_world_from_menu(window, &id);
                }
                return 0;
            }
            if (MENU_EDIT_WORLD_BASE..MENU_EDIT_WORLD_BASE + 500).contains(&command) {
                let index = command - MENU_EDIT_WORLD_BASE;
                if let Some(id) = APP
                    .as_ref()
                    .and_then(|app| app.menu_world_ids.get(index).cloned())
                {
                    open_world_edit_form(window, &id);
                }
                return 0;
            }
            if let Some(app) = APP.as_mut() {
                match command {
                    MENU_SAVE_WORLD => {
                        if let Some(scene) = SCENE.as_mut() {
                            if scene.dirty {
                                match persist_world(app, scene) {
                                    Ok(()) => {
                                        app.logger.event("INFO", "world_saved", "reason=menu")
                                    }
                                    Err(error) => app.logger.event(
                                        "ERROR",
                                        "world_save_failed",
                                        error.to_string(),
                                    ),
                                }
                            } else {
                                app.logger
                                    .event("INFO", "world_save_skipped", "reason=no_changes");
                            }
                        }
                    }
                    MENU_QUIT => {
                        PostMessageW(window, WM_CLOSE, 0, 0);
                    }
                    selected if selected >= MENU_WORLD_BASE => {
                        if (MENU_WORLD_BASE..MENU_WORLD_BASE + 500).contains(&selected) {
                            let index = selected - MENU_WORLD_BASE;
                            if let Some(id) = app.menu_world_ids.get(index).cloned() {
                                open_world_from_menu(window, app, &id);
                            }
                        } else if (MENU_COPY_WORLD_BASE..MENU_COPY_WORLD_BASE + 500)
                            .contains(&selected)
                        {
                            let index = selected - MENU_COPY_WORLD_BASE;
                            if let Some(id) = app.menu_world_ids.get(index).cloned() {
                                copy_world_from_menu(window, app, &id);
                            }
                        }
                    }
                    _ => {}
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(window);
            0
        }
        0x0211 => {
            if let Some(app) = APP.as_mut() {
                if app.cursor_locked {
                    app.cursor_locked = false;
                    ClipCursor(null());
                    ShowCursor(1);
                }
            }
            0
        }
        WM_PAINT => {
            let mut paint = PaintStruct {
                hdc: null_mut(),
                erase: 0,
                rect: Rect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                restore: 0,
                inc_update: 0,
                reserved: [0; 32],
            };
            let hdc = BeginPaint(window, &mut paint);
            if let Some((pixels, info)) = FRAME.as_ref() {
                let mut client = Rect {
                    left: 0,
                    top: 0,
                    right: info.header.width,
                    bottom: info.header.height.abs(),
                };
                GetClientRect(window, &mut client);
                StretchDIBits(
                    hdc,
                    0,
                    0,
                    client.right,
                    client.bottom,
                    0,
                    0,
                    info.header.width,
                    info.header.height.abs(),
                    pixels.as_ptr() as *const c_void,
                    info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
            }
            EndPaint(window, &paint);
            0
        }
        WM_ERASEBKGND => 1,
        WM_TIMER => {
            if let Some(app) = APP.as_mut() {
                if wparam == TIMER_ID && !app.dialog_open {
                    app.logger.event("TRACE", "timer_tick", "interval_ms=50");
                    if let Some(scene) = SCENE.as_ref() {
                        let previous_transform = app.player.transform;
                        let previous_velocity = app.player.velocity;
                        let previous_flying = app.player.flying;
                        let previous_grounded = app.player.grounded;
                        let previous_mode = app.player.mode;
                        let input = MovementInput {
                            forward: app.input.is_pressed(InputKey::Forward) as i32 as f64
                                - app.input.is_pressed(InputKey::Backward) as i32 as f64,
                            strafe: app.input.is_pressed(InputKey::Right) as i32 as f64
                                - app.input.is_pressed(InputKey::Left) as i32 as f64,
                            jump: app.input.is_pressed(InputKey::Up),
                            sneak: app.input.is_pressed(InputKey::Down),
                            sprint: app.input.is_pressed(InputKey::Sprint),
                            toggle_flight: app.toggle_flight_pending,
                        };
                        app.toggle_flight_pending = false;
                        app.jump_was_pressed = app.input.is_pressed(InputKey::Up);
                        app.player.tick(input, app.yaw as f64, |x, y, z| {
                            let position = ChunkPos {
                                x: x.div_euclid(CHUNK_SIZE as i32),
                                y: y.div_euclid(CHUNK_SIZE as i32),
                                z: z.div_euclid(CHUNK_SIZE as i32),
                            };
                            let lx = x.rem_euclid(CHUNK_SIZE as i32) as usize;
                            let ly = y.rem_euclid(CHUNK_SIZE as i32) as usize;
                            let lz = z.rem_euclid(CHUNK_SIZE as i32) as usize;
                            scene.world.get_chunk(position).is_some_and(|chunk| {
                                BlockRegistry::new().is_solid(chunk.get(lx, ly, lz).unwrap())
                            })
                        });
                        if app.player.transform != previous_transform
                            || app.player.velocity != previous_velocity
                            || app.player.flying != previous_flying
                            || app.player.grounded != previous_grounded
                            || app.player.mode != previous_mode
                        {
                            if let Some(scene) = SCENE.as_mut() {
                                scene.dirty = true;
                            }
                        }
                        while let Some(breaking) = app.pending_block_actions.pop_front() {
                            if let Some(scene) = SCENE.as_mut() {
                                handle_block_action(app, scene, breaking);
                            }
                        }
                        if app.last_save.elapsed().as_secs() >= 30 {
                            if let Some(scene) = SCENE.as_mut() {
                                if scene.dirty {
                                    match persist_world(app, scene) {
                                        Ok(()) => app.logger.event(
                                            "INFO",
                                            "world_autosaved",
                                            "reason=dirty interval_seconds=30",
                                        ),
                                        Err(error) => app.logger.event(
                                            "ERROR",
                                            "world_autosave_failed",
                                            error.to_string(),
                                        ),
                                    }
                                }
                            }
                        }
                        app.logger.event(
                            "TRACE",
                            "player_tick",
                            format!(
                                "x={:.4} y={:.4} z={:.4} vx={:.4} vy={:.4} vz={:.4} grounded={}",
                                app.player.transform.x,
                                app.player.transform.y,
                                app.player.transform.z,
                                app.player.velocity[0],
                                app.player.velocity[1],
                                app.player.velocity[2],
                                app.player.grounded
                            ),
                        );
                        let world_name = app
                            .world_metadata
                            .as_ref()
                            .map(|metadata| metadata.name.as_str())
                            .unwrap_or("Unsaved Prototype");
                        let title = wide(&format!(
                            "Voxel Engine 0.8.8 | {world_name} | Slot {} {} | block={:.2},{:.2},{:.2} speed={:.2} b/s",
                            app.selected_block.0,
                            BlockRegistry::new().definition(app.selected_block).name,
                            app.player.transform.x,
                            app.player.transform.y,
                            app.player.transform.z,
                            (app.player.velocity[0].powi(2) + app.player.velocity[2].powi(2))
                                .sqrt()
                                * 20.0
                        ));
                        SetWindowTextW(window, title.as_ptr());
                    }
                    app.last_physics = Instant::now();
                }
                let direction = Vec3::new(
                    app.pitch.cos() * app.yaw.cos(),
                    app.pitch.sin(),
                    app.pitch.cos() * app.yaw.sin(),
                );
                if wparam == RENDER_TIMER_ID {
                    let alpha = (app.last_physics.elapsed().as_secs_f32() / 0.05).clamp(0.0, 1.0);
                    let transform = app.player.previous_transform;
                    let current = app.player.transform;
                    let position = Vec3::new(
                        (transform.x + (current.x - transform.x) * alpha as f64) as f32,
                        (transform.y + (current.y - transform.y) * alpha as f64) as f32
                            + PLAYER_EYE_OFFSET as f32,
                        (transform.z + (current.z - transform.z) * alpha as f64) as f32,
                    );
                    app.camera.position = position;
                }
                app.camera.target = app.camera.position + direction;
                if wparam == RENDER_TIMER_ID {
                    if let Some(gpu) = app.gpu.as_mut() {
                        if let Err(error) = gpu.render(&app.camera) {
                            app.logger.event("WARN", "gpu_frame_failed", error);
                            app.gpu = None;
                        }
                    } else if let Some(scene) = SCENE.as_ref() {
                        FRAME = Some(build_frame(
                            app.camera,
                            app.render_width,
                            app.render_height,
                            scene,
                        ));
                    }
                    app.logger.event(
                        "TRACE",
                        "frame_rendered",
                        format!("width={} height={}", app.width, app.height),
                    );
                    InvalidateRect(window, null(), 0);
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            if let Some(app) = APP.as_mut() {
                let x = (lparam as u32 & 0xffff) as i16 as i32;
                let y = ((lparam as u32 >> 16) & 0xffff) as i16 as i32;
                if app.cursor_locked {
                    let mut client = Rect {
                        left: 0,
                        top: 0,
                        right: app.width,
                        bottom: app.height,
                    };
                    GetClientRect(window, &mut client);
                    let center_x = (client.left + client.right) / 2;
                    let center_y = (client.top + client.bottom) / 2;
                    let previous_yaw = app.yaw;
                    let previous_pitch = app.pitch;
                    app.yaw += (x - center_x) as f32 * 0.006;
                    app.pitch = (app.pitch - (y - center_y) as f32 * 0.006).clamp(-1.45, 1.45);
                    if app.yaw != previous_yaw || app.pitch != previous_pitch {
                        if let Some(scene) = SCENE.as_mut() {
                            scene.dirty = true;
                        }
                    }
                    if x != center_x || y != center_y {
                        app.logger.event(
                            "TRACE",
                            "mouse_look",
                            format!("dx={} dy={}", x - center_x, y - center_y),
                        );
                    }
                    lock_cursor(window);
                }
            }
            0
        }
        WM_KEYDOWN | WM_KEYUP => {
            if let Some(app) = APP.as_mut() {
                if wparam as u32 == VK_ESCAPE && message == WM_KEYDOWN {
                    app.cursor_locked = false;
                    ClipCursor(null());
                    ShowCursor(1);
                    app.logger.event("INFO", "cursor_released", "key=escape");
                }
                if let Some(key) = key_for(wparam as u32) {
                    if key == InputKey::Up && message == WM_KEYDOWN && !app.jump_was_pressed {
                        let now = Instant::now();
                        if app
                            .last_jump_press
                            .is_some_and(|previous| now.duration_since(previous).as_millis() <= 300)
                        {
                            app.toggle_flight_pending = true;
                            app.last_jump_press = None;
                        } else {
                            app.last_jump_press = Some(now);
                        }
                        app.jump_was_pressed = true;
                    }
                    app.input.set(key, message == WM_KEYDOWN);
                    if key == InputKey::Up && message == WM_KEYUP {
                        app.jump_was_pressed = false;
                    }
                    app.logger.event(
                        "TRACE",
                        "key_event",
                        format!("key={} pressed={}", wparam, message == WM_KEYDOWN),
                    );
                }
                if message == WM_KEYDOWN && (0x31..=0x39).contains(&(wparam as u32)) {
                    let slot = (wparam as usize) - 0x31;
                    let selection_changed = app.inventory.select_hotbar(slot).is_ok();
                    if (0x31..=0x33).contains(&(wparam as u32)) {
                        app.selected_block = crate::world::BlockId((wparam as u16) - 0x30);
                    }
                    if selection_changed {
                        if let Some(scene) = SCENE.as_mut() {
                            scene.dirty = true;
                        }
                    }
                    let name = BlockRegistry::new().definition(app.selected_block).name;
                    app.logger.event(
                        "INFO",
                        "hotbar_select",
                        format!("slot={} block={name}", slot + 1),
                    );
                }
            }
            0
        }
        WM_LBUTTONDOWN => {
            if let Some(app) = APP.as_mut() {
                if app.dialog_open {
                    return 0;
                }
                if app.cursor_locked {
                    queue_block_action(app, true);
                    return 0;
                }
                app.cursor_locked = true;
                ShowCursor(0);
                lock_cursor(window);
                app.logger
                    .event("INFO", "cursor_captured", "source=mouse_click");
            }
            0
        }
        WM_RBUTTONDOWN => {
            if let Some(app) = APP.as_mut() {
                if app.dialog_open {
                    return 0;
                }
                if app.cursor_locked {
                    queue_block_action(app, false);
                    return 0;
                }
                app.cursor_locked = true;
                ShowCursor(0);
                lock_cursor(window);
                app.logger
                    .event("INFO", "cursor_captured", "source=mouse_click");
            }
            0
        }
        WM_DESTROY => {
            destroy_world_creation_form();
            if let (Some(app), Some(scene)) = (APP.as_mut(), SCENE.as_mut()) {
                while let Some(breaking) = app.pending_block_actions.pop_front() {
                    handle_block_action(app, scene, breaking);
                }
                if scene.dirty {
                    match persist_world(app, scene) {
                        Ok(()) => app
                            .logger
                            .event("INFO", "world_saved", "reason=window_close"),
                        Err(error) => {
                            app.logger
                                .event("ERROR", "world_save_failed", error.to_string())
                        }
                    }
                }
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

fn key_for(key: u32) -> Option<InputKey> {
    match key {
        0x57 => Some(InputKey::Forward),
        0x53 => Some(InputKey::Backward),
        0x41 => Some(InputKey::Left),
        0x44 => Some(InputKey::Right),
        0x20 => Some(InputKey::Up),
        0x11 => Some(InputKey::Down),
        0x10 => Some(InputKey::Sprint),
        _ => None,
    }
}

unsafe fn lock_cursor(window: Hwnd) {
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetClientRect(window, &mut rect) != 0 {
        let mut center = Point {
            x: (rect.left + rect.right) / 2,
            y: (rect.top + rect.bottom) / 2,
        };
        if ClientToScreen(window, &mut center) != 0 {
            SetCursorPos(center.x, center.y);
        }
        GetWindowRect(window, &mut rect);
        ClipCursor(&rect);
    }
}

unsafe fn build_main_menu(app: &mut AppState) -> *mut c_void {
    let main = CreateMenu();
    let worlds_menu = CreatePopupMenu();
    let file_menu = CreatePopupMenu();
    if main.is_null() || worlds_menu.is_null() || file_menu.is_null() {
        return null_mut();
    }

    app.menu_world_ids.clear();
    app.menu_restore_actions.clear();
    let create_label = wide("Create New World...");
    AppendMenuW(
        worlds_menu,
        MF_STRING,
        MENU_CREATE_WORLD,
        create_label.as_ptr(),
    );
    AppendMenuW(worlds_menu, MF_SEPARATOR, 0, null());
    if let Some(store) = app.world_store.as_ref() {
        match store.list() {
            Ok(worlds) => {
                for summary in worlds.into_iter().take(500) {
                    let index = app.menu_world_ids.len();
                    let id = summary.id.clone();
                    let name = summary.name.unwrap_or_else(|| summary.id.clone());
                    let health = match summary.health {
                        crate::world_store::WorldHealth::Ready => "",
                        crate::world_store::WorldHealth::Corrupt => " [Corrupt]",
                        crate::world_store::WorldHealth::UnsupportedVersion => " [Newer Version]",
                        crate::world_store::WorldHealth::MissingSave => " [Missing Save]",
                    };
                    let world_menu = CreatePopupMenu();
                    if world_menu.is_null() {
                        continue;
                    }
                    let restore_menu = CreatePopupMenu();
                    if restore_menu.is_null() {
                        continue;
                    }
                    let can_open = summary.health == crate::world_store::WorldHealth::Ready;
                    let open = wide("Open");
                    let edit = wide("Edit Options...");
                    let copy = wide("Copy");
                    let delete = wide("Delete...");
                    AppendMenuW(
                        world_menu,
                        MF_STRING | if can_open { 0 } else { MF_GRAYED },
                        MENU_WORLD_BASE + index,
                        open.as_ptr(),
                    );
                    AppendMenuW(
                        world_menu,
                        MF_STRING | if can_open { 0 } else { MF_GRAYED },
                        MENU_EDIT_WORLD_BASE + index,
                        edit.as_ptr(),
                    );
                    AppendMenuW(
                        world_menu,
                        MF_STRING | if can_open { 0 } else { MF_GRAYED },
                        MENU_COPY_WORLD_BASE + index,
                        copy.as_ptr(),
                    );
                    AppendMenuW(
                        world_menu,
                        MF_STRING,
                        MENU_DELETE_WORLD_BASE + index,
                        delete.as_ptr(),
                    );
                    AppendMenuW(world_menu, MF_SEPARATOR, 0, null());
                    let mut backup_count = 0;
                    match store.list_backups(&id) {
                        Ok(backups) => {
                            for backup in backups {
                                if app.menu_restore_actions.len() >= MENU_RESTORE_BACKUP_CAPACITY {
                                    break;
                                }
                                let action = app.menu_restore_actions.len();
                                let label = wide(&backup);
                                AppendMenuW(
                                    restore_menu,
                                    MF_STRING,
                                    MENU_RESTORE_BACKUP_BASE + action,
                                    label.as_ptr(),
                                );
                                app.menu_restore_actions.push((id.clone(), backup));
                                backup_count += 1;
                            }
                        }
                        Err(error) => app.logger.event(
                            "ERROR",
                            "world_backup_list_failed",
                            format!("world={id} error={error}"),
                        ),
                    }
                    if backup_count == 0 {
                        let empty = wide("No backups available");
                        AppendMenuW(restore_menu, MF_STRING | MF_GRAYED, 0, empty.as_ptr());
                    }
                    let restore = wide("Restore Backup");
                    AppendMenuW(
                        world_menu,
                        MF_POPUP,
                        restore_menu as usize,
                        restore.as_ptr(),
                    );
                    let label = wide(&format!("{name}{health}"));
                    AppendMenuW(worlds_menu, MF_POPUP, world_menu as usize, label.as_ptr());
                    app.menu_world_ids.push(id);
                }
            }
            Err(error) => app
                .logger
                .event("ERROR", "world_list_failed", error.to_string()),
        }
    }
    if app.menu_world_ids.is_empty() {
        let empty = wide("No saved worlds");
        AppendMenuW(worlds_menu, MF_STRING | MF_GRAYED, 0, empty.as_ptr());
    }

    let save = wide("Save World");
    AppendMenuW(file_menu, MF_STRING, MENU_SAVE_WORLD, save.as_ptr());
    let quit = wide("Quit");
    AppendMenuW(file_menu, MF_STRING, MENU_QUIT, quit.as_ptr());
    let worlds_title = wide("Worlds");
    let file_title = wide("File");
    AppendMenuW(main, MF_POPUP, worlds_menu as usize, worlds_title.as_ptr());
    AppendMenuW(main, MF_POPUP, file_menu as usize, file_title.as_ptr());
    main
}

unsafe fn install_main_menu(window: Hwnd, app: &mut AppState) {
    let menu = build_main_menu(app);
    if menu.is_null() {
        return;
    }
    let previous = GetMenu(window);
    SetMenu(window, menu);
    DrawMenuBar(window);
    if !previous.is_null() {
        DestroyMenu(previous);
    }
}

#[allow(static_mut_refs)]
unsafe fn create_form_control(
    parent: Hwnd,
    class_name: &str,
    label: &str,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: usize,
) -> Hwnd {
    let class = wide(class_name);
    let label = wide(label);
    CreateWindowExW(
        0,
        class.as_ptr(),
        label.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        width,
        height,
        parent,
        if id == 0 {
            null_mut()
        } else {
            id as *mut c_void
        },
        GetModuleHandleW(null()),
        null_mut(),
    )
}

unsafe fn add_form_label(
    parent: Hwnd,
    controls: &mut Vec<Hwnd>,
    text: &str,
    left: i32,
    top: i32,
    y: i32,
) -> Hwnd {
    let control = create_form_control(parent, "STATIC", text, 0, left + 24, top + y, 412, 22, 0);
    controls.push(control);
    control
}

#[allow(static_mut_refs)]
unsafe fn open_world_creation_form(window: Hwnd) {
    if WORLD_FORM.is_some() {
        return;
    }
    let Some(store) = APP.as_ref().and_then(|app| app.world_store.clone()) else {
        if let Some(app) = APP.as_ref() {
            app.logger.event(
                "ERROR",
                "world_create_unavailable",
                "world storage is unavailable",
            );
        }
        return;
    };
    let name = match store.list().and_then(|worlds| {
        let names = worlds
            .into_iter()
            .filter_map(|world| world.name)
            .collect::<std::collections::BTreeSet<_>>();
        (1..)
            .map(|number| format!("New World {number}"))
            .find(|candidate| !names.contains(candidate))
            .ok_or_else(|| std::io::Error::other("could not choose a world name"))
    }) {
        Ok(name) => name,
        Err(error) => {
            if let Some(app) = APP.as_ref() {
                app.logger
                    .event("ERROR", "world_create_failed", error.to_string());
            }
            return;
        }
    };
    build_world_form(window, None, name);
}

#[allow(static_mut_refs)]
unsafe fn open_world_edit_form(window: Hwnd, id: &str) {
    if WORLD_FORM.is_some() {
        return;
    }
    let Some(store) = APP.as_ref().and_then(|app| app.world_store.clone()) else {
        return;
    };
    match store.load(id) {
        Ok(world) => build_world_form(window, Some(world), String::new()),
        Err(error) => {
            if let Some(app) = APP.as_ref() {
                app.logger
                    .event("ERROR", "world_edit_load_failed", error.to_string());
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn build_world_form(window: Hwnd, world: Option<StoredWorld>, new_name: String) {
    let editing = world.is_some();
    let (world_id, name, seed_text, mode, difficulty, difficulty_locked) = match world {
        Some(world) => (
            Some(world.metadata.id),
            world.metadata.name,
            world.metadata.seed.to_string(),
            world.metadata.game_mode,
            world.metadata.difficulty,
            world.metadata.difficulty_locked,
        ),
        None => (
            None,
            new_name,
            String::new(),
            StoredGameMode::Creative,
            StoredDifficulty::Normal,
            false,
        ),
    };
    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let left = ((client.right - 460) / 2).max(12);
    let top = ((client.bottom - 470) / 2).max(12);
    let panel = create_form_control(window, "STATIC", "", SS_WHITERECT, left, top, 460, 470, 0);
    let mut controls = vec![panel];
    add_form_label(
        window,
        &mut controls,
        if editing {
            "Edit World Options"
        } else {
            "Create World"
        },
        left,
        top,
        18,
    );
    add_form_label(window, &mut controls, "World name", left, top, 52);
    let name_edit = create_form_control(
        window,
        "EDIT",
        &name,
        ES_AUTOHSCROLL | WS_TABSTOP,
        left + 24,
        top + 75,
        412,
        26,
        FORM_WORLD_NAME,
    );
    controls.push(name_edit);
    SendMessageW(name_edit, EM_SETLIMITTEXT, 128, 0);
    add_form_label(
        window,
        &mut controls,
        "Seed (blank chooses a random seed; decimal or 0x hexadecimal)",
        left,
        top,
        111,
    );
    let seed_edit = create_form_control(
        window,
        "EDIT",
        &seed_text,
        ES_AUTOHSCROLL | WS_TABSTOP,
        left + 24,
        top + 134,
        412,
        26,
        FORM_WORLD_SEED,
    );
    controls.push(seed_edit);
    SendMessageW(seed_edit, EM_SETLIMITTEXT, 64, 0);
    add_form_label(window, &mut controls, "Game mode", left, top, 170);
    let mode_combo = create_form_control(
        window,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST | WS_TABSTOP,
        left + 24,
        top + 193,
        412,
        130,
        FORM_WORLD_MODE,
    );
    controls.push(mode_combo);
    let survival = wide("Survival");
    let creative = wide("Creative");
    let adventure = wide("Adventure");
    let spectator = wide("Spectator");
    SendMessageW(mode_combo, CB_ADDSTRING, 0, survival.as_ptr() as isize);
    SendMessageW(mode_combo, CB_ADDSTRING, 0, creative.as_ptr() as isize);
    SendMessageW(mode_combo, CB_ADDSTRING, 0, adventure.as_ptr() as isize);
    SendMessageW(mode_combo, CB_ADDSTRING, 0, spectator.as_ptr() as isize);
    SendMessageW(
        mode_combo,
        CB_SETCURSEL,
        match mode {
            StoredGameMode::Survival => 0,
            StoredGameMode::Creative => 1,
            StoredGameMode::Adventure => 2,
            StoredGameMode::Spectator => 3,
        },
        0,
    );
    add_form_label(window, &mut controls, "Difficulty", left, top, 229);
    let difficulty_combo = create_form_control(
        window,
        "COMBOBOX",
        "",
        CBS_DROPDOWNLIST | WS_TABSTOP,
        left + 24,
        top + 252,
        412,
        120,
        FORM_WORLD_DIFFICULTY,
    );
    controls.push(difficulty_combo);
    for option in ["Peaceful", "Easy", "Normal", "Hard"] {
        let option = wide(option);
        SendMessageW(difficulty_combo, CB_ADDSTRING, 0, option.as_ptr() as isize);
    }
    SendMessageW(
        difficulty_combo,
        CB_SETCURSEL,
        match difficulty {
            StoredDifficulty::Peaceful => 0,
            StoredDifficulty::Easy => 1,
            StoredDifficulty::Normal => 2,
            StoredDifficulty::Hard => 3,
        },
        0,
    );
    let locked_label = if editing {
        "Lock Difficulty"
    } else {
        "Lock Difficulty"
    };
    let difficulty_lock = create_form_control(
        window,
        "BUTTON",
        locked_label,
        BS_AUTOCHECKBOX | WS_TABSTOP,
        left + 24,
        top + 291,
        412,
        24,
        FORM_WORLD_DIFFICULTY_LOCKED,
    );
    controls.push(difficulty_lock);
    if difficulty_locked {
        SendMessageW(difficulty_lock, BM_SETCHECK, BST_CHECKED as usize, 0);
        EnableWindow(difficulty_combo, 0);
    }
    add_form_label(
        window,
        &mut controls,
        "Options are saved as world metadata. Changing a seed does not regenerate terrain.",
        left,
        top,
        326,
    );
    let error_label = create_form_control(
        window,
        "STATIC",
        "",
        0,
        left + 24,
        top + 352,
        412,
        22,
        FORM_WORLD_ERROR,
    );
    controls.push(error_label);
    let cancel = create_form_control(
        window,
        "BUTTON",
        "Cancel",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 224,
        top + 410,
        100,
        30,
        FORM_WORLD_CANCEL,
    );
    controls.push(cancel);
    let create = create_form_control(
        window,
        "BUTTON",
        if editing {
            "Save Options"
        } else {
            "Create World"
        },
        BS_DEFPUSHBUTTON | WS_TABSTOP,
        left + 336,
        top + 410,
        100,
        30,
        FORM_WORLD_CREATE,
    );
    controls.push(create);
    if controls.iter().any(|control| control.is_null()) {
        for control in controls
            .into_iter()
            .rev()
            .filter(|control| !control.is_null())
        {
            DestroyWindow(control);
        }
        if let Some(app) = APP.as_mut() {
            app.logger
                .event("ERROR", "world_form_failed", "a native form control failed");
        }
        return;
    }
    WORLD_FORM = Some(WorldCreationForm {
        controls,
        world_id,
        name_edit,
        seed_edit,
        mode_combo,
        difficulty_combo,
        difficulty_locked: difficulty_lock,
        error_label,
    });
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.input.clear();
        app.toggle_flight_pending = false;
        if app.cursor_locked {
            app.cursor_locked = false;
            ClipCursor(null());
            ShowCursor(1);
        }
    }
    SetFocus(name_edit);
}

unsafe fn form_control_text(window: Hwnd) -> String {
    let length = GetWindowTextLengthW(window).max(0) as usize;
    let mut text = vec![0u16; length + 1];
    let count = GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&text[..count])
}

fn optional_seed(text: &str) -> Result<u64, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(1));
    }
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|_| ())
    } else {
        text.parse::<u64>().map_err(|_| ())
    }
}

#[cfg(test)]
mod world_creation_tests {
    use super::{optional_seed, player_controller, stored_game_mode, GameMode, StoredGameMode};
    use crate::world_store::PlayerSnapshot;

    #[test]
    fn seed_field_accepts_decimal_and_hex_and_rejects_invalid_values() {
        assert_eq!(optional_seed("42"), Ok(42));
        assert_eq!(optional_seed(" 0x2a "), Ok(42));
        assert_eq!(optional_seed("0XFF"), Ok(255));
        assert!(optional_seed("-1").is_err());
        assert!(optional_seed("0xzz").is_err());
        assert!(optional_seed("18446744073709551616").is_err());
    }

    #[test]
    fn blank_seed_generates_a_value() {
        assert!(optional_seed("").unwrap() > 0);
        assert!(optional_seed("   ").unwrap() > 0);
    }

    #[test]
    fn adventure_mode_survives_player_controller_round_trip() {
        let player = player_controller(&PlayerSnapshot::default(), StoredGameMode::Adventure);
        assert_eq!(player.mode, GameMode::Adventure);
        assert_eq!(stored_game_mode(player.mode), StoredGameMode::Adventure);
    }
}

#[allow(static_mut_refs)]
unsafe fn close_world_creation_form(window: Hwnd) {
    destroy_world_creation_form();
    if let Some(app) = APP.as_mut() {
        app.dialog_open = false;
        app.input.clear();
        app.last_physics = Instant::now();
    }
    SetFocus(window);
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn destroy_world_creation_form() {
    if let Some(form) = WORLD_FORM.take() {
        for control in form.controls.into_iter().rev() {
            if !control.is_null() {
                DestroyWindow(control);
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn submit_world_creation_form(window: Hwnd) {
    let Some((
        world_id,
        name_edit,
        seed_edit,
        mode_combo,
        difficulty_combo,
        difficulty_locked,
        error_label,
    )) = WORLD_FORM.as_ref().map(|form| {
        (
            form.world_id.clone(),
            form.name_edit,
            form.seed_edit,
            form.mode_combo,
            form.difficulty_combo,
            form.difficulty_locked,
            form.error_label,
        )
    })
    else {
        return;
    };
    let name = form_control_text(name_edit);
    if name.trim().is_empty() {
        let error = wide("World name cannot be empty.");
        SetWindowTextW(error_label, error.as_ptr());
        return;
    }
    let seed = match optional_seed(&form_control_text(seed_edit)) {
        Ok(seed) => seed,
        Err(()) => {
            let error = wide("Seed must be an unsigned decimal or 0x hexadecimal value.");
            SetWindowTextW(error_label, error.as_ptr());
            return;
        }
    };
    let selected_mode = SendMessageW(mode_combo, CB_GETCURSEL, 0, 0);
    let mode = match selected_mode {
        0 => StoredGameMode::Survival,
        1 => StoredGameMode::Creative,
        2 => StoredGameMode::Adventure,
        3 => StoredGameMode::Spectator,
        _ => {
            let error = wide("Choose a supported game mode.");
            SetWindowTextW(error_label, error.as_ptr());
            return;
        }
    };
    let selected_difficulty = SendMessageW(difficulty_combo, CB_GETCURSEL, 0, 0);
    let difficulty = match selected_difficulty {
        0 => StoredDifficulty::Peaceful,
        1 => StoredDifficulty::Easy,
        2 => StoredDifficulty::Normal,
        3 => StoredDifficulty::Hard,
        _ => {
            let error = wide("Choose a supported difficulty.");
            SetWindowTextW(error_label, error.as_ptr());
            return;
        }
    };
    let is_locked = SendMessageW(difficulty_locked, BM_GETCHECK, 0, 0) == BST_CHECKED;
    let Some(app) = APP.as_mut() else {
        return;
    };
    let Some(store) = app.world_store.clone() else {
        let error = wide("World storage is unavailable.");
        SetWindowTextW(error_label, error.as_ptr());
        return;
    };
    if world_id.is_none()
        || app
            .world_metadata
            .as_ref()
            .is_some_and(|active| world_id.as_deref() == Some(active.id.as_str()))
    {
        if let Some(scene) = SCENE.as_mut() {
            if scene.dirty {
                if let Err(save_error) = persist_world(app, scene) {
                    app.logger.event(
                        "ERROR",
                        "world_options_blocked_by_save_failure",
                        save_error.to_string(),
                    );
                    let error =
                        wide("The current world could not be saved. Options were not changed.");
                    SetWindowTextW(error_label, error.as_ptr());
                    return;
                }
            }
        }
    }
    if let Some(id) = world_id {
        match store.update_options(&id, &name, seed, mode, difficulty, is_locked) {
            Ok(updated) => {
                app.logger.event(
                    "INFO",
                    "world_options_updated",
                    format!(
                        "id={} mode={mode:?} difficulty={difficulty:?} locked={is_locked}",
                        updated.metadata.id
                    ),
                );
                destroy_world_creation_form();
                app.dialog_open = false;
                app.input.clear();
                app.last_physics = Instant::now();
                if app
                    .world_metadata
                    .as_ref()
                    .is_some_and(|active| active.id == id)
                {
                    activate_world(window, app, updated);
                } else {
                    install_main_menu(window, app);
                }
            }
            Err(error) => {
                app.logger
                    .event("ERROR", "world_options_update_failed", error.to_string());
                let error = wide(&error.to_string());
                SetWindowTextW(error_label, error.as_ptr());
            }
        }
        return;
    }
    let generator = TerrainGenerator {
        seed,
        ..TerrainGenerator::default()
    };
    let world = generator.generate_region(2);
    match store.create_with_options(
        &name,
        seed,
        mode,
        difficulty,
        is_locked,
        world,
        PlayerSnapshot::default(),
    ) {
        Ok(created) => {
            app.logger.event(
                "INFO",
                "world_created",
                format!("id={} seed={seed} mode={mode:?}", created.metadata.id),
            );
            destroy_world_creation_form();
            app.dialog_open = false;
            app.input.clear();
            app.last_physics = Instant::now();
            activate_world(window, app, created);
        }
        Err(error) => {
            app.logger
                .event("ERROR", "world_create_failed", error.to_string());
            let error = wide(&error.to_string());
            SetWindowTextW(error_label, error.as_ptr());
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn copy_world_from_menu(window: Hwnd, app: &mut AppState, id: &str) {
    if app
        .world_metadata
        .as_ref()
        .is_some_and(|active| active.id == id)
    {
        if let Some(scene) = SCENE.as_mut() {
            if scene.dirty {
                if let Err(error) = persist_world(app, scene) {
                    app.logger.event(
                        "ERROR",
                        "world_copy_blocked_by_save_failure",
                        error.to_string(),
                    );
                    return;
                }
            }
        }
    }
    let Some(store) = app.world_store.clone() else {
        app.logger.event(
            "ERROR",
            "world_copy_unavailable",
            "world storage is unavailable",
        );
        return;
    };
    let source_name = match store.load(id) {
        Ok(world) => world.metadata.name,
        Err(error) => {
            app.logger
                .event("ERROR", "world_copy_failed", error.to_string());
            return;
        }
    };
    let existing_names = match store.list() {
        Ok(worlds) => worlds
            .into_iter()
            .filter_map(|world| world.name)
            .collect::<std::collections::BTreeSet<_>>(),
        Err(error) => {
            app.logger
                .event("ERROR", "world_copy_failed", error.to_string());
            return;
        }
    };
    let base_name = format!("Copy of {source_name}");
    let mut name = base_name.clone();
    let mut suffix = 2;
    while existing_names.contains(&name) {
        name = format!("{base_name} ({suffix})");
        suffix += 1;
    }
    match store.copy(id, &name) {
        Ok(copied) => {
            app.logger.event(
                "INFO",
                "world_copied",
                format!("source={id} copy={}", copied.metadata.id),
            );
            install_main_menu(window, app);
        }
        Err(error) => app
            .logger
            .event("ERROR", "world_copy_failed", error.to_string()),
    }
}

#[allow(static_mut_refs)]
unsafe fn delete_world_from_menu(window: Hwnd, id: &str) {
    let Some((store, name, active_id)) = APP.as_ref().and_then(|app| {
        let store = app.world_store.clone()?;
        let name = store
            .load(id)
            .map(|world| world.metadata.name)
            .unwrap_or_else(|_| id.to_owned());
        let active_id = app
            .world_metadata
            .as_ref()
            .map(|metadata| metadata.id.clone());
        Some((store, name, active_id))
    }) else {
        return;
    };
    if active_id.as_deref() == Some(id) {
        if let Some(app) = APP.as_ref() {
            app.logger.event(
                "WARN",
                "world_delete_rejected",
                "cannot delete the active world",
            );
        }
        return;
    }
    let prompt = wide(&format!(
        "Permanently delete '{name}' and its backups? This cannot be undone."
    ));
    let caption = wide("Delete World");
    if MessageBoxW(
        window,
        prompt.as_ptr(),
        caption.as_ptr(),
        MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
    ) != IDYES
    {
        return;
    }
    match store.delete(id) {
        Ok(()) => {
            if let Some(app) = APP.as_mut() {
                app.logger
                    .event("INFO", "world_deleted", format!("id={id} name={name}"));
                install_main_menu(window, app);
            }
        }
        Err(error) => {
            if let Some(app) = APP.as_ref() {
                app.logger
                    .event("ERROR", "world_delete_failed", error.to_string());
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn restore_backup_from_menu(window: Hwnd, id: &str, backup: &str) {
    let Some((store, name)) = APP.as_ref().and_then(|app| {
        let store = app.world_store.clone()?;
        let name = store
            .load(id)
            .map(|world| world.metadata.name)
            .unwrap_or_else(|_| id.to_owned());
        Some((store, name))
    }) else {
        return;
    };
    let prompt = wide(&format!(
        "Restore backup '{backup}' for '{name}'? The current world state will be preserved as a backup."
    ));
    let caption = wide("Restore World Backup");
    if MessageBoxW(
        window,
        prompt.as_ptr(),
        caption.as_ptr(),
        MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
    ) != IDYES
    {
        return;
    }

    let Some(app) = APP.as_mut() else {
        return;
    };
    let active = app
        .world_metadata
        .as_ref()
        .is_some_and(|metadata| metadata.id == id);
    let restored = if active {
        while let Some(breaking) = app.pending_block_actions.pop_front() {
            if let Some(scene) = SCENE.as_mut() {
                handle_block_action(app, scene, breaking);
            }
        }
        let player = current_player_snapshot(app);
        let Some(scene) = SCENE.as_ref() else {
            return;
        };
        let Some(metadata) = app.world_metadata.as_mut() else {
            return;
        };
        store.restore_backup_with_snapshot(id, backup, Some((metadata, &scene.world, player)))
    } else {
        store.restore_backup(id, backup)
    };
    match restored {
        Ok(saved) => {
            app.logger.event(
                "INFO",
                "world_backup_restored",
                format!("world={id} backup={backup}"),
            );
            if active {
                activate_world(window, app, saved);
            } else {
                install_main_menu(window, app);
            }
        }
        Err(error) => app.logger.event(
            "ERROR",
            "world_backup_restore_failed",
            format!("world={id} backup={backup} error={error}"),
        ),
    }
}

#[allow(static_mut_refs)]
unsafe fn open_world_from_menu(window: Hwnd, app: &mut AppState, id: &str) {
    if app
        .world_metadata
        .as_ref()
        .is_some_and(|metadata| metadata.id == id)
    {
        return;
    }
    if let Some(scene) = SCENE.as_mut() {
        if scene.dirty {
            if let Err(error) = persist_world(app, scene) {
                app.logger.event(
                    "ERROR",
                    "world_switch_blocked_by_save_failure",
                    error.to_string(),
                );
                return;
            }
        }
    }
    let Some(store) = app.world_store.clone() else {
        app.logger.event(
            "ERROR",
            "world_open_unavailable",
            "world storage is unavailable",
        );
        return;
    };
    match store.load(id) {
        Ok(saved) => activate_world(window, app, saved),
        Err(error) => app
            .logger
            .event("ERROR", "world_open_failed", error.to_string()),
    }
}

unsafe fn activate_world(window: Hwnd, app: &mut AppState, saved: crate::world_store::StoredWorld) {
    app.player = player_controller(&saved.player, saved.metadata.game_mode);
    app.selected_block = saved.player.selected_block;
    app.inventory = saved.player.inventory;
    app.drops = saved.player.drops.clone();
    app.yaw = saved.player.yaw;
    app.pitch = saved.player.pitch;
    app.world_metadata = Some(saved.metadata);
    app.input.clear();
    app.pending_block_actions.clear();
    app.jump_was_pressed = false;
    app.last_jump_press = None;
    app.last_physics = Instant::now();
    let scene = build_scene(saved.world);
    if let Some(gpu) = app.gpu.as_mut() {
        let mesh_refs: Vec<&Mesh> = scene.meshes.iter().collect();
        gpu.upload_meshes(&mesh_refs, &scene.materials);
    } else {
        FRAME = Some(build_frame(
            app.camera,
            app.render_width,
            app.render_height,
            &scene,
        ));
    }
    SCENE = Some(scene);
    install_main_menu(window, app);
    let world_name = app
        .world_metadata
        .as_ref()
        .map(|metadata| metadata.name.as_str())
        .unwrap_or("Unknown World");
    let title = wide(&format!("Voxel Engine 0.8.8 | {world_name}"));
    SetWindowTextW(window, title.as_ptr());
    InvalidateRect(window, null(), 0);
    app.logger.event("INFO", "world_opened", world_name);
}

#[allow(static_mut_refs)]
pub fn run(logger: Option<Logger>) {
    let logger = logger.unwrap_or_else(|| Logger::start("voxel-engine.log").expect("logger"));
    let loaded = WorldStore::default_root().and_then(|root| {
        let store = WorldStore::new(root);
        let generated = TerrainGenerator::default().generate_region(2);
        let saved = store.open_or_create_prototype(generated)?;
        Ok((store, saved))
    });
    let (world_store, world_metadata, initial_player, world) = match loaded {
        Ok((store, saved)) => (Some(store), Some(saved.metadata), saved.player, saved.world),
        Err(error) => {
            logger.event(
                "ERROR",
                "world_open_failed",
                format!("falling back to an unsaved prototype world: {error}"),
            );
            (
                None,
                None,
                PlayerSnapshot::default(),
                TerrainGenerator::default().generate_region(2),
            )
        }
    };
    let mode = world_metadata
        .as_ref()
        .map(|metadata| metadata.game_mode)
        .unwrap_or(StoredGameMode::Creative);
    let player = player_controller(&initial_player, mode);
    let scene = build_scene(world);
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    unsafe {
        APP = Some(AppState {
            camera: Camera::default(),
            input: InputState::default(),
            yaw: initial_player.yaw,
            pitch: initial_player.pitch,
            cursor_locked: true,
            width,
            height,
            render_width: (width as u32).min(MAX_RENDER_WIDTH),
            render_height: (height as u32).min(MAX_RENDER_HEIGHT),
            logger,
            last_physics: Instant::now(),
            gpu: None,
            selected_block: initial_player.selected_block,
            inventory: initial_player.inventory,
            drops: initial_player.drops.clone(),
            last_jump_press: None,
            jump_was_pressed: false,
            toggle_flight_pending: false,
            pending_block_actions: VecDeque::new(),
            world_store,
            world_metadata,
            menu_world_ids: Vec::new(),
            menu_restore_actions: Vec::new(),
            dialog_open: false,
            last_save: Instant::now(),
            player,
        });
        if let Some(app) = APP.as_ref() {
            app.logger.event(
                "INFO",
                "window_start",
                format!("width={} height={}", width, height),
            );
        }
        SCENE = Some(scene);
        FRAME = Some(build_frame(
            Camera::default(),
            (width as u32).min(MAX_RENDER_WIDTH),
            (height as u32).min(MAX_RENDER_HEIGHT),
            SCENE.as_ref().unwrap(),
        ));
    }
    let class = wide("VoxelEngineWindow");
    let title = wide("Voxel Engine 0.8.8 Creative Block Interaction Prototype");
    unsafe {
        let instance = GetModuleHandleW(null());
        let wnd_class = WndClassW {
            style: CS_HREDRAW | CS_VREDRAW,
            wnd_proc: Some(window_proc),
            extra_class: 0,
            extra_window: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu: null(),
            name: class.as_ptr(),
        };
        RegisterClassW(&wnd_class);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            width,
            height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if let Some(app) = APP.as_mut() {
            install_main_menu(window, app);
        }
        SetTimer(window, TIMER_ID, TICK_INTERVAL_MS, null_mut());
        SetTimer(window, RENDER_TIMER_ID, 16, null_mut());
        if let Some(app) = APP.as_mut() {
            match GpuRenderer::new(window as isize, width as u32, height as u32) {
                Ok(mut gpu) => {
                    let scene = SCENE.as_ref().unwrap();
                    let mesh_refs: Vec<&Mesh> = scene.meshes.iter().collect();
                    gpu.upload_meshes(&mesh_refs, &scene.materials);
                    app.gpu = Some(gpu);
                    app.logger.event("INFO", "render_backend", "wgpu_textured");
                }
                Err(error) => app.logger.event("WARN", "gpu_unavailable", error),
            }
        }
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        ShowCursor(0);
        lock_cursor(window);
        let mut message = Message {
            hwnd: null_mut(),
            message: 0,
            wparam: 0,
            lparam: 0,
            time: 0,
            point: Point { x: 0, y: 0 },
            private: 0,
        };
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            if WORLD_FORM.is_some() && IsDialogMessageW(window, &message) != 0 {
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn build_scene(world: crate::world::World) -> Scene {
    const REGION_RADIUS: i32 = 2;
    let mut materials = MaterialRegistry::default();
    let extracted = std::path::Path::new("assets/extracted/textures/blocks");
    let mut texture = |name: &str| {
        load_png(extracted.join(name))
            .ok()
            .and_then(|image| materials.register_texture(image).ok())
    };
    let dirt = texture("dirt.png");
    let grass = texture("grass_block_side.png");
    let grass_top = texture("grass_block_top.png");
    let stone = texture("stone.png");
    drop(texture);
    materials
        .register(
            SOIL,
            Material {
                color: [110, 160, 86],
                texture: dirt,
            },
        )
        .unwrap();
    materials
        .register(
            SURFACE,
            Material {
                color: [150, 105, 66],
                texture: grass,
            },
        )
        .unwrap();
    materials
        .register(
            ROCK,
            Material {
                color: [106, 106, 112],
                texture: stone,
            },
        )
        .unwrap();
    let block_textures = [None, dirt, grass, stone];
    let mesh_positions: Vec<_> = (-REGION_RADIUS..=REGION_RADIUS)
        .flat_map(|z| (-REGION_RADIUS..=REGION_RADIUS).map(move |x| ChunkPos { x, y: 0, z }))
        .collect();
    let meshes = mesh_positions
        .iter()
        .map(|position| build_chunk_mesh(&world, *position, block_textures, grass_top.or(dirt)))
        .collect();
    Scene {
        world,
        meshes,
        mesh_positions,
        materials,
        block_textures,
        grass_top_texture: grass_top.or(dirt),
        dirty: false,
    }
}

fn build_chunk_mesh(
    world: &crate::world::World,
    position: ChunkPos,
    block_textures: [Option<u16>; 4],
    grass_top: Option<u16>,
) -> Mesh {
    let Some(chunk) = world.get_chunk(position) else {
        return Mesh::default();
    };
    let mut mesh = greedy_mesh_chunk(chunk, |local_x, local_y, local_z| {
        let world_position = [
            position.x * CHUNK_SIZE as i32 + local_x,
            position.y * CHUNK_SIZE as i32 + local_y,
            position.z * CHUNK_SIZE as i32 + local_z,
        ];
        world
            .block_at(world_position)
            .is_some_and(|block| BlockRegistry::new().is_solid(block))
    });
    for vertex in &mut mesh.vertices {
        vertex.position.x += (position.x * CHUNK_SIZE as i32) as f32;
        vertex.position.y += (position.y * CHUNK_SIZE as i32) as f32;
        vertex.position.z += (position.z * CHUNK_SIZE as i32) as f32;
        vertex.texture = match (vertex.block, vertex.normal.y) {
            (SURFACE, y) if y > 0.5 => grass_top,
            (SURFACE, y) if y < -0.5 => block_textures[1],
            (SURFACE, _) => block_textures[2],
            (SOIL, _) => block_textures[1],
            (ROCK, _) => block_textures[3],
            _ => None,
        };
    }
    mesh.recalculate_bounds();
    mesh
}

fn rebuild_meshes_for_block(scene: &mut Scene, world_position: [i32; 3]) {
    let chunk_position = ChunkPos {
        x: world_position[0].div_euclid(CHUNK_SIZE as i32),
        y: world_position[1].div_euclid(CHUNK_SIZE as i32),
        z: world_position[2].div_euclid(CHUNK_SIZE as i32),
    };
    let local = [
        world_position[0].rem_euclid(CHUNK_SIZE as i32),
        world_position[1].rem_euclid(CHUNK_SIZE as i32),
        world_position[2].rem_euclid(CHUNK_SIZE as i32),
    ];
    let mut affected = vec![chunk_position];
    for axis in 0..3 {
        let direction = if local[axis] == 0 {
            Some(-1)
        } else if local[axis] == CHUNK_SIZE as i32 - 1 {
            Some(1)
        } else {
            None
        };
        if let Some(direction) = direction {
            let mut neighbor = chunk_position;
            match axis {
                0 => neighbor.x += direction,
                1 => neighbor.y += direction,
                _ => neighbor.z += direction,
            }
            affected.push(neighbor);
        }
    }
    let world = &scene.world;
    let block_textures = scene.block_textures;
    let grass_top = scene.grass_top_texture;
    for position in affected {
        if let Some(index) = scene
            .mesh_positions
            .iter()
            .position(|entry| *entry == position)
        {
            scene.meshes[index] = build_chunk_mesh(world, position, block_textures, grass_top);
        }
    }
}

fn queue_block_action(app: &mut AppState, breaking: bool) {
    if app.pending_block_actions.len() >= 16 {
        app.logger
            .event("WARN", "block_action_queue_full", "dropping excess input");
        return;
    }
    app.pending_block_actions.push_back(breaking);
}

fn handle_block_action(app: &mut AppState, scene: &mut Scene, breaking: bool) {
    if !app.player.mode.can_edit_blocks() {
        app.logger.event(
            "INFO",
            "block_action_rejected",
            format!("mode={:?}", app.player.mode),
        );
        return;
    }
    let player = app.player.transform;
    let origin = [player.x, player.y + PLAYER_EYE_OFFSET, player.z];
    let direction = [
        app.pitch.cos() * app.yaw.cos(),
        app.pitch.sin(),
        app.pitch.cos() * app.yaw.sin(),
    ];
    let Some(hit) = raycast_blocks(
        &scene.world,
        &BlockRegistry::new(),
        origin,
        [
            direction[0] as f64,
            direction[1] as f64,
            direction[2] as f64,
        ],
        PROTOTYPE_INTERACTION_REACH,
    ) else {
        app.logger
            .event("TRACE", "block_action_miss", "no target in reach");
        return;
    };
    let position = hit.position.0;
    let changed = if breaking {
        break_block(&mut scene.world, hit).is_some()
    } else {
        let player = app.player.transform;
        let half = app.player.half_extents;
        place_full_block(
            &mut scene.world,
            &BlockRegistry::new(),
            hit,
            app.selected_block,
            Aabb {
                min: [player.x - half[0], player.y - half[1], player.z - half[2]],
                max: [player.x + half[0], player.y + half[1], player.z + half[2]],
            },
        )
    };
    if changed {
        scene.dirty = true;
        let changed_position = if breaking {
            position
        } else {
            [
                position[0] + hit.face_normal[0],
                position[1] + hit.face_normal[1],
                position[2] + hit.face_normal[2],
            ]
        };
        rebuild_meshes_for_block(scene, changed_position);
        if let Some(gpu) = app.gpu.as_mut() {
            let mesh_refs: Vec<&Mesh> = scene.meshes.iter().collect();
            gpu.upload_meshes(&mesh_refs, &scene.materials);
        }
    }
    app.logger.event(
        if changed { "INFO" } else { "TRACE" },
        if breaking {
            "block_break"
        } else {
            "block_place"
        },
        format!(
            "target={},{},{} block={} changed={changed}",
            position[0], position[1], position[2], hit.block.0
        ),
    );
}

fn player_controller(snapshot: &PlayerSnapshot, mode: StoredGameMode) -> PlayerController {
    let mut player = PlayerController::new(crate::entity::Transform {
        x: snapshot.position[0],
        y: snapshot.position[1],
        z: snapshot.position[2],
    });
    player.previous_transform = player.transform;
    player.velocity = snapshot.velocity;
    player.mode = match mode {
        StoredGameMode::Creative => GameMode::Creative,
        StoredGameMode::Adventure => GameMode::Adventure,
        StoredGameMode::Spectator => GameMode::Spectator,
        StoredGameMode::Survival => GameMode::Survival,
    };
    player.flying = snapshot.flying;
    player.grounded = snapshot.grounded;
    player
}

fn stored_game_mode(mode: GameMode) -> StoredGameMode {
    match mode {
        GameMode::Survival => StoredGameMode::Survival,
        GameMode::Creative => StoredGameMode::Creative,
        GameMode::Adventure => StoredGameMode::Adventure,
        GameMode::Spectator => StoredGameMode::Spectator,
    }
}

fn current_player_snapshot(app: &AppState) -> PlayerSnapshot {
    PlayerSnapshot {
        position: [
            app.player.transform.x,
            app.player.transform.y,
            app.player.transform.z,
        ],
        velocity: app.player.velocity,
        yaw: app.yaw,
        pitch: app.pitch,
        flying: app.player.flying,
        grounded: app.player.grounded,
        selected_block: app.selected_block,
        inventory: app.inventory,
        drops: app.drops.clone(),
    }
}

fn persist_world(app: &mut AppState, scene: &mut Scene) -> std::io::Result<()> {
    let snapshot = current_player_snapshot(app);
    let game_mode = stored_game_mode(app.player.mode);
    let store = app.world_store.as_ref().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "world storage is unavailable")
    })?;
    let metadata = app.world_metadata.as_mut().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "world metadata is unavailable",
        )
    })?;
    metadata.game_mode = game_mode;
    store.save(metadata, &scene.world, snapshot)?;
    scene.dirty = false;
    app.last_save = Instant::now();
    Ok(())
}

fn build_frame(camera: Camera, width: u32, height: u32, scene: &Scene) -> (Vec<u8>, BitmapInfo) {
    let mesh_refs: Vec<&Mesh> = scene.meshes.iter().collect();
    let render_width = width * RENDER_SCALE;
    let render_height = height * RENDER_SCALE;
    let frame = CpuRenderer {
        clear_color: [92, 142, 190],
    }
    .render(
        &mesh_refs,
        &camera,
        &scene.materials,
        DirectionalLight::default(),
        render_width,
        render_height,
    )
    .unwrap();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 3];
            for sy in 0..RENDER_SCALE {
                for sx in 0..RENDER_SCALE {
                    let index =
                        ((y * RENDER_SCALE + sy) * render_width + x * RENDER_SCALE + sx) as usize;
                    let pixel = frame.color[index];
                    sum[0] += pixel[0] as u32;
                    sum[1] += pixel[1] as u32;
                    sum[2] += pixel[2] as u32;
                }
            }
            let samples = RENDER_SCALE * RENDER_SCALE;
            pixels.extend([
                (sum[2] / samples) as u8,
                (sum[1] / samples) as u8,
                (sum[0] / samples) as u8,
                0,
            ]);
        }
    }
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: size_of::<BitmapInfoHeader>() as u32,
            width: width as i32,
            height: -(height as i32),
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_pels: 0,
            y_pels: 0,
            used: 0,
            important: 0,
        },
        colors: [[0; 4]; 1],
    };
    (pixels, info)
}
