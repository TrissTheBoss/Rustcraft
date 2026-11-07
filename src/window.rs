//! Minimal native Windows presentation layer for the dependency-free prototype.
#![cfg(windows)]

use std::collections::VecDeque;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use std::time::Instant;

use crate::blocks::Aabb;
use crate::blocks::BlockRegistry;
use crate::drops::{BlockLootTable, ItemDrops, ITEM_PICKUP_DELAY_TICKS};
#[cfg(windows)]
use crate::gpu::GpuRenderer;
use crate::grid::PLAYER_EYE_OFFSET;
use crate::interaction::{
    break_block, break_block_with_drop, place_block, place_hotbar_block, raycast_blocks,
    BlockRayHit, PROTOTYPE_INTERACTION_REACH,
};
use crate::inventory::{
    ArmorSlot, ClickButton, Inventory, InventoryMenu, InventorySlot, ItemCatalog, ItemDefinition,
    ItemId, ItemKey,
};
use crate::logger::Logger;
use crate::mining::{MiningCatalog, MiningEvent, MiningSession, MiningTool};
use crate::movement::{FixedStepClock, GameMode, MovementInput, PlayerController};
use crate::render::{
    load_png, mesh_chunk_with_shapes, Camera, CpuRenderer, DirectionalLight, InputKey, InputState,
    Material, MaterialRegistry, Mesh, Renderer, Vec3,
};
use crate::settings::{
    BindingAction, HoldToggle, KeyBindingError, KeyBindings, Settings, SettingsStore,
};
use crate::survival::PlayerVitals;
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
const WM_DRAWITEM: u32 = 0x002B;
const WM_CTLCOLORSTATIC: u32 = 0x0138;
const WM_TIMER: u32 = 0x0113;
const WM_APP_DEATH: u32 = 0x8001;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_MOUSEWHEEL: u32 = 0x020A;
const MOUSE_WHEEL_DELTA: i32 = 120;
const CS_HREDRAW: u32 = 0x0002;
const CS_VREDRAW: u32 = 0x0001;
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const SW_SHOW: i32 = 5;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC0020;
const TRANSPARENT: i32 = 1;
const DT_LEFT: u32 = 0x0000;
const DT_CENTER: u32 = 0x0001;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;
const TIMER_ID: usize = 1;
const RENDER_TIMER_ID: usize = 2;
const DEATH_RESPAWN_ID: usize = 9001;
const DEATH_TITLE_ID: usize = 9002;
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
const FORM_WORLD_KEEP_INVENTORY: usize = 1109;
const DIALOG_IDOK: usize = 1;
const DIALOG_IDCANCEL: usize = 2;
const INVENTORY_SLOT_ID_BASE: usize = 8000;
const INVENTORY_SLOT_COUNT: usize = 41;
const INVENTORY_MODE_ID: usize = INVENTORY_SLOT_ID_BASE + INVENTORY_SLOT_COUNT;
const INVENTORY_DROP_ID: usize = INVENTORY_MODE_ID + 1;
const INVENTORY_CLOSE_ID: usize = INVENTORY_DROP_ID + 1;
const INVENTORY_CATALOG_SEARCH_ID: usize = 8200;
const INVENTORY_CATALOG_ITEM_ID_BASE: usize = 8210;
const INVENTORY_CATALOG_ITEM_COUNT: usize = 11;
const PAUSE_RESUME_ID: usize = 8100;
const PAUSE_SAVE_QUIT_ID: usize = PAUSE_RESUME_ID + 1;
const PAUSE_OPTIONS_ID: usize = PAUSE_SAVE_QUIT_ID + 1;
const PAUSE_TITLE_ID: usize = PAUSE_OPTIONS_ID + 1;
const TITLE_SINGLEPLAYER_ID: usize = 9000;
const TITLE_OPTIONS_ID: usize = TITLE_SINGLEPLAYER_ID + 1;
const TITLE_QUIT_ID: usize = TITLE_OPTIONS_ID + 1;
const TITLE_PLAY_ID: usize = TITLE_QUIT_ID + 1;
const OPTIONS_FOV_ID: usize = 8110;
const OPTIONS_SENSITIVITY_ID: usize = OPTIONS_FOV_ID + 1;
const OPTIONS_INVERT_ID: usize = OPTIONS_SENSITIVITY_ID + 1;
const OPTIONS_SPRINT_TOGGLE_ID: usize = OPTIONS_INVERT_ID + 1;
const OPTIONS_SNEAK_TOGGLE_ID: usize = OPTIONS_SPRINT_TOGGLE_ID + 1;
const OPTIONS_APPLY_ID: usize = OPTIONS_SNEAK_TOGGLE_ID + 1;
const OPTIONS_DEFAULTS_ID: usize = OPTIONS_APPLY_ID + 1;
const OPTIONS_BACK_ID: usize = OPTIONS_DEFAULTS_ID + 1;
const OPTIONS_BINDING_ID_BASE: usize = 8120;
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
const BS_OWNERDRAW: u32 = 0x000B;
const ODT_BUTTON: u32 = 4;
const ODS_SELECTED: u32 = 0x0001;
const ODS_DISABLED: u32 = 0x0004;
const ODS_FOCUS: u32 = 0x0010;
const HOLLOW_BRUSH: i32 = 5;
const IDYES: i32 = 6;
const MB_YESNO: u32 = 0x0004;
const MB_ICONWARNING: u32 = 0x0030;
const MB_DEFBUTTON2: u32 = 0x0100;
const TPM_RIGHTBUTTON: u32 = 0x0002;
const TPM_RETURNCMD: u32 = 0x0100;
const TICK_INTERVAL_MS: u32 = 50;
const VK_ESCAPE: u32 = 0x1B;
const VK_CONTROL: u32 = 0x11;
const VK_E: u32 = 0x45;
const VK_F: u32 = 0x46;
const VK_Q: u32 = 0x51;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_KILLFOCUS: u32 = 0x0008;
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
#[derive(Clone, Copy)]
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
struct DrawItemStruct {
    control_type: u32,
    control_id: u32,
    item_id: u32,
    item_action: u32,
    item_state: u32,
    item_window: Hwnd,
    hdc: Hdc,
    item_rect: Rect,
    item_data: usize,
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
    fn GetSubMenu(menu: *mut c_void, position: i32) -> *mut c_void;
    fn DestroyMenu(menu: *mut c_void) -> i32;
    fn PostMessageW(window: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> i32;
    fn MessageBoxW(window: Hwnd, text: *const u16, caption: *const u16, flags: u32) -> i32;
    fn SendMessageW(window: Hwnd, message: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
    fn RedrawWindow(window: Hwnd, rect: *const Rect, region: *mut c_void, flags: u32) -> i32;
    fn GetParent(window: Hwnd) -> Hwnd;
    fn MoveWindow(window: Hwnd, x: i32, y: i32, width: i32, height: i32, repaint: i32) -> i32;
    fn GetWindowTextLengthW(window: Hwnd) -> i32;
    fn GetWindowTextW(window: Hwnd, text: *mut u16, max_chars: i32) -> i32;
    fn GetKeyState(key: i32) -> i16;
    fn SetFocus(window: Hwnd) -> Hwnd;
    fn IsDialogMessageW(window: Hwnd, message: *const Message) -> i32;
    fn EnableWindow(window: Hwnd, enable: i32) -> i32;
    fn DrawMenuBar(window: Hwnd) -> i32;
    fn DestroyWindow(window: Hwnd) -> i32;
    fn ClipCursor(rect: *const Rect) -> i32;
    fn ShowCursor(show: i32) -> i32;
    fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn GetClientRect(window: Hwnd, rect: *mut Rect) -> i32;
    fn DrawTextW(hdc: Hdc, text: *const u16, count: i32, rect: *mut Rect, format: u32) -> i32;
    fn ClientToScreen(window: Hwnd, point: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn TrackPopupMenu(
        menu: *mut c_void,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        owner: Hwnd,
        rect: *const Rect,
    ) -> u32;
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
    fn SetBkMode(hdc: Hdc, mode: i32) -> i32;
    fn SetTextColor(hdc: Hdc, color: u32) -> u32;
    fn GetStockObject(object: i32) -> *mut c_void;
    fn CreateSolidBrush(color: u32) -> *mut c_void;
    fn FillRect(hdc: Hdc, rect: *const Rect, brush: *mut c_void) -> i32;
    fn DeleteObject(object: *mut c_void) -> i32;
    fn CreateBitmap(
        width: i32,
        height: i32,
        planes: u32,
        bits_per_pixel: u32,
        bits: *const c_void,
    ) -> *mut c_void;
    fn CreatePatternBrush(bitmap: *mut c_void) -> *mut c_void;
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
static mut TITLE_FORM: Option<TitleForm> = None;
static mut INVENTORY_FORM: Option<InventoryForm> = None;
static mut PAUSE_FORM: Option<PauseForm> = None;
static mut OPTIONS_FORM: Option<OptionsForm> = None;
static mut DEATH_FORM: Option<DeathForm> = None;
static PANEL_TEXTURE_BRUSH: OnceLock<usize> = OnceLock::new();
static BUTTON_TEXTURE_BRUSH: OnceLock<usize> = OnceLock::new();

struct Scene {
    world: crate::world::World,
    terrain_generator: TerrainGenerator,
    stream_center: (i32, i32),
    failed_stream_center: Option<(i32, i32)>,
    meshes: Vec<Mesh>,
    drop_meshes: Vec<Mesh>,
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
    settings: Settings,
    settings_store: Option<SettingsStore>,
    player: PlayerController,
    vitals: PlayerVitals,
    food: crate::survival::FoodState,
    activity_exhaustion: crate::survival::ActivityExhaustion,
    hunger_clock: crate::survival::HungerClock,
    gpu: Option<GpuRenderer>,
    viewport: Hwnd,
    presenting_dialog: bool,
    last_menu_frame: Instant,
    gpu_frame_count: u32,
    gpu_frame_window: Instant,
    last_physics: Instant,
    physics_clock: FixedStepClock,
    selected_block: crate::world::BlockId,
    hotbar_scroll_remainder: i32,
    inventory: Inventory,
    inventory_menu: InventoryMenu,
    item_catalog: ItemCatalog,
    loot_table: BlockLootTable,
    drops: ItemDrops,
    death_items_retained: bool,
    mining_catalog: MiningCatalog,
    mining_session: MiningSession,
    breaking_held: bool,
    food_use_held: bool,
    food_use: crate::food_use::FoodUseSession,
    sprint_toggled: bool,
    sneak_toggled: bool,
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
    keep_inventory: Hwnd,
    error_label: Hwnd,
}

struct TitleForm {
    controls: Vec<Hwnd>,
    status_label: Hwnd,
}

struct InventoryForm {
    controls: Vec<Hwnd>,
    slot_controls: Vec<Hwnd>,
    catalog_search: Hwnd,
    catalog_buttons: Vec<Hwnd>,
    catalog_visible: Vec<u16>,
    cursor_label: Hwnd,
    status_label: Hwnd,
    mode_button: Hwnd,
    secondary_click: bool,
    cursor_was_locked: bool,
}

struct PauseForm {
    controls: Vec<Hwnd>,
    status_label: Hwnd,
    cursor_was_locked: bool,
}

struct OptionsForm {
    controls: Vec<Hwnd>,
    field_of_view_edit: Hwnd,
    sensitivity_edit: Hwnd,
    invert_mouse_y_check: Hwnd,
    sprint_toggle_check: Hwnd,
    sneak_toggle_check: Hwnd,
    status_label: Hwnd,
    binding_buttons: Vec<(BindingAction, Hwnd)>,
    key_bindings: KeyBindings,
    capture_action: Option<BindingAction>,
    escape_canceled_down: bool,
    return_to_capture: bool,
    return_to_title: bool,
}

struct DeathForm {
    controls: Vec<Hwnd>,
    status_label: Hwnd,
}

const MF_STRING: u32 = 0x0000;
const MF_SEPARATOR: u32 = 0x0800;
const MF_POPUP: u32 = 0x0010;
const MF_GRAYED: u32 = 0x0001;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(static_mut_refs)]
unsafe extern "system" fn viewport_proc(
    window: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP
        | WM_MBUTTONDOWN | WM_MOUSEWHEEL => {
            SendMessageW(GetParent(window), message, wparam, lparam)
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut paint: PaintStruct = std::mem::zeroed();
            let hdc = BeginPaint(window, &mut paint);
            if let (Some(app), Some(scene)) = (APP.as_ref(), SCENE.as_ref()) {
                if !app.dialog_open {
                    let mut client: Rect = std::mem::zeroed();
                    GetClientRect(window, &mut client);
                    paint_hud(hdc, client, app, scene);
                }
            }
            EndPaint(window, &paint);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

#[allow(static_mut_refs)]
unsafe extern "system" fn window_proc(
    window: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        0x0005 => {
            // WM_SIZE
            let width = (lparam as u32 & 0xffff) as i32;
            let height = ((lparam as u32 >> 16) & 0xffff) as i32;
            if width > 0 && height > 0 {
                if let Some(app) = APP.as_mut() {
                    app.width = width;
                    app.height = height;
                    app.render_width = (width as u32).min(MAX_RENDER_WIDTH);
                    app.render_height = (height as u32).min(MAX_RENDER_HEIGHT);
                    if !app.viewport.is_null() {
                        MoveWindow(app.viewport, 0, 0, width, height, 1);
                    }
                    if let Some(gpu) = app.gpu.as_mut() {
                        gpu.resize(width as u32, height as u32);
                    }
                }
            }
            0
        }
        WM_COMMAND => {
            let command = wparam & 0xffff;
            if handle_title_command(window, command) {
                return 0;
            }
            if handle_inventory_command(window, command, ((wparam >> 16) & 0xffff) as u16) {
                return 0;
            }
            if handle_options_command(window, command) {
                return 0;
            }
            if handle_pause_command(window, command) {
                return 0;
            }
            if handle_death_command(window, command) {
                return 0;
            }
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
            destroy_title_form();
            destroy_death_form();
            if INVENTORY_FORM.is_some() {
                close_inventory_form(window);
                return 0;
            }
            destroy_options_form();
            destroy_pause_form();
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
        WM_DRAWITEM if lparam != 0 => {
            let item = &mut *(lparam as *mut DrawItemStruct);
            if item.control_type == ODT_BUTTON {
                draw_menu_button(item);
                1
            } else {
                DefWindowProcW(window, message, wparam, lparam)
            }
        }
        WM_CTLCOLORSTATIC => {
            SetTextColor(wparam as Hdc, rgb(230, 236, 238));
            SetBkMode(wparam as Hdc, TRANSPARENT);
            let texture = menu_texture_brush(false);
            if texture.is_null() {
                GetStockObject(HOLLOW_BRUSH) as Lresult
            } else {
                texture as Lresult
            }
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
            if !APP
                .as_ref()
                .is_some_and(|app| app.gpu.is_some() && !app.dialog_open)
            {
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
            }
            if let (Some(app), Some(scene)) = (APP.as_ref(), SCENE.as_ref()) {
                if !app.dialog_open && app.gpu.is_none() {
                    let mut client = Rect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    GetClientRect(window, &mut client);
                    paint_hud(hdc, client, app, scene);
                }
            }
            EndPaint(window, &paint);
            0
        }
        WM_ERASEBKGND => 1,
        WM_APP_DEATH => {
            open_death_form(window);
            0
        }
        WM_TIMER => {
            if let Some(app) = APP.as_mut() {
                if wparam == TIMER_ID && !app.dialog_open {
                    let now = Instant::now();
                    let physics_steps = app
                        .physics_clock
                        .advance(now.duration_since(app.last_physics));
                    app.last_physics = now;
                    for _ in 0..physics_steps {
                        if let Some(scene) = SCENE.as_ref() {
                            let previous_transform = app.player.transform;
                            let previous_velocity = app.player.velocity;
                            let previous_flying = app.player.flying;
                            let previous_grounded = app.player.grounded;
                            let previous_mode = app.player.mode;
                            let mut input = MovementInput {
                                forward: app.input.is_pressed(InputKey::Forward) as i32 as f64
                                    - app.input.is_pressed(InputKey::Backward) as i32 as f64,
                                strafe: app.input.is_pressed(InputKey::Right) as i32 as f64
                                    - app.input.is_pressed(InputKey::Left) as i32 as f64,
                                jump: app.input.is_pressed(InputKey::Up),
                                sneak: app.settings.sneak_behavior.active(
                                    app.input.is_pressed(InputKey::Down),
                                    app.sneak_toggled,
                                ),
                                sprint: app.settings.sprint_behavior.active(
                                    app.input.is_pressed(InputKey::Sprint),
                                    app.sprint_toggled,
                                ),
                                toggle_flight: app.toggle_flight_pending,
                            };
                            let uses_hunger =
                                matches!(app.player.mode, GameMode::Survival | GameMode::Adventure)
                                    && !app.vitals.is_dead();
                            if uses_hunger {
                                input.sprint &=
                                    app.food.can_sprint() && input.forward > 0.0 && !input.sneak;
                            }
                            app.toggle_flight_pending = false;
                            app.jump_was_pressed = app.input.is_pressed(InputKey::Up);
                            let fall_distance = app.player.tick_with_collision_shapes(
                                input,
                                app.yaw as f64,
                                |x, y, z| {
                                    let block_position = [x, y, z];
                                    match (
                                        scene.world.block_at(block_position),
                                        scene.world.block_state_at(block_position),
                                    ) {
                                        (Some(block), Some(state)) => {
                                            BlockRegistry::new().collision_shape(block, state)
                                        }
                                        _ => crate::blocks::CollisionShape::EMPTY,
                                    }
                                },
                            );
                            let difficulty = hunger_difficulty(app);
                            if uses_hunger
                                && difficulty != crate::survival::HungerDifficulty::Peaceful
                                && !previous_flying
                                && !app.player.flying
                            {
                                let dx = app.player.transform.x - previous_transform.x;
                                let dz = app.player.transform.z - previous_transform.z;
                                let sprint_distance = if input.sprint && previous_grounded {
                                    dx.hypot(dz)
                                } else {
                                    0.0
                                };
                                let cost = app.activity_exhaustion.record(
                                    sprint_distance,
                                    input.jump && previous_grounded,
                                    input.sprint,
                                );
                                if cost > 0 {
                                    app.food.apply_exhaustion(cost);
                                    if let Some(scene) = SCENE.as_mut() {
                                        scene.dirty = true;
                                    }
                                }
                            }
                            let before_hunger = (app.food, app.vitals);
                            let hunger_death = if uses_hunger {
                                app.hunger_clock
                                    .tick(&mut app.food, &mut app.vitals, difficulty)
                                    .became_dead
                            } else {
                                app.hunger_clock = crate::survival::HungerClock::default();
                                false
                            };
                            if before_hunger != (app.food, app.vitals) {
                                if let Some(scene) = SCENE.as_mut() {
                                    scene.dirty = true;
                                }
                            }
                            if fall_distance.is_some() || hunger_death {
                                let distance = fall_distance.unwrap_or(0.0);
                                if matches!(
                                    app.player.mode,
                                    GameMode::Survival | GameMode::Adventure
                                ) {
                                    let outcome = app.vitals.apply_fall_damage(distance);
                                    if outcome.applied > 0 {
                                        app.logger.event(
                                            "INFO",
                                            "fall_damage",
                                            format!(
                                                "distance={distance:.2} damage={} health={}",
                                                outcome.applied,
                                                app.vitals.health()
                                            ),
                                        );
                                        if let Some(scene) = SCENE.as_mut() {
                                            scene.dirty = true;
                                        }
                                    }
                                    if outcome.became_dead || hunger_death {
                                        app.death_items_retained = false;
                                        let keep_inventory = app
                                            .world_metadata
                                            .as_ref()
                                            .is_some_and(|metadata| metadata.keep_inventory);
                                        if keep_inventory {
                                            app.logger.event(
                                                "INFO",
                                                "death_inventory_kept",
                                                "keepInventory is enabled",
                                            );
                                        } else {
                                            let position = [
                                                app.player.transform.x,
                                                app.player.transform.y,
                                                app.player.transform.z,
                                            ];
                                            match app.inventory.drop_all(
                                                &mut app.drops,
                                                position,
                                                ITEM_PICKUP_DELAY_TICKS,
                                                &app.item_catalog,
                                            ) {
                                                Ok(stack_count) => {
                                                    if let Some(scene) = SCENE.as_mut() {
                                                        scene.dirty = true;
                                                        scene.drop_meshes =
                                                            build_drop_meshes(&app.drops);
                                                        if let Some(gpu) = app.gpu.as_mut() {
                                                            upload_scene_meshes(gpu, scene);
                                                        }
                                                    }
                                                    app.logger.event(
                                                        "INFO",
                                                        "death_inventory_dropped",
                                                        format!("stacks={stack_count}"),
                                                    );
                                                }
                                                Err(error) => {
                                                    app.death_items_retained = true;
                                                    app.logger.event(
                                                        "ERROR",
                                                        "death_inventory_drop_failed",
                                                        error.to_string(),
                                                    );
                                                }
                                            }
                                        }
                                        PostMessageW(window, WM_APP_DEATH, 0, 0);
                                    }
                                }
                            }
                            let center = (
                                (app.player.transform.x / CHUNK_SIZE as f64).floor() as i32,
                                (app.player.transform.z / CHUNK_SIZE as f64).floor() as i32,
                            );
                            if let Some(scene) = SCENE.as_mut() {
                                match scene.stream_to(center) {
                                    Ok(true) => {
                                        app.logger.event(
                                            "INFO",
                                            "terrain_region_loaded",
                                            format!("center={},{}", center.0, center.1),
                                        );
                                        if let Some(gpu) = app.gpu.as_mut() {
                                            upload_scene_meshes(gpu, scene);
                                        }
                                    }
                                    Ok(false) => {}
                                    Err(error) => app.logger.event(
                                        "WARN",
                                        "terrain_region_load_failed",
                                        error,
                                    ),
                                }
                            }
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
                            if app.food_use.tick(
                                app.food_use_held
                                    && !app.vitals.is_dead()
                                    && matches!(
                                        app.player.mode,
                                        GameMode::Survival | GameMode::Adventure
                                    ),
                                &mut app.inventory,
                                &mut app.food,
                            ) {
                                if let Some(scene) = SCENE.as_mut() {
                                    scene.dirty = true;
                                }
                                app.logger
                                    .event("INFO", "food_consumed", "item=prototype_ration");
                            }
                            let mining_target =
                                if app.player.mode == GameMode::Survival && app.breaking_held {
                                    SCENE
                                        .as_ref()
                                        .and_then(|scene| targeted_block_hit(app, &scene.world))
                                } else {
                                    None
                                };
                            let required_ticks = mining_target
                                .map(|hit| {
                                    app.mining_catalog
                                        .ticks_to_break(hit.block, MiningTool::HAND)
                                        .unwrap_or(u16::MAX)
                                })
                                .unwrap_or(0);
                            let mining_event = app.mining_session.tick(
                                app.player.mode == GameMode::Survival && app.breaking_held,
                                mining_target,
                                required_ticks,
                            );
                            if matches!(mining_event, MiningEvent::Completed(_)) {
                                if let Some(scene) = SCENE.as_mut() {
                                    handle_block_action(app, scene, true);
                                }
                            }
                            if !app.drops.is_empty() {
                                if let Some(scene) = SCENE.as_mut() {
                                    let player_position = [
                                        app.player.transform.x,
                                        app.player.transform.y,
                                        app.player.transform.z,
                                    ];
                                    match app.drops.tick(
                                        &scene.world,
                                        player_position,
                                        &mut app.inventory,
                                        &app.item_catalog,
                                        &BlockRegistry::new(),
                                    ) {
                                        Ok(result) => {
                                            scene.dirty = true;
                                            let drop_meshes = build_drop_meshes(&app.drops);
                                            if drop_meshes != scene.drop_meshes {
                                                scene.drop_meshes = drop_meshes;
                                                if let Some(gpu) = app.gpu.as_mut() {
                                                    upload_scene_meshes(gpu, scene);
                                                }
                                            }
                                            if result.picked_up_items > 0
                                                || result.removed_entities > 0
                                            {
                                                app.logger.event(
                                                    "INFO",
                                                    "dropped_item_lifecycle",
                                                    format!(
                                                        "picked_up={} removed={} remaining={}",
                                                        result.picked_up_items,
                                                        result.removed_entities,
                                                        app.drops.len()
                                                    ),
                                                );
                                            }
                                        }
                                        Err(error) => app.logger.event(
                                            "WARN",
                                            "dropped_item_tick_failed",
                                            error.to_string(),
                                        ),
                                    }
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
                            let world_name = app
                                .world_metadata
                                .as_ref()
                                .map(|metadata| metadata.name.as_str())
                                .unwrap_or("Unsaved Prototype");
                            let title = wide(&format!(
                            "Voxel Engine 0.8.8 | {world_name} | Slot {} {} | block={:.2},{:.2},{:.2} speed={:.2} b/s | mine={:.1}%",
                            app.selected_block.0,
                            BlockRegistry::new().definition(app.selected_block).name,
                            app.player.transform.x,
                            app.player.transform.y,
                            app.player.transform.z,
                            (app.player.velocity[0].powi(2) + app.player.velocity[2].powi(2))
                                .sqrt()
                                * 20.0,
                            app.mining_session.progress_permille() as f32 / 10.0
                        ));
                            SetWindowTextW(window, title.as_ptr());
                        }
                    }
                }
                // Keep the title backdrop visibly alive while its modal menu pauses gameplay.
                // last_physics is reset when the title opens and remains unchanged until play.
                let render_yaw = if TITLE_FORM.is_some() {
                    app.yaw + app.last_physics.elapsed().as_secs_f32() * 0.12
                } else {
                    app.yaw
                };
                let direction = Vec3::new(
                    app.pitch.cos() * render_yaw.cos(),
                    app.pitch.sin(),
                    app.pitch.cos() * render_yaw.sin(),
                );
                if wparam == RENDER_TIMER_ID {
                    let alpha = app.physics_clock.interpolation_alpha();
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
                    let transition = app.presenting_dialog != app.dialog_open;
                    if transition {
                        app.presenting_dialog = app.dialog_open;
                        ShowWindow(
                            app.viewport,
                            if app.dialog_open || app.gpu.is_none() {
                                0
                            } else {
                                SW_SHOW
                            },
                        );
                        app.gpu_frame_count = 0;
                        app.gpu_frame_window = Instant::now();
                        app.logger.event(
                            "INFO",
                            "render_backend",
                            if app.dialog_open {
                                "menu_cpu_gdi"
                            } else {
                                "gameplay_wgpu_textured"
                            },
                        );
                        RedrawWindow(window, null(), null_mut(), 0x185);
                    }
                    if let Some(gpu) = app.gpu.as_mut().filter(|_| !app.dialog_open) {
                        if let Err(error) = gpu.render(&app.camera) {
                            app.logger.event("WARN", "gpu_frame_failed", error);
                            app.gpu = None;
                            ShowWindow(app.viewport, 0);
                        } else {
                            app.gpu_frame_count += 1;
                            let seconds = app.gpu_frame_window.elapsed().as_secs_f64();
                            if seconds >= 2.0 {
                                app.logger.event(
                                    "INFO",
                                    "gpu_frame_rate",
                                    format!(
                                        "frames={} seconds={seconds:.3} fps={:.1}",
                                        app.gpu_frame_count,
                                        f64::from(app.gpu_frame_count) / seconds
                                    ),
                                );
                                app.gpu_frame_count = 0;
                                app.gpu_frame_window = Instant::now();
                            }
                        }
                        InvalidateRect(app.viewport, null(), 0);
                    } else if (!app.dialog_open
                        || transition
                        || app.last_menu_frame.elapsed().as_millis() >= 250)
                        && SCENE.is_some()
                    {
                        let scene = SCENE.as_ref().unwrap();
                        FRAME = Some(build_frame(
                            app.camera,
                            app.render_width,
                            app.render_height,
                            scene,
                        ));
                        app.last_menu_frame = Instant::now();
                        if transition && app.dialog_open {
                            RedrawWindow(window, null(), null_mut(), 0x185);
                        }
                    }
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
                    let sensitivity = app.settings.mouse_sensitivity_permille as f32 / 1000.0;
                    app.yaw += (x - center_x) as f32 * 0.006 * sensitivity;
                    let pitch_direction = if app.settings.invert_mouse_y {
                        1.0
                    } else {
                        -1.0
                    };
                    app.pitch = (app.pitch
                        + (y - center_y) as f32 * 0.006 * sensitivity * pitch_direction)
                        .clamp(-1.45, 1.45);
                    if app.yaw != previous_yaw || app.pitch != previous_pitch {
                        if let Some(scene) = SCENE.as_mut() {
                            scene.dirty = true;
                        }
                    }
                    lock_cursor(window);
                }
            }
            0
        }
        WM_KEYDOWN | WM_KEYUP => {
            if TITLE_FORM.is_some() {
                return 0;
            }
            if INVENTORY_FORM.is_some() {
                if wparam as u32 == VK_ESCAPE && message == WM_KEYDOWN {
                    close_inventory_form(window);
                }
                return 0;
            }
            if OPTIONS_FORM.is_some() {
                if OPTIONS_FORM
                    .as_ref()
                    .is_some_and(|form| form.escape_canceled_down)
                {
                    if wparam as u32 == VK_ESCAPE && message == WM_KEYUP {
                        if let Some(form) = OPTIONS_FORM.as_mut() {
                            form.escape_canceled_down = false;
                        }
                    }
                    return 0;
                }
                if let Some(action) = OPTIONS_FORM.as_ref().and_then(|form| form.capture_action) {
                    if message == WM_KEYDOWN && is_initial_keydown(lparam) {
                        if wparam as u32 == VK_ESCAPE {
                            cancel_options_binding_capture(window);
                        } else {
                            capture_options_binding(window, action, wparam as u16);
                        }
                    }
                    return 0;
                }
                if wparam as u32 == VK_ESCAPE && message == WM_KEYDOWN && is_initial_keydown(lparam)
                {
                    close_options_form(window);
                }
                return 0;
            }
            if PAUSE_FORM.is_some() {
                if wparam as u32 == VK_ESCAPE && message == WM_KEYDOWN && is_initial_keydown(lparam)
                {
                    close_pause_form(window);
                }
                return 0;
            }
            if DEATH_FORM.is_some() {
                return 0;
            }
            if wparam as u32 == VK_E && message == WM_KEYDOWN {
                if APP.as_ref().is_some_and(|app| !app.dialog_open) {
                    open_inventory_form(window);
                    return 0;
                }
            }
            if message == WM_KEYDOWN && is_initial_keydown(lparam) {
                match wparam as u32 {
                    VK_Q => {
                        drop_selected_hotbar_items(GetKeyState(VK_CONTROL as i32) < 0);
                        return 0;
                    }
                    VK_F => {
                        swap_hotbar_item_with_offhand();
                        return 0;
                    }
                    _ => {}
                }
            }
            if wparam as u32 == VK_ESCAPE && message == WM_KEYDOWN && is_initial_keydown(lparam) {
                open_pause_form(window);
                return 0;
            }
            if let Some(app) = APP.as_mut() {
                let mapped_keys = key_for(wparam as u32, app.settings.key_bindings);
                for key in mapped_keys.iter().copied() {
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
                    let key_down = message == WM_KEYDOWN;
                    let initial_press = key_down && is_initial_keydown(lparam);
                    match key {
                        InputKey::Sprint if app.settings.sprint_behavior == HoldToggle::Toggle => {
                            app.sprint_toggled = if key_down {
                                HoldToggle::Toggle.after_key_down(app.sprint_toggled, initial_press)
                            } else {
                                HoldToggle::Toggle.after_key_up(app.sprint_toggled)
                            };
                            app.input.set(key, false);
                        }
                        InputKey::Down if app.settings.sneak_behavior == HoldToggle::Toggle => {
                            app.sneak_toggled = if key_down {
                                HoldToggle::Toggle.after_key_down(app.sneak_toggled, initial_press)
                            } else {
                                HoldToggle::Toggle.after_key_up(app.sneak_toggled)
                            };
                            app.input.set(key, false);
                        }
                        _ => app.input.set(key, key_down),
                    }
                    if key == InputKey::Up && message == WM_KEYUP {
                        app.jump_was_pressed = false;
                    }
                }
                if !mapped_keys.is_empty() {
                    app.logger.event(
                        "TRACE",
                        "key_event",
                        format!("key={} pressed={}", wparam, message == WM_KEYDOWN),
                    );
                }
                if message == WM_KEYDOWN && (0x31..=0x39).contains(&(wparam as u32)) {
                    let slot = (wparam as usize) - 0x31;
                    let selection_changed = app.inventory.select_hotbar(slot).is_ok();
                    if let Some(block) = app
                        .inventory
                        .selected_stack()
                        .and_then(|stack| prototype_block_for_item(stack.key))
                    {
                        app.selected_block = block;
                    } else if app.player.mode == GameMode::Creative
                        && app.inventory.selected_stack().is_none()
                        && (0x31..=0x33).contains(&(wparam as u32))
                    {
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
        WM_MOUSEWHEEL => {
            if let Some(app) = APP.as_mut() {
                if app.dialog_open {
                    return 0;
                }
                let delta = i32::from(((wparam >> 16) as u16) as i16);
                app.hotbar_scroll_remainder = app.hotbar_scroll_remainder.saturating_add(delta);
                let steps = app.hotbar_scroll_remainder / MOUSE_WHEEL_DELTA;
                app.hotbar_scroll_remainder %= MOUSE_WHEEL_DELTA;
                if steps != 0 {
                    let slot = hotbar_slot_after_scroll(
                        usize::from(app.inventory.selected_hotbar()),
                        steps,
                    );
                    if app.inventory.select_hotbar(slot).is_ok() {
                        if let Some(block) = app
                            .inventory
                            .selected_stack()
                            .and_then(|stack| prototype_block_for_item(stack.key))
                        {
                            app.selected_block = block;
                        }
                        if let Some(scene) = SCENE.as_mut() {
                            scene.dirty = true;
                        }
                        app.logger.event(
                            "INFO",
                            "hotbar_select",
                            format!("slot={} source=mouse_wheel", slot + 1),
                        );
                    }
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
                    if app.player.mode == GameMode::Survival {
                        app.breaking_held = true;
                    } else {
                        queue_block_action(app, true);
                    }
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
        WM_LBUTTONUP => {
            if let Some(app) = APP.as_mut() {
                app.breaking_held = false;
                app.mining_session.reset();
            }
            0
        }
        WM_KILLFOCUS => {
            if let Some(app) = APP.as_mut() {
                app.input.clear();
                app.food_use_held = false;
                app.food_use.reset();
                app.jump_was_pressed = false;
                app.breaking_held = false;
                app.mining_session.reset();
            }
            0
        }
        WM_MBUTTONDOWN => {
            if let Some(app) = APP.as_mut() {
                if app.dialog_open {
                    return 0;
                }
                if !app.cursor_locked {
                    app.cursor_locked = true;
                    ShowCursor(0);
                    lock_cursor(window);
                    app.logger
                        .event("INFO", "cursor_captured", "source=mouse_click");
                    return 0;
                }
                if app.player.mode != GameMode::Creative {
                    return 0;
                }
                let Some(hit) = SCENE
                    .as_ref()
                    .and_then(|scene| targeted_block_hit(app, &scene.world))
                else {
                    return 0;
                };
                let block = hit.block;
                let key = ItemKey::new(ItemId(block.0));
                if prototype_block_for_item(key).is_none() {
                    return 0;
                }
                match app.inventory.pick_item_to_hotbar(key, &app.item_catalog) {
                    Ok(slot) => {
                        app.selected_block = block;
                        if let Some(scene) = SCENE.as_mut() {
                            scene.dirty = true;
                        }
                        app.logger.event(
                            "INFO",
                            "creative_pick_block",
                            format!("block={} slot={}", block.0, slot + 1),
                        );
                    }
                    Err(error) => {
                        app.logger
                            .event("WARN", "creative_pick_block_rejected", error.to_string())
                    }
                }
            }
            0
        }
        WM_RBUTTONDOWN => {
            if let Some(app) = APP.as_mut() {
                if app.dialog_open {
                    return 0;
                }
                if app.cursor_locked {
                    if app
                        .inventory
                        .selected_stack()
                        .is_some_and(|stack| crate::food_use::is_food(stack.key))
                    {
                        app.food_use_held = true;
                    } else {
                        queue_block_action(app, false);
                    }
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
        WM_RBUTTONUP => {
            if let Some(app) = APP.as_mut() {
                app.food_use_held = false;
                app.food_use.reset();
            }
            0
        }
        WM_DESTROY => {
            destroy_world_creation_form();
            destroy_options_form();
            destroy_pause_form();
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

fn key_for(key: u32, bindings: KeyBindings) -> Vec<InputKey> {
    let Ok(key) = u16::try_from(key) else {
        return Vec::new();
    };
    bindings
        .actions(key)
        .into_iter()
        .map(|action| match action {
            BindingAction::Forward => InputKey::Forward,
            BindingAction::Backward => InputKey::Backward,
            BindingAction::Left => InputKey::Left,
            BindingAction::Right => InputKey::Right,
            BindingAction::Jump => InputKey::Up,
            BindingAction::Sneak => InputKey::Down,
            BindingAction::Sprint => InputKey::Sprint,
        })
        .collect()
}

fn key_label(key: u16) -> String {
    match key {
        0x10 => "Shift".to_owned(),
        0x11 => "Ctrl".to_owned(),
        0x20 => "Space".to_owned(),
        0x09 => "Tab".to_owned(),
        0x0d => "Enter".to_owned(),
        0x25 => "Left Arrow".to_owned(),
        0x26 => "Up Arrow".to_owned(),
        0x27 => "Right Arrow".to_owned(),
        0x28 => "Down Arrow".to_owned(),
        0x30..=0x39 | 0x41..=0x5a => (key as u8 as char).to_string(),
        0x70..=0x87 => format!("F{}", key - 0x6f),
        _ => format!("Key 0x{key:02X}"),
    }
}

fn is_initial_keydown(lparam: Lparam) -> bool {
    lparam as usize & 0x4000_0000 == 0
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
unsafe fn open_title_form(window: Hwnd) {
    if TITLE_FORM.is_some()
        || WORLD_FORM.is_some()
        || INVENTORY_FORM.is_some()
        || PAUSE_FORM.is_some()
        || OPTIONS_FORM.is_some()
    {
        return;
    }
    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let width = 440;
    let height = 380;
    let left = ((client.right - width) / 2).max(12);
    let top = ((client.bottom - height) / 2).max(12);
    let mut controls = Vec::new();
    controls.push(create_form_control(
        window,
        "STATIC",
        "",
        SS_WHITERECT,
        left,
        top,
        width,
        height,
        0,
    ));
    controls.push(create_form_control(
        window,
        "STATIC",
        "Voxel Engine",
        0,
        left + 32,
        top + 24,
        width - 64,
        38,
        0,
    ));
    let world_name = APP
        .as_ref()
        .and_then(|app| app.world_metadata.as_ref())
        .map(|metadata| metadata.name.as_str())
        .unwrap_or("No active world");
    let status_text = format!("Active save: {world_name}. Choose a world or resume play.");
    let status_label = create_form_control(
        window,
        "STATIC",
        &status_text,
        0,
        left + 32,
        top + 70,
        width - 64,
        42,
        0,
    );
    controls.push(status_label);
    let play = create_form_control(
        window,
        "BUTTON",
        "Resume World",
        BS_DEFPUSHBUTTON | WS_TABSTOP,
        left + 32,
        top + 124,
        width - 64,
        38,
        TITLE_PLAY_ID,
    );
    controls.push(play);
    let singleplayer = create_form_control(
        window,
        "BUTTON",
        "Singleplayer / Worlds...",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 32,
        top + 170,
        width - 64,
        38,
        TITLE_SINGLEPLAYER_ID,
    );
    controls.push(singleplayer);
    let options = create_form_control(
        window,
        "BUTTON",
        "Options",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 32,
        top + 216,
        width - 64,
        38,
        TITLE_OPTIONS_ID,
    );
    controls.push(options);
    let quit = create_form_control(
        window,
        "BUTTON",
        "Quit",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 32,
        top + 262,
        width - 64,
        38,
        TITLE_QUIT_ID,
    );
    controls.push(quit);
    controls.push(create_form_control(
        window,
        "STATIC",
        "Multiplayer, resource packs, and accessibility pages are not implemented yet.",
        0,
        left + 32,
        top + 310,
        width - 64,
        48,
        0,
    ));
    if controls.iter().any(|control| control.is_null()) {
        for control in controls
            .into_iter()
            .rev()
            .filter(|control| !control.is_null())
        {
            DestroyWindow(control);
        }
        if let Some(app) = APP.as_ref() {
            app.logger.event(
                "ERROR",
                "title_form_failed",
                "a native screen control failed",
            );
        }
        return;
    }
    TITLE_FORM = Some(TitleForm {
        controls,
        status_label,
    });
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.breaking_held = false;
        app.mining_session.reset();
        app.last_physics = Instant::now();
        if app.cursor_locked {
            app.cursor_locked = false;
            ClipCursor(null());
            ShowCursor(1);
        }
        app.logger
            .event("INFO", "title_opened", "source=startup-or-pause");
    }
    SetFocus(play);
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn destroy_title_form() {
    if let Some(form) = TITLE_FORM.take() {
        for control in form.controls.into_iter().rev() {
            if !control.is_null() {
                DestroyWindow(control);
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn close_title_form(window: Hwnd, enter_game: bool) {
    if TITLE_FORM.is_none() {
        return;
    }
    destroy_title_form();
    if let Some(app) = APP.as_mut() {
        if enter_game && app.vitals.is_dead() {
            app.dialog_open = true;
            app.input.clear();
            app.food_use_held = false;
            app.food_use.reset();
            app.last_physics = Instant::now();
            PostMessageW(window, WM_APP_DEATH, 0, 0);
            return;
        }
        app.dialog_open = false;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
        if enter_game {
            app.cursor_locked = true;
            ShowCursor(0);
            lock_cursor(window);
        }
        app.logger.event(
            "INFO",
            "title_closed",
            if enter_game {
                "action=play"
            } else {
                "action=options"
            },
        );
    }
    if !enter_game {
        ClipCursor(null());
    }
    SetFocus(window);
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn handle_title_command(window: Hwnd, command: usize) -> bool {
    if TITLE_FORM.is_none() {
        return false;
    }
    match command {
        TITLE_PLAY_ID => close_title_form(window, true),
        TITLE_SINGLEPLAYER_ID => {
            let root = GetMenu(window);
            let worlds_menu = GetSubMenu(root, 0);
            if worlds_menu.is_null() {
                if let Some(form) = TITLE_FORM.as_ref() {
                    let text = wide("World management is unavailable.");
                    SetWindowTextW(form.status_label, text.as_ptr());
                }
                return true;
            }
            let mut button_rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if let Some(form) = TITLE_FORM.as_ref() {
                if let Some(button) = form.controls.get(3) {
                    GetWindowRect(*button, &mut button_rect);
                }
            }
            let selected = TrackPopupMenu(
                worlds_menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                button_rect.left,
                button_rect.bottom,
                0,
                window,
                null(),
            );
            if selected != 0 {
                SendMessageW(window, WM_COMMAND, selected as usize, 0);
            }
        }
        TITLE_OPTIONS_ID => {
            close_title_form(window, false);
            open_options_form(window, true);
        }
        TITLE_QUIT_ID => {
            PostMessageW(window, WM_CLOSE, 0, 0);
        }
        _ => return false,
    }
    true
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
    let style =
        if class_name == "BUTTON" && matches!(style & 0x0f, BS_PUSHBUTTON | BS_DEFPUSHBUTTON) {
            style | BS_OWNERDRAW
        } else if class_name == "STATIC" && style == SS_WHITERECT {
            // SS_WHITERECT forces the system white brush and bypasses
            // WM_CTLCOLORSTATIC. A plain static surface uses our panel brush.
            0
        } else {
            style
        };
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

fn inventory_slot(index: usize) -> Option<InventorySlot> {
    match index {
        0..=35 => Some(InventorySlot::Main(index)),
        36 => Some(InventorySlot::Armor(ArmorSlot::Head)),
        37 => Some(InventorySlot::Armor(ArmorSlot::Chest)),
        38 => Some(InventorySlot::Armor(ArmorSlot::Legs)),
        39 => Some(InventorySlot::Armor(ArmorSlot::Feet)),
        40 => Some(InventorySlot::Offhand),
        _ => None,
    }
}

fn inventory_item_name(item: ItemId) -> String {
    match item.0 {
        1 => "Soil".to_owned(),
        2 => "Surface".to_owned(),
        3 => "Rock".to_owned(),
        4 => "Slab".to_owned(),
        5 => "Carpet".to_owned(),
        6 => "Snow Layer".to_owned(),
        7 => "Farmland".to_owned(),
        8 => "Path".to_owned(),
        9 => "Fence".to_owned(),
        10 => "Pane".to_owned(),
        11 => "Prototype Ration".to_owned(),
        id => format!("Unknown Item {id}"),
    }
}

fn matching_creative_items(query: &str) -> Vec<u16> {
    let query = query.trim().to_lowercase();
    (1..=INVENTORY_CATALOG_ITEM_COUNT as u16)
        .filter(|id| {
            query.is_empty()
                || inventory_item_name(ItemId(*id))
                    .to_lowercase()
                    .contains(&query)
        })
        .collect()
}

fn inventory_slot_label(app: &AppState, index: usize) -> String {
    let Some(slot) = inventory_slot(index) else {
        return "Invalid slot".to_owned();
    };
    let prefix = match index {
        0..=8 => format!(
            "{}{}",
            if app.inventory.selected_hotbar() as usize == index {
                ">"
            } else {
                ""
            },
            index + 1
        ),
        9..=35 => format!("{}", index - 8),
        36 => "Head".to_owned(),
        37 => "Chest".to_owned(),
        38 => "Legs".to_owned(),
        39 => "Feet".to_owned(),
        _ => "Offhand".to_owned(),
    };
    match app.inventory.stack(slot).ok().flatten() {
        Some(stack) => format!(
            "{prefix}: {} x{}",
            inventory_item_name(stack.key.id),
            stack.count
        ),
        None => format!("{prefix}: empty"),
    }
}

#[allow(static_mut_refs)]
unsafe fn refresh_inventory_form() {
    let Some((slots, cursor_label, status_label, cursor_text, catalog_search, catalog_buttons)) =
        (|| {
            let app = APP.as_ref()?;
            let form = INVENTORY_FORM.as_ref()?;
            let cursor_text = match app.inventory_menu.cursor() {
                Some(stack) => format!(
                    "Cursor: {} x{}",
                    inventory_item_name(stack.key.id),
                    stack.count
                ),
                None => "Cursor: empty".to_owned(),
            };
            Some((
                form.slot_controls.clone(),
                form.cursor_label,
                form.status_label,
                cursor_text,
                form.catalog_search,
                form.catalog_buttons.clone(),
            ))
        })()
    else {
        return;
    };
    if let Some(app) = APP.as_ref() {
        for (index, control) in slots.into_iter().enumerate() {
            let text = wide(&inventory_slot_label(app, index));
            SetWindowTextW(control, text.as_ptr());
        }
        let text = wide(&cursor_text);
        SetWindowTextW(cursor_label, text.as_ptr());
        let visible = matching_creative_items(&form_control_text(catalog_search));
        let catalog_enabled = app.player.mode == GameMode::Creative;
        if let Some(form) = INVENTORY_FORM.as_mut() {
            form.catalog_visible = visible
                .iter()
                .take(catalog_buttons.len())
                .copied()
                .collect();
        }
        for (index, button) in catalog_buttons.into_iter().enumerate() {
            let id = visible.get(index).copied();
            let label = if catalog_enabled {
                id.map(|id| inventory_item_name(ItemId(id)))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let label = wide(&label);
            SetWindowTextW(button, label.as_ptr());
            EnableWindow(button, (catalog_enabled && id.is_some()) as i32);
        }
    }
    let _ = status_label;
}

#[allow(static_mut_refs)]
unsafe fn drop_inventory_cursor() {
    let result = {
        let Some(app) = APP.as_mut() else {
            return;
        };
        let player = app.player.transform;
        let position = [
            player.x + app.yaw.cos() as f64 * 0.8,
            player.y + PLAYER_EYE_OFFSET - 0.2,
            player.z + app.yaw.sin() as f64 * 0.8,
        ];
        app.inventory_menu.drop_cursor(
            &mut app.drops,
            position,
            ITEM_PICKUP_DELAY_TICKS,
            &app.item_catalog,
        )
    };
    match result {
        Ok(Some(id)) => {
            if let Some(scene) = SCENE.as_mut() {
                scene.dirty = true;
                if let Some(app) = APP.as_ref() {
                    scene.drop_meshes = build_drop_meshes(&app.drops);
                }
                if let Some(app) = APP.as_mut() {
                    if let Some(gpu) = app.gpu.as_mut() {
                        upload_scene_meshes(gpu, scene);
                    }
                    app.logger
                        .event("INFO", "inventory_item_dropped", format!("id={}", id.0));
                }
            }
            if let Some(form) = INVENTORY_FORM.as_ref() {
                let empty = wide("");
                SetWindowTextW(form.status_label, empty.as_ptr());
            }
        }
        Ok(None) => set_inventory_status("Cursor stack is empty."),
        Err(error) => set_inventory_status(&format!("Could not drop cursor stack: {error}")),
    }
    refresh_inventory_form();
}

#[allow(static_mut_refs)]
unsafe fn drop_selected_hotbar_items(drop_entire_stack: bool) {
    let result = {
        let Some(app) = APP.as_mut() else {
            return;
        };
        let player = app.player.transform;
        let position = [
            player.x + app.yaw.cos() as f64 * 0.8,
            player.y + PLAYER_EYE_OFFSET - 0.2,
            player.z + app.yaw.sin() as f64 * 0.8,
        ];
        let slot = InventorySlot::Main(usize::from(app.inventory.selected_hotbar()));
        let count = app
            .inventory
            .selected_stack()
            .map(|stack| if drop_entire_stack { stack.count } else { 1 })
            .unwrap_or(1);
        app.inventory.drop_from_slot(
            slot,
            count,
            &mut app.drops,
            position,
            ITEM_PICKUP_DELAY_TICKS,
            &app.item_catalog,
        )
    };
    match result {
        Ok(Some(id)) => {
            if let (Some(app), Some(scene)) = (APP.as_mut(), SCENE.as_mut()) {
                scene.dirty = true;
                scene.drop_meshes = build_drop_meshes(&app.drops);
                if let Some(gpu) = app.gpu.as_mut() {
                    upload_scene_meshes(gpu, scene);
                }
                app.logger.event(
                    "INFO",
                    "hotbar_item_dropped",
                    format!("id={} all={drop_entire_stack}", id.0),
                );
            }
        }
        Ok(None) => {}
        Err(error) => {
            if let Some(app) = APP.as_mut() {
                app.logger
                    .event("WARN", "hotbar_drop_rejected", error.to_string());
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn swap_hotbar_item_with_offhand() {
    let result = {
        let Some(app) = APP.as_mut() else {
            return;
        };
        let selected = InventorySlot::Main(usize::from(app.inventory.selected_hotbar()));
        app.inventory
            .swap(selected, InventorySlot::Offhand, &app.item_catalog)
    };
    match result {
        Ok(()) => {
            if let (Some(app), Some(scene)) = (APP.as_mut(), SCENE.as_mut()) {
                scene.dirty = true;
                if let Some(block) = app
                    .inventory
                    .selected_stack()
                    .and_then(|stack| prototype_block_for_item(stack.key))
                {
                    app.selected_block = block;
                }
                app.logger.event("INFO", "hotbar_offhand_swapped", "");
            }
        }
        Err(error) => {
            if let Some(app) = APP.as_mut() {
                app.logger
                    .event("WARN", "hotbar_offhand_swap_rejected", error.to_string());
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn set_inventory_status(message: &str) {
    if let Some(form) = INVENTORY_FORM.as_ref() {
        let text = wide(message);
        SetWindowTextW(form.status_label, text.as_ptr());
    }
}

#[allow(static_mut_refs)]
unsafe fn open_pause_form(window: Hwnd) {
    let cursor_was_locked = APP.as_ref().is_some_and(|app| app.cursor_locked);
    open_pause_form_with_capture(window, cursor_was_locked, cursor_was_locked);
}

#[allow(static_mut_refs)]
unsafe fn open_pause_form_with_capture(
    window: Hwnd,
    cursor_was_locked: bool,
    make_cursor_visible: bool,
) {
    if PAUSE_FORM.is_some() || INVENTORY_FORM.is_some() || WORLD_FORM.is_some() {
        return;
    }
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.cursor_locked = false;
        app.breaking_held = false;
        app.mining_session.reset();
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
        app.logger.event("INFO", "pause_opened", "source=escape");
    } else {
        return;
    }
    ClipCursor(null());
    if make_cursor_visible {
        ShowCursor(1);
    }

    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let width = 360;
    let height = 300;
    let left = ((client.right - width) / 2).max(12);
    let top = ((client.bottom - height) / 2).max(12);
    let mut controls = Vec::new();
    controls.push(create_form_control(
        window,
        "STATIC",
        "",
        SS_WHITERECT,
        left,
        top,
        width,
        height,
        0,
    ));
    controls.push(create_form_control(
        window,
        "STATIC",
        "Paused",
        0,
        left + 24,
        top + 18,
        width - 48,
        28,
        0,
    ));
    controls.push(create_form_control(
        window,
        "STATIC",
        "Video field of view and mouse controls are available in Options.",
        0,
        left + 24,
        top + 50,
        width - 48,
        34,
        0,
    ));
    let resume_button = create_form_control(
        window,
        "BUTTON",
        "Resume",
        BS_DEFPUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 86,
        width - 48,
        34,
        PAUSE_RESUME_ID,
    );
    controls.push(resume_button);
    let options_button = create_form_control(
        window,
        "BUTTON",
        "Options",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 126,
        width - 48,
        34,
        PAUSE_OPTIONS_ID,
    );
    controls.push(options_button);
    controls.push(create_form_control(
        window,
        "BUTTON",
        "Save & Quit",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 166,
        width - 48,
        34,
        PAUSE_SAVE_QUIT_ID,
    ));
    controls.push(create_form_control(
        window,
        "BUTTON",
        "Save & Return to Title",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 206,
        width - 48,
        34,
        PAUSE_TITLE_ID,
    ));
    let status_label = create_form_control(
        window,
        "STATIC",
        "",
        0,
        left + 24,
        top + 246,
        width - 48,
        32,
        0,
    );
    controls.push(status_label);
    PAUSE_FORM = Some(PauseForm {
        controls,
        status_label,
        cursor_was_locked,
    });
    if !resume_button.is_null() {
        SetFocus(resume_button);
    }
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn handle_pause_command(window: Hwnd, command: usize) -> bool {
    if PAUSE_FORM.is_none() {
        return false;
    }
    match command {
        PAUSE_RESUME_ID | DIALOG_IDCANCEL => close_pause_form(window),
        PAUSE_OPTIONS_ID => open_options_form(window, false),
        PAUSE_SAVE_QUIT_ID => save_and_quit_from_pause(window),
        PAUSE_TITLE_ID => save_and_return_to_title(window),
        _ => return true,
    }
    true
}

#[allow(static_mut_refs)]
unsafe fn save_and_return_to_title(window: Hwnd) {
    let result = match (APP.as_mut(), SCENE.as_mut()) {
        (Some(app), Some(scene)) if scene.dirty => persist_world(app, scene),
        _ => Ok(()),
    };
    if let Err(error) = result {
        if let Some(form) = PAUSE_FORM.as_ref() {
            let text = wide(&format!("Could not save world: {error}"));
            SetWindowTextW(form.status_label, text.as_ptr());
        }
        if let Some(app) = APP.as_ref() {
            app.logger
                .event("ERROR", "pause_return_title_failed", error.to_string());
        }
        return;
    }
    destroy_pause_form();
    if let Some(app) = APP.as_mut() {
        app.dialog_open = false;
        app.cursor_locked = false;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
        app.logger
            .event("INFO", "pause_return_title", "world_saved=true");
    }
    ClipCursor(null());
    open_title_form(window);
}

#[allow(static_mut_refs)]
unsafe fn save_and_quit_from_pause(window: Hwnd) {
    let result = match (APP.as_mut(), SCENE.as_mut()) {
        (Some(app), Some(scene)) if scene.dirty => persist_world(app, scene),
        _ => Ok(()),
    };
    if let Err(error) = result {
        if let Some(form) = PAUSE_FORM.as_ref() {
            let text = wide(&format!("Could not save world: {error}"));
            SetWindowTextW(form.status_label, text.as_ptr());
        }
        if let Some(app) = APP.as_ref() {
            app.logger
                .event("ERROR", "pause_save_quit_failed", error.to_string());
        }
        return;
    }

    let was_cursor_locked = PAUSE_FORM
        .as_ref()
        .is_some_and(|form| form.cursor_was_locked);
    destroy_pause_form();
    if let Some(app) = APP.as_mut() {
        app.dialog_open = false;
        app.cursor_locked = false;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.logger
            .event("INFO", "pause_save_quit", "world_saved=true");
    }
    ClipCursor(null());
    if was_cursor_locked {
        ShowCursor(1);
    }
    PostMessageW(window, WM_CLOSE, 0, 0);
}

#[allow(static_mut_refs)]
unsafe fn close_pause_form(window: Hwnd) {
    let restore_cursor = PAUSE_FORM
        .as_ref()
        .is_some_and(|form| form.cursor_was_locked);
    destroy_pause_form();
    if let Some(app) = APP.as_mut() {
        app.dialog_open = false;
        app.cursor_locked = false;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
        app.logger.event("INFO", "pause_closed", "action=resume");
        if restore_cursor {
            app.cursor_locked = true;
        }
    }
    ClipCursor(null());
    if restore_cursor {
        ShowCursor(0);
        lock_cursor(window);
    }
    SetFocus(window);
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn destroy_pause_form() {
    if let Some(form) = PAUSE_FORM.take() {
        for control in form.controls.into_iter().rev() {
            if !control.is_null() {
                DestroyWindow(control);
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn open_death_form(window: Hwnd) {
    if DEATH_FORM.is_some() || !APP.as_ref().is_some_and(|app| app.vitals.is_dead()) {
        return;
    }
    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let width = 380;
    let height = 210;
    let left = ((client.right - width) / 2).max(12);
    let top = ((client.bottom - height) / 2).max(12);
    let mut controls = Vec::new();
    controls.push(create_form_control(
        window,
        "STATIC",
        "",
        SS_WHITERECT,
        left,
        top,
        width,
        height,
        0,
    ));
    controls.push(create_form_control(
        window,
        "STATIC",
        "You died",
        0,
        left + 24,
        top + 16,
        width - 48,
        32,
        0,
    ));
    let status_text = if APP.as_ref().is_some_and(|app| {
        app.world_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.keep_inventory)
    }) {
        "Keep inventory is enabled. Your items remain with you."
    } else if APP.as_ref().is_some_and(|app| app.death_items_retained) {
        "The world could not accept all drops. Your inventory is retained."
    } else {
        "Your inventory drops into the world. Respawn to continue."
    };
    let status_label = create_form_control(
        window,
        "STATIC",
        status_text,
        0,
        left + 24,
        top + 50,
        width - 48,
        34,
        0,
    );
    controls.push(status_label);
    let respawn = create_form_control(
        window,
        "BUTTON",
        "Respawn",
        BS_DEFPUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 96,
        width - 48,
        34,
        DEATH_RESPAWN_ID,
    );
    controls.push(respawn);
    controls.push(create_form_control(
        window,
        "BUTTON",
        "Save & Return to Title",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 138,
        width - 48,
        34,
        DEATH_TITLE_ID,
    ));
    if controls.iter().any(|control| control.is_null()) {
        for control in controls
            .into_iter()
            .rev()
            .filter(|control| !control.is_null())
        {
            DestroyWindow(control);
        }
        return;
    }
    DEATH_FORM = Some(DeathForm {
        controls,
        status_label,
    });
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.death_items_retained = false;
        app.breaking_held = false;
        app.mining_session.reset();
        app.cursor_locked = false;
        app.last_physics = Instant::now();
    }
    ClipCursor(null());
    ShowCursor(1);
    if !respawn.is_null() {
        SetFocus(respawn);
    }
}

#[allow(static_mut_refs)]
unsafe fn handle_death_command(window: Hwnd, command: usize) -> bool {
    if DEATH_FORM.is_none() {
        return false;
    }
    match command {
        DEATH_RESPAWN_ID => respawn_player(window),
        DEATH_TITLE_ID => {
            let result = match (APP.as_mut(), SCENE.as_mut()) {
                (Some(app), Some(scene)) if scene.dirty => persist_world(app, scene),
                _ => Ok(()),
            };
            if let Err(error) = result {
                if let Some(form) = DEATH_FORM.as_ref() {
                    let text = wide(&format!("Could not save world: {error}"));
                    SetWindowTextW(form.status_label, text.as_ptr());
                }
                return true;
            }
            destroy_death_form();
            if let Some(app) = APP.as_mut() {
                app.dialog_open = false;
                app.last_physics = Instant::now();
            }
            open_title_form(window);
        }
        DIALOG_IDCANCEL => {}
        _ => {}
    }
    true
}

#[allow(static_mut_refs)]
unsafe fn respawn_player(window: Hwnd) {
    let previous_player = APP.as_ref().map(|app| (app.player, app.vitals, app.food));
    let previous_scene_dirty = SCENE.as_ref().is_some_and(|scene| scene.dirty);
    let result = (|| {
        let (Some(app), Some(scene)) = (APP.as_mut(), SCENE.as_mut()) else {
            return Err("world is unavailable".to_string());
        };
        let x = app.player.transform.x.floor() as i32;
        let z = app.player.transform.z.floor() as i32;
        let mut y = 0;
        for candidate in 0..128 {
            if BlockRegistry::new().is_solid(
                scene
                    .world
                    .block_at([x, candidate, z])
                    .unwrap_or(crate::world::BlockId(0)),
            ) {
                y = candidate + 1;
            }
        }
        app.player.transform = crate::entity::Transform {
            x: x as f64 + 0.5,
            y: y as f64 + 1.0 + crate::grid::PLAYER_HALF_HEIGHT,
            z: z as f64 + 0.5,
        };
        app.player.previous_transform = app.player.transform;
        app.player.velocity = [0.0; 3];
        app.player.grounded = false;
        app.player.fall_distance = 0.0;
        app.player.fall_damage_armed = false;
        app.player.flying = false;
        app.vitals.respawn();
        app.hunger_clock = crate::survival::HungerClock::default();
        scene.dirty = true;
        persist_world(app, scene).map_err(|error| error.to_string())
    })();
    if let Err(error) = result {
        if let (Some((player, vitals, food)), Some(app)) = (previous_player, APP.as_mut()) {
            app.player = player;
            app.vitals = vitals;
            app.food = food;
        }
        if let Some(scene) = SCENE.as_mut() {
            scene.dirty = previous_scene_dirty;
        }
        if let Some(form) = DEATH_FORM.as_ref() {
            let text = wide(&format!("Respawn save failed: {error}"));
            SetWindowTextW(form.status_label, text.as_ptr());
        }
        return;
    }
    destroy_death_form();
    if let Some(app) = APP.as_mut() {
        app.dialog_open = false;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
        app.cursor_locked = true;
    }
    ShowCursor(0);
    lock_cursor(window);
}

#[allow(static_mut_refs)]
unsafe fn destroy_death_form() {
    if let Some(form) = DEATH_FORM.take() {
        for control in form.controls.into_iter().rev() {
            if !control.is_null() {
                DestroyWindow(control);
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn open_options_form(window: Hwnd, return_to_title: bool) {
    let return_to_capture = !return_to_title
        && PAUSE_FORM
            .as_ref()
            .is_some_and(|form| form.cursor_was_locked);
    if (PAUSE_FORM.is_none() && !return_to_title) || OPTIONS_FORM.is_some() || TITLE_FORM.is_some()
    {
        return;
    }
    if PAUSE_FORM.is_some() {
        destroy_pause_form();
    }

    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let width = 520;
    let height = 580;
    let left = ((client.right - width) / 2).max(12);
    let top = ((client.bottom - height) / 2).max(12);
    let current_settings = APP.as_ref().map(|app| app.settings).unwrap_or_default();
    let mut controls = Vec::new();
    controls.push(create_form_control(
        window,
        "STATIC",
        "",
        SS_WHITERECT,
        left,
        top,
        width,
        height,
        0,
    ));
    controls.push(create_form_control(
        window,
        "STATIC",
        "Options — Video and Controls",
        0,
        left + 24,
        top + 16,
        width - 48,
        28,
        0,
    ));
    controls.push(create_form_control(
        window,
        "STATIC",
        "Field of view (30–110 degrees)",
        0,
        left + 24,
        top + 54,
        width - 48,
        22,
        0,
    ));
    let field_of_view_edit = create_form_control(
        window,
        "EDIT",
        &APP.as_ref()
            .map(|app| app.settings.field_of_view_degrees.to_string())
            .unwrap_or_else(|| Settings::default().field_of_view_degrees.to_string()),
        ES_AUTOHSCROLL | WS_TABSTOP,
        left + 24,
        top + 78,
        width - 48,
        28,
        OPTIONS_FOV_ID,
    );
    SendMessageW(field_of_view_edit, EM_SETLIMITTEXT, 5, 0);
    controls.push(field_of_view_edit);
    controls.push(create_form_control(
        window,
        "STATIC",
        "Mouse sensitivity (100–2000; 1000 is current default)",
        0,
        left + 24,
        top + 116,
        width - 48,
        22,
        0,
    ));
    let sensitivity_edit = create_form_control(
        window,
        "EDIT",
        &APP.as_ref()
            .map(|app| app.settings.mouse_sensitivity_permille.to_string())
            .unwrap_or_else(|| Settings::default().mouse_sensitivity_permille.to_string()),
        ES_AUTOHSCROLL | WS_TABSTOP,
        left + 24,
        top + 140,
        width - 48,
        28,
        OPTIONS_SENSITIVITY_ID,
    );
    SendMessageW(sensitivity_edit, EM_SETLIMITTEXT, 5, 0);
    controls.push(sensitivity_edit);
    let invert_mouse_y_check = create_form_control(
        window,
        "BUTTON",
        "Invert mouse Y",
        BS_AUTOCHECKBOX | WS_TABSTOP,
        left + 24,
        top + 178,
        width - 48,
        28,
        OPTIONS_INVERT_ID,
    );
    if APP.as_ref().is_some_and(|app| app.settings.invert_mouse_y) {
        SendMessageW(invert_mouse_y_check, BM_SETCHECK, BST_CHECKED as usize, 0);
    }
    controls.push(invert_mouse_y_check);
    let sprint_toggle_check = create_form_control(
        window,
        "BUTTON",
        "Toggle sprint (off = hold)",
        BS_AUTOCHECKBOX | WS_TABSTOP,
        left + 24,
        top + 204,
        width - 48,
        28,
        OPTIONS_SPRINT_TOGGLE_ID,
    );
    if current_settings.sprint_behavior == HoldToggle::Toggle {
        SendMessageW(sprint_toggle_check, BM_SETCHECK, BST_CHECKED as usize, 0);
    }
    controls.push(sprint_toggle_check);
    let sneak_toggle_check = create_form_control(
        window,
        "BUTTON",
        "Toggle sneak (off = hold)",
        BS_AUTOCHECKBOX | WS_TABSTOP,
        left + 24,
        top + 234,
        width - 48,
        28,
        OPTIONS_SNEAK_TOGGLE_ID,
    );
    if current_settings.sneak_behavior == HoldToggle::Toggle {
        SendMessageW(sneak_toggle_check, BM_SETCHECK, BST_CHECKED as usize, 0);
    }
    controls.push(sneak_toggle_check);
    controls.push(create_form_control(
        window,
        "STATIC",
        "Click a movement action, then press a key. Escape cancels binding.",
        0,
        left + 24,
        top + 264,
        width - 48,
        22,
        0,
    ));
    let mut binding_buttons = Vec::with_capacity(BindingAction::ALL.len());
    for (index, action) in BindingAction::ALL.into_iter().enumerate() {
        let row_y = top + 286 + index as i32 * 34;
        controls.push(create_form_control(
            window,
            "STATIC",
            action.label(),
            0,
            left + 24,
            row_y + 4,
            240,
            24,
            0,
        ));
        let button = create_form_control(
            window,
            "BUTTON",
            &key_label(current_settings.key_bindings.key(action)),
            BS_PUSHBUTTON | WS_TABSTOP,
            left + 280,
            row_y,
            190,
            28,
            OPTIONS_BINDING_ID_BASE + index,
        );
        binding_buttons.push((action, button));
        controls.push(button);
    }
    let status_label = create_form_control(
        window,
        "STATIC",
        "Project settings are not yet verified against Java Edition 26.3.",
        0,
        left + 24,
        top + 520,
        width - 48,
        24,
        0,
    );
    controls.push(status_label);
    controls.push(create_form_control(
        window,
        "BUTTON",
        "Use Defaults",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 24,
        top + 546,
        120,
        34,
        OPTIONS_DEFAULTS_ID,
    ));
    controls.push(create_form_control(
        window,
        "BUTTON",
        "Apply",
        BS_DEFPUSHBUTTON | WS_TABSTOP,
        left + 160,
        top + 546,
        120,
        34,
        OPTIONS_APPLY_ID,
    ));
    controls.push(create_form_control(
        window,
        "BUTTON",
        "Back",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 296,
        top + 546,
        120,
        34,
        OPTIONS_BACK_ID,
    ));
    OPTIONS_FORM = Some(OptionsForm {
        controls,
        field_of_view_edit,
        sensitivity_edit,
        invert_mouse_y_check,
        sprint_toggle_check,
        sneak_toggle_check,
        status_label,
        binding_buttons,
        key_bindings: current_settings.key_bindings,
        capture_action: None,
        escape_canceled_down: false,
        return_to_capture,
        return_to_title,
    });
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
    }
    if !field_of_view_edit.is_null() {
        SetFocus(field_of_view_edit);
    }
}

#[allow(static_mut_refs)]
unsafe fn handle_options_command(window: Hwnd, command: usize) -> bool {
    if OPTIONS_FORM.is_none() {
        return false;
    }
    match command {
        OPTIONS_BACK_ID | DIALOG_IDCANCEL => close_options_form(window),
        OPTIONS_DEFAULTS_ID => {
            if let Some(form) = OPTIONS_FORM.as_ref() {
                let defaults = Settings::default();
                let fov = wide(&defaults.field_of_view_degrees.to_string());
                let sensitivity = wide(&defaults.mouse_sensitivity_permille.to_string());
                SetWindowTextW(form.field_of_view_edit, fov.as_ptr());
                SetWindowTextW(form.sensitivity_edit, sensitivity.as_ptr());
                SendMessageW(form.invert_mouse_y_check, BM_SETCHECK, 0, 0);
                SendMessageW(form.sprint_toggle_check, BM_SETCHECK, 0, 0);
                SendMessageW(form.sneak_toggle_check, BM_SETCHECK, 0, 0);
                form.binding_buttons
                    .iter()
                    .zip(BindingAction::ALL)
                    .for_each(|((_, button), action)| {
                        let text = wide(&key_label(defaults.key_bindings.key(action)));
                        SetWindowTextW(*button, text.as_ptr());
                    });
                if let Some(form) = OPTIONS_FORM.as_mut() {
                    form.key_bindings = defaults.key_bindings;
                    form.capture_action = None;
                    form.escape_canceled_down = false;
                }
                set_options_status("Defaults staged. Select Apply to save them.");
            }
        }
        OPTIONS_APPLY_ID => apply_options_form(window),
        binding_id
            if (OPTIONS_BINDING_ID_BASE..OPTIONS_BINDING_ID_BASE + BindingAction::ALL.len())
                .contains(&binding_id) =>
        {
            let index = binding_id - OPTIONS_BINDING_ID_BASE;
            let action = BindingAction::ALL[index];
            if let Some(form) = OPTIONS_FORM.as_mut() {
                form.capture_action = Some(action);
                if let Some((_, button)) = form
                    .binding_buttons
                    .iter()
                    .find(|(candidate, _)| *candidate == action)
                {
                    let text = wide("Press a key (Escape cancels)");
                    SetWindowTextW(*button, text.as_ptr());
                }
            }
            SetFocus(window);
            set_options_status(&format!(
                "Press a key for {}. Escape cancels.",
                action.label()
            ));
        }
        _ => return true,
    }
    true
}

#[allow(static_mut_refs)]
unsafe fn apply_options_form(window: Hwnd) {
    let Some(form) = OPTIONS_FORM.as_ref() else {
        return;
    };
    let field_of_view_degrees = form_control_text(form.field_of_view_edit)
        .trim()
        .parse::<u16>();
    let mouse_sensitivity_permille = form_control_text(form.sensitivity_edit)
        .trim()
        .parse::<u16>();
    let (Ok(field_of_view_degrees), Ok(mouse_sensitivity_permille)) =
        (field_of_view_degrees, mouse_sensitivity_permille)
    else {
        set_options_status("Enter whole numbers for both settings.");
        return;
    };
    let settings = Settings {
        field_of_view_degrees,
        mouse_sensitivity_permille,
        invert_mouse_y: SendMessageW(form.invert_mouse_y_check, BM_GETCHECK, 0, 0) == BST_CHECKED,
        key_bindings: form.key_bindings,
        sprint_behavior: if SendMessageW(form.sprint_toggle_check, BM_GETCHECK, 0, 0) == BST_CHECKED
        {
            HoldToggle::Toggle
        } else {
            HoldToggle::Hold
        },
        sneak_behavior: if SendMessageW(form.sneak_toggle_check, BM_GETCHECK, 0, 0) == BST_CHECKED {
            HoldToggle::Toggle
        } else {
            HoldToggle::Hold
        },
    };
    if let Err(error) = settings.validate() {
        let message = match error {
            crate::settings::SettingsValidationError::FieldOfViewOutOfRange => {
                "Field of view must be between 30 and 110 degrees."
            }
            crate::settings::SettingsValidationError::MouseSensitivityOutOfRange => {
                "Mouse sensitivity must be between 100 and 2000."
            }
            crate::settings::SettingsValidationError::InvalidKeyBindings => {
                "One or more key bindings are invalid. Restore defaults or rebind them."
            }
        };
        set_options_status(message);
        return;
    }
    let Some(store) = APP.as_ref().and_then(|app| app.settings_store.clone()) else {
        set_options_status("Settings location is unavailable; changes were not applied.");
        return;
    };
    if let Err(error) = store.save(settings) {
        set_options_status(&format!("Could not save settings: {error}"));
        if let Some(app) = APP.as_ref() {
            app.logger
                .event("ERROR", "settings_save_failed", error.to_string());
        }
        return;
    }
    if let Some(app) = APP.as_mut() {
        app.settings = settings;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.sprint_toggled = false;
        app.sneak_toggled = false;
        app.jump_was_pressed = false;
        app.camera.fov_degrees = settings.field_of_view_degrees as f32;
        app.logger.event("INFO", "settings_saved", "scope=local");
    }
    close_options_form(window);
}

#[allow(static_mut_refs)]
unsafe fn set_options_status(message: &str) {
    if let Some(form) = OPTIONS_FORM.as_ref() {
        let text = wide(message);
        SetWindowTextW(form.status_label, text.as_ptr());
    }
}

#[allow(static_mut_refs)]
unsafe fn cancel_options_binding_capture(window: Hwnd) {
    let Some(form) = OPTIONS_FORM.as_mut() else {
        return;
    };
    if let Some(action) = form.capture_action.take() {
        if let Some((_, button)) = form
            .binding_buttons
            .iter()
            .find(|(candidate, _)| *candidate == action)
        {
            let text = wide(&key_label(form.key_bindings.key(action)));
            SetWindowTextW(*button, text.as_ptr());
        }
    }
    SetFocus(window);
    set_options_status("Key binding canceled.");
}

#[allow(static_mut_refs)]
unsafe fn capture_options_binding(window: Hwnd, action: BindingAction, key: u16) {
    let Some(mut bindings) = OPTIONS_FORM.as_ref().map(|form| form.key_bindings) else {
        return;
    };
    let conflicts = bindings.conflicts(action, key);
    let allow_shared = if conflicts.is_empty() {
        false
    } else {
        let conflict_names = conflicts
            .iter()
            .map(|conflict| conflict.label())
            .collect::<Vec<_>>()
            .join(", ");
        let prompt = wide(&format!(
            "This key is already assigned to {conflict_names}. Assign it to both actions?"
        ));
        let caption = wide("Shared Key Binding");
        MessageBoxW(
            window,
            prompt.as_ptr(),
            caption.as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    };

    let result = bindings.assign(action, key, allow_shared);
    if let Some(form) = OPTIONS_FORM.as_mut() {
        form.capture_action = None;
        form.escape_canceled_down = true;
        if result.is_ok() {
            form.key_bindings = bindings;
        }
        if let Some((_, button)) = form
            .binding_buttons
            .iter()
            .find(|(candidate, _)| *candidate == action)
        {
            let displayed_key = if result.is_ok() {
                key
            } else {
                form.key_bindings.key(action)
            };
            let text = wide(&key_label(displayed_key));
            SetWindowTextW(*button, text.as_ptr());
        }
    }
    SetFocus(window);
    match result {
        Ok(()) => {
            refresh_options_binding_buttons();
            if allow_shared {
                set_options_status("Shared binding staged. Select Apply to save it.");
            } else {
                set_options_status("Binding staged. Select Apply to save it.");
            }
        }
        Err(KeyBindingError::Conflict) => {
            set_options_status(
                "Binding unchanged; use the same key only after confirming the conflict.",
            );
        }
        Err(KeyBindingError::ReservedOrInvalid) => {
            set_options_status(
                "That key is reserved by inventory, hotbar, pause, or is unsupported.",
            );
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn refresh_options_binding_buttons() {
    if let Some(form) = OPTIONS_FORM.as_ref() {
        for (action, button) in &form.binding_buttons {
            let text = wide(&key_label(form.key_bindings.key(*action)));
            SetWindowTextW(*button, text.as_ptr());
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn close_options_form(window: Hwnd) {
    let return_to_title = OPTIONS_FORM
        .as_ref()
        .is_some_and(|form| form.return_to_title);
    let return_to_capture = OPTIONS_FORM
        .as_ref()
        .is_some_and(|form| form.return_to_capture);
    destroy_options_form();
    if return_to_title {
        open_title_form(window);
    } else {
        open_pause_form_with_capture(window, return_to_capture, false);
    }
}

#[allow(static_mut_refs)]
unsafe fn destroy_options_form() {
    if let Some(form) = OPTIONS_FORM.take() {
        for control in form.controls.into_iter().rev() {
            if !control.is_null() {
                DestroyWindow(control);
            }
        }
    }
}

#[allow(static_mut_refs)]
unsafe fn open_inventory_form(window: Hwnd) {
    if INVENTORY_FORM.is_some() || WORLD_FORM.is_some() {
        return;
    }
    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    GetClientRect(window, &mut client);
    let width = 1080;
    let height = 380;
    let left = ((client.right - width) / 2).max(12);
    let top = ((client.bottom - height) / 2).max(12);
    let mut controls = Vec::new();
    let panel = create_form_control(
        window,
        "STATIC",
        "",
        SS_WHITERECT,
        left,
        top,
        width,
        height,
        0,
    );
    controls.push(panel);
    let title = create_form_control(
        window,
        "STATIC",
        "Inventory - left click selects; choose Secondary Click to split/place one",
        0,
        left + 20,
        top + 14,
        700,
        24,
        0,
    );
    controls.push(title);
    let main_label = create_form_control(
        window,
        "STATIC",
        "Inventory",
        0,
        left + 20,
        top + 42,
        520,
        22,
        0,
    );
    controls.push(main_label);
    let mut slot_controls = vec![null_mut(); INVENTORY_SLOT_COUNT];
    for index in 9..36 {
        let offset = index - 9;
        let row = offset / 9;
        let column = offset % 9;
        let control = create_form_control(
            window,
            "BUTTON",
            "",
            BS_PUSHBUTTON | WS_TABSTOP,
            left + 20 + column as i32 * 60,
            top + 66 + row as i32 * 48,
            56,
            42,
            INVENTORY_SLOT_ID_BASE + index,
        );
        slot_controls[index] = control;
        controls.push(control);
    }
    let hotbar_label = create_form_control(
        window,
        "STATIC",
        "Hotbar",
        0,
        left + 20,
        top + 218,
        520,
        20,
        0,
    );
    controls.push(hotbar_label);
    for index in 0..9 {
        let control = create_form_control(
            window,
            "BUTTON",
            "",
            BS_PUSHBUTTON | WS_TABSTOP,
            left + 20 + index as i32 * 60,
            top + 238,
            56,
            42,
            INVENTORY_SLOT_ID_BASE + index,
        );
        slot_controls[index] = control;
        controls.push(control);
    }
    let equipment_label = create_form_control(
        window,
        "STATIC",
        "Equipment",
        0,
        left + 570,
        top + 42,
        155,
        20,
        0,
    );
    controls.push(equipment_label);
    for index in 36..INVENTORY_SLOT_COUNT {
        let control = create_form_control(
            window,
            "BUTTON",
            "",
            BS_PUSHBUTTON | WS_TABSTOP,
            left + 570,
            top + 66 + (index - 36) as i32 * 44,
            155,
            38,
            INVENTORY_SLOT_ID_BASE + index,
        );
        slot_controls[index] = control;
        controls.push(control);
    }
    let catalog_label = create_form_control(
        window,
        "STATIC",
        "Creative Catalog - prototype items",
        0,
        left + 760,
        top + 42,
        300,
        20,
        0,
    );
    controls.push(catalog_label);
    let catalog_search = create_form_control(
        window,
        "EDIT",
        "",
        ES_AUTOHSCROLL | WS_TABSTOP,
        left + 760,
        top + 66,
        270,
        26,
        INVENTORY_CATALOG_SEARCH_ID,
    );
    controls.push(catalog_search);
    SendMessageW(catalog_search, EM_SETLIMITTEXT, 64, 0);
    let mut catalog_buttons = Vec::with_capacity(INVENTORY_CATALOG_ITEM_COUNT);
    for index in 0..INVENTORY_CATALOG_ITEM_COUNT {
        let column = index % 2;
        let row = index / 2;
        let button = create_form_control(
            window,
            "BUTTON",
            "",
            BS_PUSHBUTTON | WS_TABSTOP,
            left + 760 + column as i32 * 138,
            top + 100 + row as i32 * 46,
            132,
            40,
            INVENTORY_CATALOG_ITEM_ID_BASE + index,
        );
        catalog_buttons.push(button);
        controls.push(button);
    }
    let cursor_label = create_form_control(
        window,
        "STATIC",
        "Cursor: empty",
        0,
        left + 20,
        top + 318,
        280,
        22,
        0,
    );
    controls.push(cursor_label);
    let mode_button = create_form_control(
        window,
        "BUTTON",
        "Click Mode: Primary",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 310,
        top + 314,
        140,
        30,
        INVENTORY_MODE_ID,
    );
    controls.push(mode_button);
    let drop_button = create_form_control(
        window,
        "BUTTON",
        "Drop Cursor",
        BS_PUSHBUTTON | WS_TABSTOP,
        left + 460,
        top + 314,
        150,
        30,
        INVENTORY_DROP_ID,
    );
    controls.push(drop_button);
    let status_label =
        create_form_control(window, "STATIC", "", 0, left + 20, top + 346, 520, 20, 0);
    controls.push(status_label);
    let close = create_form_control(
        window,
        "BUTTON",
        "Close (E)",
        BS_DEFPUSHBUTTON | WS_TABSTOP,
        left + 625,
        top + 314,
        100,
        30,
        INVENTORY_CLOSE_ID,
    );
    controls.push(close);
    if controls.iter().any(|control| control.is_null()) {
        for control in controls
            .into_iter()
            .rev()
            .filter(|control| !control.is_null())
        {
            DestroyWindow(control);
        }
        if let Some(app) = APP.as_ref() {
            app.logger.event(
                "ERROR",
                "inventory_form_failed",
                "a native screen control failed",
            );
        }
        return;
    }
    let cursor_was_locked = APP.as_ref().is_some_and(|app| app.cursor_locked);
    INVENTORY_FORM = Some(InventoryForm {
        controls,
        slot_controls,
        catalog_search,
        catalog_buttons,
        catalog_visible: Vec::new(),
        cursor_label,
        status_label,
        mode_button,
        secondary_click: false,
        cursor_was_locked,
    });
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.breaking_held = false;
        app.mining_session.reset();
        app.last_physics = Instant::now();
        if app.cursor_locked {
            app.cursor_locked = false;
            ClipCursor(null());
            ShowCursor(1);
        }
    }
    refresh_inventory_form();
    if let Some(form) = INVENTORY_FORM.as_ref() {
        SetFocus(form.slot_controls[0]);
    }
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
    let (world_id, name, seed_text, mode, difficulty, difficulty_locked, keep_inventory) =
        match world {
            Some(world) => (
                Some(world.metadata.id),
                world.metadata.name,
                world.metadata.seed.to_string(),
                world.metadata.game_mode,
                world.metadata.difficulty,
                world.metadata.difficulty_locked,
                world.metadata.keep_inventory,
            ),
            None => (
                None,
                new_name,
                String::new(),
                StoredGameMode::Creative,
                StoredDifficulty::Normal,
                false,
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
    let top = ((client.bottom - 520) / 2).max(12);
    let panel = create_form_control(window, "STATIC", "", SS_WHITERECT, left, top, 460, 520, 0);
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
    let keep_inventory_check = create_form_control(
        window,
        "BUTTON",
        "Keep inventory on death",
        BS_AUTOCHECKBOX | WS_TABSTOP,
        left + 24,
        top + 322,
        412,
        24,
        FORM_WORLD_KEEP_INVENTORY,
    );
    controls.push(keep_inventory_check);
    if keep_inventory {
        SendMessageW(keep_inventory_check, BM_SETCHECK, BST_CHECKED as usize, 0);
    }
    add_form_label(
        window,
        &mut controls,
        "Changing the displayed seed does not alter generated terrain. Rule changes apply to this world.",
        left,
        top,
        352,
    );
    let error_label = create_form_control(
        window,
        "STATIC",
        "",
        0,
        left + 24,
        top + 383,
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
        top + 456,
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
        top + 456,
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
        keep_inventory: keep_inventory_check,
        error_label,
    });
    if let Some(app) = APP.as_mut() {
        app.dialog_open = true;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
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
    use super::{
        build_scene, food_pips, health_pips, hotbar_slot_after_scroll, inventory_slot,
        is_initial_keydown, key_for, matching_creative_items, optional_seed, player_controller,
        prototype_block_for_item, stored_game_mode, GameMode, InventorySlot, StoredGameMode,
    };
    use crate::inventory::{ItemId, ItemKey};
    use crate::render::InputKey;
    use crate::settings::{BindingAction, KeyBindings};
    use crate::terrain::{TerrainGenerator, ROCK};
    use crate::world::ChunkPos;
    use crate::world_store::PlayerSnapshot;

    #[test]
    fn only_unmodified_prototype_block_items_map_to_placeable_blocks() {
        for id in 1..=10 {
            assert_eq!(
                prototype_block_for_item(ItemKey::new(ItemId(id))),
                Some(crate::world::BlockId(id))
            );
        }
        assert_eq!(
            prototype_block_for_item(ItemKey {
                id: ItemId(1),
                variant: 7,
            }),
            None
        );
        assert_eq!(prototype_block_for_item(ItemKey::new(ItemId(11))), None);
    }

    #[test]
    fn hotbar_wheel_scroll_wraps_and_uses_each_notch() {
        assert_eq!(hotbar_slot_after_scroll(0, 1), 8);
        assert_eq!(hotbar_slot_after_scroll(8, -1), 0);
        assert_eq!(hotbar_slot_after_scroll(3, 2), 1);
        assert_eq!(hotbar_slot_after_scroll(3, -2), 5);
    }

    #[test]
    fn creative_catalog_search_is_case_insensitive_and_bounded_to_registered_items() {
        assert_eq!(matching_creative_items(""), (1..=11).collect::<Vec<_>>());
        assert_eq!(matching_creative_items("sNoW"), vec![6]);
        assert_eq!(matching_creative_items("pan"), vec![10]);
        assert!(matching_creative_items("not a block").is_empty());
        assert_eq!(matching_creative_items("  fence  "), vec![9]);
    }

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

    #[test]
    fn inventory_screen_maps_main_hotbar_equipment_and_offhand_slots() {
        for index in 0..36 {
            assert_eq!(inventory_slot(index), Some(InventorySlot::Main(index)));
        }
        assert_eq!(
            inventory_slot(36),
            Some(InventorySlot::Armor(crate::inventory::ArmorSlot::Head))
        );
        assert_eq!(
            inventory_slot(37),
            Some(InventorySlot::Armor(crate::inventory::ArmorSlot::Chest))
        );
        assert_eq!(
            inventory_slot(38),
            Some(InventorySlot::Armor(crate::inventory::ArmorSlot::Legs))
        );
        assert_eq!(
            inventory_slot(39),
            Some(InventorySlot::Armor(crate::inventory::ArmorSlot::Feet))
        );
        assert_eq!(inventory_slot(40), Some(InventorySlot::Offhand));
        assert_eq!(inventory_slot(41), None);
    }

    #[test]
    fn pause_escape_ignores_repeated_keydown_messages() {
        assert!(is_initial_keydown(0));
        assert!(!is_initial_keydown(0x4000_0000));
    }

    #[test]
    fn health_hud_pips_show_full_half_and_empty_hearts() {
        assert_eq!(health_pips(20), [2; 10]);
        assert_eq!(health_pips(7), [2, 2, 2, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(health_pips(0), [0; 10]);
        assert_eq!(health_pips(255), [2; 10]);
    }

    #[test]
    fn hunger_hud_pips_show_full_half_and_empty_food() {
        assert_eq!(food_pips(20), [2; 10]);
        assert_eq!(food_pips(7), [2, 2, 2, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(food_pips(0), [0; 10]);
        assert_eq!(food_pips(255), [2; 10]);
    }

    #[test]
    fn configured_movement_bindings_drive_all_assigned_actions() {
        let mut bindings = KeyBindings::default();
        assert_eq!(key_for(0x57, bindings), vec![InputKey::Forward]);
        bindings.assign(BindingAction::Forward, 0x41, true).unwrap();
        assert_eq!(
            key_for(0x41, bindings),
            vec![InputKey::Forward, InputKey::Left]
        );
        assert!(key_for(0x45, bindings).is_empty());
        assert!(key_for(0x31, bindings).is_empty());
    }

    #[test]
    fn scene_streams_render_region_around_player_without_replacing_edits() {
        let generator = TerrainGenerator::default();
        let mut world = generator.generate_region(2);
        world.set_block([-31, 15, 1], ROCK);
        let mut scene = build_scene(world, generator, (0, 0));

        assert_eq!(scene.mesh_positions.len(), 25);
        assert!(scene
            .mesh_positions
            .contains(&ChunkPos { x: -2, y: 0, z: 2 }));
        assert!(scene.stream_to((1, 0)).unwrap());
        assert_eq!(scene.mesh_positions.len(), 25);
        assert!(scene
            .mesh_positions
            .contains(&ChunkPos { x: 3, y: 0, z: 0 }));
        assert!(scene
            .world
            .get_chunk(ChunkPos { x: 3, y: 0, z: 0 })
            .is_some());
        assert_eq!(scene.world.chunk_count(), 26);
        assert_eq!(scene.world.block_at([-31, 15, 1]), Some(ROCK));
        assert!(!scene.stream_to((1, 0)).unwrap());
        assert!(scene.stream_to((0, 0)).unwrap());
        assert_eq!(scene.world.chunk_count(), 25);
        assert_eq!(scene.world.block_at([-31, 15, 1]), Some(ROCK));
    }
}

#[allow(static_mut_refs)]
unsafe fn close_world_creation_form(window: Hwnd) {
    destroy_world_creation_form();
    if let Some(app) = APP.as_mut() {
        app.dialog_open = TITLE_FORM.is_some();
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
    }
    SetFocus(window);
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn handle_inventory_command(window: Hwnd, command: usize, notification: u16) -> bool {
    if INVENTORY_FORM.is_none() {
        return false;
    }
    if command == INVENTORY_CLOSE_ID || command == DIALOG_IDCANCEL {
        close_inventory_form(window);
        return true;
    }
    if command == INVENTORY_MODE_ID {
        if let Some(form) = INVENTORY_FORM.as_mut() {
            form.secondary_click = !form.secondary_click;
            let label = if form.secondary_click {
                "Click Mode: Secondary"
            } else {
                "Click Mode: Primary"
            };
            let text = wide(label);
            SetWindowTextW(form.mode_button, text.as_ptr());
        }
        return true;
    }
    if command == INVENTORY_DROP_ID {
        drop_inventory_cursor();
        return true;
    }
    if command == INVENTORY_CATALOG_SEARCH_ID && notification == 0x0300 {
        refresh_inventory_form();
        return true;
    }
    if (INVENTORY_CATALOG_ITEM_ID_BASE
        ..INVENTORY_CATALOG_ITEM_ID_BASE + INVENTORY_CATALOG_ITEM_COUNT)
        .contains(&command)
    {
        let button_index = command - INVENTORY_CATALOG_ITEM_ID_BASE;
        let item_id = INVENTORY_FORM
            .as_ref()
            .and_then(|form| form.catalog_visible.get(button_index).copied());
        if let (Some(app), Some(item_id)) = (APP.as_mut(), item_id) {
            if app.player.mode != GameMode::Creative {
                return true;
            }
            let key = ItemKey::new(ItemId(item_id));
            match app.inventory.pick_item_to_hotbar(key, &app.item_catalog) {
                Ok(_) => {
                    if let Some(block) = prototype_block_for_item(key) {
                        app.selected_block = block;
                    }
                    if let Some(scene) = SCENE.as_mut() {
                        scene.dirty = true;
                    }
                    app.logger.event(
                        "INFO",
                        "creative_catalog_pick",
                        format!("item_id={item_id}"),
                    );
                    if let Some(form) = INVENTORY_FORM.as_ref() {
                        let status = wide(&format!(
                            "Selected {} in the hotbar",
                            inventory_item_name(ItemId(item_id))
                        ));
                        SetWindowTextW(form.status_label, status.as_ptr());
                    }
                }
                Err(error) => {
                    if let Some(form) = INVENTORY_FORM.as_ref() {
                        let text = wide(&error.to_string());
                        SetWindowTextW(form.status_label, text.as_ptr());
                    }
                }
            }
            refresh_inventory_form();
        }
        return true;
    }
    if (INVENTORY_SLOT_ID_BASE..INVENTORY_SLOT_ID_BASE + INVENTORY_SLOT_COUNT).contains(&command) {
        let index = command - INVENTORY_SLOT_ID_BASE;
        let secondary = INVENTORY_FORM
            .as_ref()
            .is_some_and(|form| form.secondary_click);
        if let (Some(app), Some(slot)) = (APP.as_mut(), inventory_slot(index)) {
            let before_inventory = app.inventory;
            let before_menu = app.inventory_menu.clone();
            let result = app.inventory_menu.click(
                &mut app.inventory,
                slot,
                if secondary {
                    ClickButton::Secondary
                } else {
                    ClickButton::Primary
                },
                &app.item_catalog,
            );
            if result.is_ok() {
                if before_inventory != app.inventory || before_menu != app.inventory_menu {
                    if let Some(scene) = SCENE.as_mut() {
                        scene.dirty = true;
                    }
                }
                if let Some(form) = INVENTORY_FORM.as_ref() {
                    let empty = wide("");
                    SetWindowTextW(form.status_label, empty.as_ptr());
                }
            } else if let Err(error) = result {
                if let Some(form) = INVENTORY_FORM.as_ref() {
                    let text = wide(&error.to_string());
                    SetWindowTextW(form.status_label, text.as_ptr());
                }
            }
        }
        refresh_inventory_form();
        return true;
    }
    true
}

#[allow(static_mut_refs)]
unsafe fn close_inventory_form(window: Hwnd) {
    let restore_cursor = INVENTORY_FORM
        .as_ref()
        .is_some_and(|form| form.cursor_was_locked);
    let had_cursor = APP
        .as_ref()
        .is_some_and(|app| app.inventory_menu.cursor().is_some());
    if let Some(app) = APP.as_mut() {
        if let Err(error) = app
            .inventory_menu
            .return_cursor(&mut app.inventory, &app.item_catalog)
        {
            if let Some(form) = INVENTORY_FORM.as_ref() {
                let text = wide(&format!(
                    "Cannot close while cursor stack cannot fit: {error}"
                ));
                SetWindowTextW(form.status_label, text.as_ptr());
            }
            app.logger
                .event("WARN", "inventory_close_rejected", error.to_string());
            return;
        }
    }
    destroy_inventory_form();
    if let Some(app) = APP.as_mut() {
        if had_cursor {
            if let Some(scene) = SCENE.as_mut() {
                scene.dirty = true;
            }
        }
        app.dialog_open = false;
        app.input.clear();
        app.food_use_held = false;
        app.food_use.reset();
        app.last_physics = Instant::now();
        if restore_cursor {
            app.cursor_locked = true;
            ShowCursor(0);
            lock_cursor(window);
        }
    }
    SetFocus(window);
    InvalidateRect(window, null(), 0);
}

#[allow(static_mut_refs)]
unsafe fn destroy_inventory_form() {
    if let Some(form) = INVENTORY_FORM.take() {
        for control in form.controls.into_iter().rev() {
            if !control.is_null() {
                DestroyWindow(control);
            }
        }
    }
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
        keep_inventory_check,
        error_label,
    )) = WORLD_FORM.as_ref().map(|form| {
        (
            form.world_id.clone(),
            form.name_edit,
            form.seed_edit,
            form.mode_combo,
            form.difficulty_combo,
            form.difficulty_locked,
            form.keep_inventory,
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
    let keep_inventory = SendMessageW(keep_inventory_check, BM_GETCHECK, 0, 0) == BST_CHECKED;
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
        match store.update_options_and_rules(
            &id,
            &name,
            seed,
            mode,
            difficulty,
            is_locked,
            keep_inventory,
        ) {
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
                app.dialog_open = TITLE_FORM.is_some();
                app.input.clear();
                app.food_use_held = false;
                app.food_use.reset();
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
    match store.create_with_options_and_rules(
        &name,
        seed,
        mode,
        difficulty,
        is_locked,
        keep_inventory,
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
            app.dialog_open = TITLE_FORM.is_some();
            app.input.clear();
            app.food_use_held = false;
            app.food_use.reset();
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

#[allow(static_mut_refs)]
unsafe fn activate_world(window: Hwnd, app: &mut AppState, saved: crate::world_store::StoredWorld) {
    let terrain_seed = saved.metadata.terrain_seed;
    let player_position = saved.player.position;
    let keep_inventory = saved.metadata.keep_inventory;
    app.player = player_controller(&saved.player, saved.metadata.game_mode);
    app.vitals = PlayerVitals::from_saved_health(saved.player.health).unwrap_or_default();
    app.food = saved.player.food;
    app.hunger_clock = crate::survival::HungerClock::default();
    app.activity_exhaustion = crate::survival::ActivityExhaustion::default();
    app.death_items_retained = false;
    app.selected_block = saved.player.selected_block;
    app.inventory = saved.player.inventory;
    app.drops = saved.player.drops.clone();
    let mut death_drops_created = false;
    if !keep_inventory
        && app.vitals.is_dead()
        && matches!(app.player.mode, GameMode::Survival | GameMode::Adventure)
        && app.inventory.saved_slots().iter().any(Option::is_some)
    {
        let position = [
            app.player.transform.x,
            app.player.transform.y,
            app.player.transform.z,
        ];
        match app.inventory.drop_all(
            &mut app.drops,
            position,
            ITEM_PICKUP_DELAY_TICKS,
            &app.item_catalog,
        ) {
            Ok(stack_count) => {
                death_drops_created = stack_count > 0;
                app.logger.event(
                    "INFO",
                    "legacy_death_inventory_dropped",
                    format!("stacks={stack_count}"),
                );
            }
            Err(error) => {
                app.death_items_retained = true;
                app.logger.event(
                    "ERROR",
                    "legacy_death_inventory_drop_failed",
                    error.to_string(),
                );
            }
        }
    }
    app.yaw = saved.player.yaw;
    app.pitch = saved.player.pitch;
    app.world_metadata = Some(saved.metadata);
    app.input.clear();
    app.food_use_held = false;
    app.food_use.reset();
    app.pending_block_actions.clear();
    app.breaking_held = false;
    app.mining_session.reset();
    app.jump_was_pressed = false;
    app.last_jump_press = None;
    app.last_physics = Instant::now();
    let terrain_generator = TerrainGenerator {
        seed: terrain_seed,
        ..TerrainGenerator::default()
    };
    let stream_center = (
        (player_position[0] / CHUNK_SIZE as f64).floor() as i32,
        (player_position[2] / CHUNK_SIZE as f64).floor() as i32,
    );
    let mut world = saved.world;
    if let Err(error) = terrain_generator.ensure_region(
        &mut world,
        ChunkPos {
            x: stream_center.0,
            y: 0,
            z: stream_center.1,
        },
        STREAM_RADIUS,
    ) {
        app.logger
            .event("WARN", "initial_terrain_region_failed", error);
    }
    let mut scene = build_scene(world, terrain_generator, stream_center);
    scene.drop_meshes = build_drop_meshes(&app.drops);
    scene.dirty = death_drops_created;
    if let Some(gpu) = app.gpu.as_mut() {
        upload_scene_meshes(gpu, &scene);
    } else {
        FRAME = Some(build_frame(
            app.camera,
            app.render_width,
            app.render_height,
            &scene,
        ));
    }
    SCENE = Some(scene);
    if death_drops_created {
        if let Some(scene) = SCENE.as_mut() {
            if let Err(error) = persist_world(app, scene) {
                app.logger.event(
                    "ERROR",
                    "death_inventory_migration_save_failed",
                    error.to_string(),
                );
            }
        }
    }
    if TITLE_FORM.is_some() {
        close_title_form(window, true);
    }
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
    if app.vitals.is_dead() {
        PostMessageW(window, WM_APP_DEATH, 0, 0);
    }
}

#[allow(static_mut_refs)]
pub fn run(logger: Option<Logger>) {
    let logger = logger.unwrap_or_else(|| Logger::start("voxel-engine.log").expect("logger"));
    let (settings_store, settings) = match SettingsStore::default_path() {
        Ok(path) => {
            let store = SettingsStore::new(path);
            match store.load() {
                Ok(loaded) => {
                    for warning in loaded.warnings {
                        logger.event("WARN", "settings_load_notice", warning.to_string());
                    }
                    (Some(store), loaded.settings)
                }
                Err(error) => {
                    logger.event("WARN", "settings_load_fallback", error.to_string());
                    (Some(store), Settings::default())
                }
            }
        }
        Err(error) => {
            logger.event("WARN", "settings_path_unavailable", error.to_string());
            (None, Settings::default())
        }
    };
    let loaded = WorldStore::default_root().and_then(|root| {
        let store = WorldStore::new(root);
        let generated = TerrainGenerator::default().generate_region(2);
        let saved = store.open_or_create_prototype(generated)?;
        Ok((store, saved))
    });
    let (world_store, world_metadata, initial_player, mut world) = match loaded {
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
    let terrain_generator = TerrainGenerator {
        seed: world_metadata
            .as_ref()
            .map(|metadata| metadata.terrain_seed)
            .unwrap_or_else(|| TerrainGenerator::default().seed),
        ..TerrainGenerator::default()
    };
    let stream_center = (
        (initial_player.position[0] / CHUNK_SIZE as f64).floor() as i32,
        (initial_player.position[2] / CHUNK_SIZE as f64).floor() as i32,
    );
    if let Err(error) = terrain_generator.ensure_region(
        &mut world,
        ChunkPos {
            x: stream_center.0,
            y: 0,
            z: stream_center.1,
        },
        STREAM_RADIUS,
    ) {
        logger.event("WARN", "initial_terrain_region_failed", error);
    }
    let mut scene = build_scene(world, terrain_generator, stream_center);
    scene.drop_meshes = build_drop_meshes(&initial_player.drops);
    let mut camera = Camera::default();
    camera.fov_degrees = settings.field_of_view_degrees as f32;
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    unsafe {
        APP = Some(AppState {
            camera,
            input: InputState::default(),
            yaw: initial_player.yaw,
            pitch: initial_player.pitch,
            cursor_locked: true,
            width,
            height,
            render_width: (width as u32).min(MAX_RENDER_WIDTH),
            render_height: (height as u32).min(MAX_RENDER_HEIGHT),
            logger,
            settings,
            settings_store,
            last_physics: Instant::now(),
            physics_clock: FixedStepClock::default(),
            gpu: None,
            viewport: null_mut(),
            presenting_dialog: true,
            last_menu_frame: Instant::now(),
            gpu_frame_count: 0,
            gpu_frame_window: Instant::now(),
            selected_block: initial_player.selected_block,
            hotbar_scroll_remainder: 0,
            inventory: initial_player.inventory,
            inventory_menu: InventoryMenu::default(),
            item_catalog: prototype_item_catalog(),
            loot_table: BlockLootTable::prototype(),
            drops: initial_player.drops.clone(),
            death_items_retained: false,
            mining_catalog: MiningCatalog::prototype(),
            mining_session: MiningSession::default(),
            breaking_held: false,
            food_use_held: false,
            food_use: crate::food_use::FoodUseSession::default(),
            sprint_toggled: false,
            sneak_toggled: false,
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
            vitals: PlayerVitals::from_saved_health(initial_player.health).unwrap_or_default(),
            food: initial_player.food,
            activity_exhaustion: crate::survival::ActivityExhaustion::default(),
            hunger_clock: crate::survival::HungerClock::default(),
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
            camera,
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
            WS_OVERLAPPEDWINDOW | 0x0200_0000,
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
            let viewport_class = wide("VoxelEngineViewport");
            let viewport_registration = WndClassW {
                style: CS_HREDRAW | CS_VREDRAW,
                wnd_proc: Some(viewport_proc),
                extra_class: 0,
                extra_window: 0,
                instance,
                icon: null_mut(),
                cursor: null_mut(),
                background: null_mut(),
                menu: null(),
                name: viewport_class.as_ptr(),
            };
            RegisterClassW(&viewport_registration);
            let mut client: Rect = std::mem::zeroed();
            GetClientRect(window, &mut client);
            app.viewport = CreateWindowExW(
                0,
                viewport_class.as_ptr(),
                null(),
                WS_CHILD | 0x0400_0000,
                0,
                0,
                client.right,
                client.bottom,
                window,
                null_mut(),
                instance,
                null_mut(),
            );
            match GpuRenderer::new(
                app.viewport as isize,
                client.right.max(1) as u32,
                client.bottom.max(1) as u32,
            ) {
                Ok(mut gpu) => {
                    if let Some(scene) = SCENE.as_ref() {
                        upload_scene_meshes(&mut gpu, scene);
                    }
                    app.gpu = Some(gpu);
                }
                Err(error) => app.logger.event("WARN", "gpu_unavailable", error),
            }
        }
        SetTimer(window, TIMER_ID, TICK_INTERVAL_MS, null_mut());
        SetTimer(window, RENDER_TIMER_ID, 16, null_mut());
        if let Some(app) = APP.as_mut() {
            // The flip-model swap chain occludes native child controls on some
            // Windows systems. Keep the GDI presenter active until menus are
            // drawn in the GPU compositor, so every menu remains visible.
            app.logger.event(
                "INFO",
                "render_backend",
                "startup_menu_cpu_gdi_gameplay_gpu_on_resume",
            );
        }
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        ShowCursor(0);
        lock_cursor(window);
        open_title_form(window);
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
            let binding_capture_active = OPTIONS_FORM
                .as_ref()
                .is_some_and(|form| form.capture_action.is_some() || form.escape_canceled_down);
            if !binding_capture_active
                && (WORLD_FORM.is_some()
                    || TITLE_FORM.is_some()
                    || INVENTORY_FORM.is_some()
                    || PAUSE_FORM.is_some()
                    || DEATH_FORM.is_some()
                    || OPTIONS_FORM.is_some())
                && IsDialogMessageW(window, &message) != 0
            {
                continue;
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

const STREAM_RADIUS: i32 = 2;

impl Scene {
    fn stream_to(&mut self, center: (i32, i32)) -> Result<bool, &'static str> {
        if center == self.stream_center || self.failed_stream_center == Some(center) {
            return Ok(false);
        }
        let positions = match region_positions(center, STREAM_RADIUS) {
            Ok(positions) => positions,
            Err(error) => {
                self.failed_stream_center = Some(center);
                return Err(error);
            }
        };
        if let Err(error) = self.terrain_generator.ensure_region(
            &mut self.world,
            ChunkPos {
                x: center.0,
                y: 0,
                z: center.1,
            },
            STREAM_RADIUS,
        ) {
            self.failed_stream_center = Some(center);
            return Err(error);
        }
        self.world.evict_generated_chunks_outside(&positions);
        self.meshes = positions
            .iter()
            .map(|position| {
                build_chunk_mesh(
                    &self.world,
                    *position,
                    self.block_textures,
                    self.grass_top_texture,
                )
            })
            .collect();
        self.mesh_positions = positions;
        self.stream_center = center;
        self.failed_stream_center = None;
        Ok(true)
    }
}

fn region_positions(center: (i32, i32), radius: i32) -> Result<Vec<ChunkPos>, &'static str> {
    if !(0..=16).contains(&radius) {
        return Err("terrain region radius is outside the supported bound");
    }
    let mut positions = Vec::with_capacity(((radius * 2 + 1).pow(2)) as usize);
    for z_offset in -radius..=radius {
        for x_offset in -radius..=radius {
            positions.push(ChunkPos {
                x: center
                    .0
                    .checked_add(x_offset)
                    .ok_or("terrain chunk coordinate overflow")?,
                y: 0,
                z: center
                    .1
                    .checked_add(z_offset)
                    .ok_or("terrain chunk coordinate overflow")?,
            });
        }
    }
    Ok(positions)
}

fn build_scene(
    mut world: crate::world::World,
    terrain_generator: TerrainGenerator,
    stream_center: (i32, i32),
) -> Scene {
    let mesh_positions = region_positions(stream_center, STREAM_RADIUS).unwrap_or_default();
    world.evict_generated_chunks_outside(&mesh_positions);
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
    let meshes = mesh_positions
        .iter()
        .map(|position| build_chunk_mesh(&world, *position, block_textures, grass_top.or(dirt)))
        .collect();
    Scene {
        world,
        terrain_generator,
        stream_center,
        failed_stream_center: None,
        meshes,
        drop_meshes: Vec::new(),
        mesh_positions,
        materials,
        block_textures,
        grass_top_texture: grass_top.or(dirt),
        dirty: false,
    }
}

fn prototype_item_catalog() -> ItemCatalog {
    let mut catalog = ItemCatalog::default();
    for id in 1..=11 {
        catalog
            .register(ItemId(id), ItemDefinition::stackable(64))
            .expect("prototype item IDs are unique and valid");
    }
    catalog
}

fn hunger_difficulty(app: &AppState) -> crate::survival::HungerDifficulty {
    use crate::survival::HungerDifficulty as H;
    match app
        .world_metadata
        .as_ref()
        .map(|world| world.difficulty)
        .unwrap_or_default()
    {
        StoredDifficulty::Peaceful => H::Peaceful,
        StoredDifficulty::Easy => H::Easy,
        StoredDifficulty::Normal => H::Normal,
        StoredDifficulty::Hard => H::Hard,
    }
}

fn prototype_block_for_item(key: ItemKey) -> Option<crate::world::BlockId> {
    (key.variant == 0 && (1..=10).contains(&key.id.0)).then_some(crate::world::BlockId(key.id.0))
}

fn hotbar_slot_after_scroll(current: usize, positive_scroll_steps: i32) -> usize {
    (current as i64 - i64::from(positive_scroll_steps)).rem_euclid(9) as usize
}

fn upload_scene_meshes(gpu: &mut GpuRenderer, scene: &Scene) {
    let meshes: Vec<&Mesh> = scene.meshes.iter().chain(&scene.drop_meshes).collect();
    gpu.upload_meshes(&meshes, &scene.materials);
}

fn build_drop_meshes(drops: &ItemDrops) -> Vec<Mesh> {
    let mut combined = Mesh::default();
    for drop in drops.iter() {
        let cube = crate::render::cube_mesh(
            drop.position,
            0.125,
            crate::world::BlockId(drop.stack.key.id.0),
        );
        let vertex_offset = combined.vertices.len() as u32;
        combined.vertices.extend(cube.vertices);
        combined
            .indices
            .extend(cube.indices.into_iter().map(|index| index + vertex_offset));
    }
    if combined.is_empty() {
        Vec::new()
    } else {
        combined.recalculate_bounds();
        vec![combined]
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
    let registry = BlockRegistry::new();
    let mut mesh = mesh_chunk_with_shapes(
        chunk,
        |local_x, local_y, local_z| {
            let world_position = [
                position.x * CHUNK_SIZE as i32 + local_x,
                position.y * CHUNK_SIZE as i32 + local_y,
                position.z * CHUNK_SIZE as i32 + local_z,
            ];
            match (
                world.block_at(world_position),
                world.block_state_at(world_position),
            ) {
                (Some(block), Some(state)) => registry.is_full_cube(block, state),
                _ => false,
            }
        },
        |x, y, z| {
            let world_position = [
                position.x * CHUNK_SIZE as i32 + x,
                position.y * CHUNK_SIZE as i32 + y,
                position.z * CHUNK_SIZE as i32 + z,
            ];
            match (
                world.block_at(world_position),
                world.block_state_at(world_position),
            ) {
                (Some(block), Some(state)) => registry.visual_boxes(block, state),
                _ => crate::blocks::VoxelShape::EMPTY.boxes(),
            }
        },
    );
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

fn targeted_block_hit(app: &AppState, world: &crate::world::World) -> Option<BlockRayHit> {
    let player = app.player.transform;
    let origin = [player.x, player.y + PLAYER_EYE_OFFSET, player.z];
    let direction = [
        app.pitch.cos() as f64 * app.yaw.cos() as f64,
        app.pitch.sin() as f64,
        app.pitch.cos() as f64 * app.yaw.sin() as f64,
    ];
    raycast_blocks(
        world,
        &BlockRegistry::new(),
        origin,
        direction,
        PROTOTYPE_INTERACTION_REACH,
    )
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
    let Some(hit) = targeted_block_hit(app, &scene.world) else {
        app.logger
            .event("TRACE", "block_action_miss", "no target in reach");
        return;
    };
    let position = hit.position.0;
    let harvestable = app.mining_catalog.can_harvest(hit.block, MiningTool::HAND);
    let changed = if breaking && app.player.mode == GameMode::Survival && !harvestable {
        break_block(&mut scene.world, hit).is_some()
    } else if breaking && app.player.mode == GameMode::Survival {
        let drop_position = [
            position[0] as f64 + 0.5,
            position[1] as f64 + 0.5,
            position[2] as f64 + 0.5,
        ];
        match break_block_with_drop(
            &mut scene.world,
            hit,
            &mut app.drops,
            &app.loot_table,
            &app.item_catalog,
            drop_position,
            ITEM_PICKUP_DELAY_TICKS,
        ) {
            Ok(broken) => broken.is_some(),
            Err(error) => {
                app.logger
                    .event("WARN", "block_break_rejected", error.to_string());
                false
            }
        }
    } else if breaking {
        break_block(&mut scene.world, hit).is_some()
    } else {
        let player = app.player.transform;
        let half = app.player.half_extents;
        let player_bounds = Aabb {
            min: [player.x - half[0], player.y - half[1], player.z - half[2]],
            max: [player.x + half[0], player.y + half[1], player.z + half[2]],
        };
        if app.inventory.selected_stack().is_some() {
            place_hotbar_block(
                &mut scene.world,
                &BlockRegistry::new(),
                hit,
                &mut app.inventory,
                &app.item_catalog,
                prototype_block_for_item,
                app.player.mode == GameMode::Survival,
                player_bounds,
            )
        } else if app.player.mode == GameMode::Creative {
            // Preserve the existing Creative-only prototype selector until
            // creative inventory contents are implemented.
            place_block(
                &mut scene.world,
                &BlockRegistry::new(),
                hit,
                app.selected_block,
                player_bounds,
            )
        } else {
            false
        }
    };
    if changed {
        if breaking
            && app.player.mode == GameMode::Survival
            && hunger_difficulty(app) != crate::survival::HungerDifficulty::Peaceful
        {
            app.food.apply_exhaustion(5);
        }
        scene.dirty = true;
        if breaking && app.player.mode == GameMode::Survival {
            scene.drop_meshes = build_drop_meshes(&app.drops);
        }
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
            upload_scene_meshes(gpu, scene);
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
    player.fall_distance = snapshot.fall_distance;
    player.fall_damage_armed = snapshot.fall_damage_armed;
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
        health: app.vitals.health(),
        fall_distance: app.player.fall_distance,
        fall_damage_armed: app.player.fall_damage_armed,
        food: app.food,
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
    let mesh_refs: Vec<&Mesh> = scene.meshes.iter().chain(&scene.drop_meshes).collect();
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

#[allow(static_mut_refs)]
unsafe fn paint_hud(hdc: Hdc, client: Rect, app: &AppState, scene: &Scene) {
    if client.right <= client.left || client.bottom <= client.top {
        return;
    }
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, rgb(245, 245, 245));

    let center_x = (client.left + client.right) / 2;
    let center_y = (client.top + client.bottom) / 2;
    fill_hud_rect(
        hdc,
        Rect {
            left: center_x - 1,
            top: center_y - 7,
            right: center_x + 2,
            bottom: center_y + 8,
        },
        rgb(20, 20, 20),
    );
    fill_hud_rect(
        hdc,
        Rect {
            left: center_x - 6,
            top: center_y - 1,
            right: center_x + 7,
            bottom: center_y + 2,
        },
        rgb(20, 20, 20),
    );
    fill_hud_rect(
        hdc,
        Rect {
            left: center_x,
            top: center_y - 6,
            right: center_x + 1,
            bottom: center_y + 7,
        },
        rgb(245, 245, 245),
    );
    fill_hud_rect(
        hdc,
        Rect {
            left: center_x - 5,
            top: center_y,
            right: center_x + 6,
            bottom: center_y + 1,
        },
        rgb(245, 245, 245),
    );

    let available = (client.right - client.left - 32).max(9 * 28);
    let slot_size = (available / 9).clamp(28, 48);
    let strip_width = slot_size * 9;
    let strip_left = (client.left + client.right - strip_width) / 2;
    let strip_top = client.bottom - slot_size - 18;

    let mode_has_health = matches!(app.player.mode, GameMode::Survival | GameMode::Adventure);
    if mode_has_health {
        let label = wide(&format!(
            "Health: {}/{}",
            app.vitals.health(),
            crate::survival::MAX_HEALTH
        ));
        let mut rect = Rect {
            left: 18,
            top: strip_top - 45,
            right: 180,
            bottom: strip_top - 24,
        };
        DrawTextW(
            hdc,
            label.as_ptr(),
            -1,
            &mut rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
        let start_x = 20;
        let top = strip_top - 19;
        for (heart, value) in health_pips(app.vitals.health()).into_iter().enumerate() {
            let left = start_x + heart as i32 * 19;
            let rect = Rect {
                left,
                top,
                right: left + 16,
                bottom: top + 12,
            };
            fill_hud_rect(hdc, rect, rgb(45, 28, 32));
            if value >= 2 {
                fill_hud_rect(
                    hdc,
                    Rect {
                        left: left + 2,
                        top: top + 2,
                        right: left + 14,
                        bottom: top + 10,
                    },
                    rgb(210, 42, 52),
                );
            } else if value == 1 {
                fill_hud_rect(
                    hdc,
                    Rect {
                        left: left + 2,
                        top: top + 2,
                        right: left + 8,
                        bottom: top + 10,
                    },
                    rgb(210, 42, 52),
                );
            }
        }
        let food_label = wide(&format!(
            "Hunger: {}/{}",
            app.food.food_level(),
            crate::survival::MAX_FOOD_LEVEL
        ));
        let mut food_rect = Rect {
            left: 18,
            top: strip_top - 69,
            right: 180,
            bottom: strip_top - 48,
        };
        DrawTextW(
            hdc,
            food_label.as_ptr(),
            -1,
            &mut food_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
        for (drumstick, value) in food_pips(app.food.food_level()).into_iter().enumerate() {
            let left = start_x + drumstick as i32 * 19;
            let top = strip_top - 42;
            let rect = Rect {
                left,
                top,
                right: left + 16,
                bottom: top + 12,
            };
            fill_hud_rect(hdc, rect, rgb(45, 37, 25));
            if value >= 2 {
                fill_hud_rect(
                    hdc,
                    Rect {
                        left: left + 2,
                        top: top + 2,
                        right: left + 14,
                        bottom: top + 10,
                    },
                    rgb(223, 154, 54),
                );
            } else if value == 1 {
                fill_hud_rect(
                    hdc,
                    Rect {
                        left: left + 2,
                        top: top + 2,
                        right: left + 8,
                        bottom: top + 10,
                    },
                    rgb(223, 154, 54),
                );
            }
        }
    }

    let selected = usize::from(app.inventory.selected_hotbar());
    for index in 0..9 {
        let left = strip_left + index as i32 * slot_size;
        let outer = Rect {
            left,
            top: strip_top,
            right: left + slot_size - 2,
            bottom: strip_top + slot_size - 2,
        };
        fill_hud_rect(
            hdc,
            outer,
            if index == selected {
                rgb(235, 205, 105)
            } else {
                rgb(12, 12, 12)
            },
        );
        fill_hud_rect(
            hdc,
            Rect {
                left: left + 2,
                top: strip_top + 2,
                right: outer.right - 2,
                bottom: outer.bottom - 2,
            },
            rgb(80, 80, 80),
        );
        if let Ok(Some(stack)) = app.inventory.stack(InventorySlot::Main(index)) {
            let block = crate::world::BlockId(stack.key.id.0);
            let color = scene.materials.get(block).color;
            let icon_left = left + 7;
            let icon_top = strip_top + 7;
            fill_hud_rect(
                hdc,
                Rect {
                    left: icon_left,
                    top: icon_top,
                    right: icon_left + slot_size - 18,
                    bottom: icon_top + slot_size - 24,
                },
                rgb(color[0], color[1], color[2]),
            );
            let count = wide(&stack.count.to_string());
            let mut count_rect = Rect {
                left: left + 2,
                top: outer.bottom - 18,
                right: outer.right - 3,
                bottom: outer.bottom - 2,
            };
            DrawTextW(
                hdc,
                count.as_ptr(),
                -1,
                &mut count_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
        }
        let number = wide(&(index + 1).to_string());
        let mut number_rect = Rect {
            left: left + 2,
            top: strip_top + 2,
            right: left + 13,
            bottom: strip_top + 16,
        };
        DrawTextW(
            hdc,
            number.as_ptr(),
            -1,
            &mut number_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }

    if let Some(stack) = app.inventory.selected_stack() {
        let block = crate::world::BlockId(stack.key.id.0);
        let name = BlockRegistry::new().definition(block).name;
        let label = wide(&format!("{}  ×{}", name, stack.count));
        let mut rect = Rect {
            left: strip_left,
            top: strip_top - 24,
            right: strip_left + strip_width,
            bottom: strip_top - 4,
        };
        DrawTextW(
            hdc,
            label.as_ptr(),
            -1,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

unsafe fn fill_hud_rect(hdc: Hdc, rect: Rect, color: u32) {
    let brush = CreateSolidBrush(color);
    if !brush.is_null() {
        FillRect(hdc, &rect, brush);
        DeleteObject(brush);
    }
}

unsafe fn menu_texture_brush(button: bool) -> *mut c_void {
    let cached = if button {
        &BUTTON_TEXTURE_BRUSH
    } else {
        &PANEL_TEXTURE_BRUSH
    };
    *cached.get_or_init(|| {
        let mut pixels = Vec::with_capacity(16 * 16 * 4);
        for y in 0..16u32 {
            for x in 0..16u32 {
                let fleck = (x * 13 + y * 7 + (x ^ y) * 3) % 29 == 0;
                let seam = x % 8 == 0 || y % 8 == 0;
                let color = if button {
                    if fleck {
                        rgb(59, 78, 83)
                    } else if seam {
                        rgb(31, 43, 50)
                    } else {
                        rgb(40, 54, 62)
                    }
                } else if fleck {
                    rgb(45, 59, 63)
                } else if seam {
                    rgb(20, 29, 35)
                } else {
                    rgb(29, 40, 46)
                };
                pixels.extend_from_slice(&color.to_le_bytes());
            }
        }
        let bitmap = CreateBitmap(16, 16, 1, 32, pixels.as_ptr() as *const c_void);
        if bitmap.is_null() {
            return 0;
        }
        let brush = CreatePatternBrush(bitmap);
        DeleteObject(bitmap);
        brush as usize
    }) as *mut c_void
}

unsafe fn draw_menu_button(item: &mut DrawItemStruct) {
    let rect = item.item_rect;
    let inset = Rect {
        left: rect.left + 2,
        top: rect.top + 2,
        right: rect.right - 2,
        bottom: rect.bottom - 2,
    };
    let texture = menu_texture_brush(true);
    if !texture.is_null() {
        FillRect(item.hdc, &inset, texture);
    }
    let pressed = item.item_state & ODS_SELECTED != 0;
    let disabled = item.item_state & ODS_DISABLED != 0;
    let frame = if item.item_state & ODS_FOCUS != 0 {
        rgb(224, 190, 117)
    } else if pressed {
        rgb(64, 102, 111)
    } else {
        rgb(108, 137, 143)
    };
    fill_hud_rect(
        item.hdc,
        Rect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 2,
        },
        frame,
    );
    fill_hud_rect(
        item.hdc,
        Rect {
            left: rect.left,
            top: rect.top + 2,
            right: rect.left + 2,
            bottom: rect.bottom - 2,
        },
        frame,
    );
    fill_hud_rect(
        item.hdc,
        Rect {
            left: rect.right - 2,
            top: rect.top + 2,
            right: rect.right,
            bottom: rect.bottom - 2,
        },
        frame,
    );
    fill_hud_rect(
        item.hdc,
        Rect {
            left: rect.left + 2,
            top: rect.bottom - 2,
            right: rect.right - 2,
            bottom: rect.bottom,
        },
        rgb(17, 23, 28),
    );

    let length = GetWindowTextLengthW(item.item_window).max(0) as usize;
    let mut label = vec![0u16; length + 1];
    GetWindowTextW(item.item_window, label.as_mut_ptr(), label.len() as i32);
    let mut text_rect = Rect {
        left: inset.left + 8,
        top: inset.top + if pressed { 2 } else { 0 },
        right: inset.right - 8,
        bottom: inset.bottom,
    };
    SetBkMode(item.hdc, TRANSPARENT);
    SetTextColor(
        item.hdc,
        if disabled {
            rgb(139, 148, 150)
        } else {
            rgb(238, 241, 235)
        },
    );
    DrawTextW(
        item.hdc,
        label.as_ptr(),
        -1,
        &mut text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn health_pips(health: u8) -> [u8; 10] {
    std::array::from_fn(|index| {
        health
            .min(crate::survival::MAX_HEALTH)
            .saturating_sub((index * 2) as u8)
            .min(2)
    })
}

fn food_pips(food_level: u8) -> [u8; 10] {
    std::array::from_fn(|index| {
        food_level
            .min(crate::survival::MAX_FOOD_LEVEL)
            .saturating_sub((index * 2) as u8)
            .min(2)
    })
}
