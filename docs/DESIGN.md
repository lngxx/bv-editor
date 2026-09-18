# bv-editor — เอกสารออกแบบสถาปัตยกรรม (v0.1 draft)

สถานะ: ฉบับร่างสำหรับคุยกัน — ยังไม่มีโค้ด implementation ใดๆ
วันที่: 2026-09-18

## 1. เป้าหมายและข้อจำกัด (Goals & Constraints)

1. เป็น **editor ทั่วไปสำหรับเกมที่ทำด้วย Bevy** ไม่ผูกกับเกมใดเกมหนึ่ง
2. **ฝังเข้าเกมไหนก็ได้** ผ่าน `app.add_plugins(EditorPlugin)` (บวก `add_systems` ตามจำเป็น) — เกมไม่ต้องแก้โค้ดของตัวเองเพื่อรองรับ editor
3. รองรับ **extension** — คนอื่นเพิ่มแผง/เครื่องมือใหม่เข้า editor ได้โดยไม่ต้องแก้ core crate
4. เลือก **built-in lib ของ Bevy ให้มากที่สุดก่อน** หลีกเลี่ยง dependency นอกวงจนกว่าจะจำเป็นจริง
5. UI: `bevy_ui` (retained mode, native) — **ไม่ใช้ egui**
6. Extension model: **static Rust plugin ผ่าน Cargo** (compile-time, ผ่าน `Plugin` trait ของ Bevy เอง) — **ไม่ใช้** dynamic dylib/FFI และ **ไม่ใช้** scripting engine
7. แบ่งงานเป็น phase เล็กๆ ที่ทดสอบได้จริงทีละขั้น

ผลจากข้อ 5+6: ต้นทุนหลักของโปรเจกต์นี้คือ **ต้องสร้าง widget/docking framework เองบน `bevy_ui`** เพราะ Bevy เองยังไม่มี docking panel, tree view, property grid สำเร็จรูป (ต่างจากสาย `bevy_egui + egui_dock` ที่ของพวกนี้มีคนทำไว้แล้ว) ส่วน extension model แบบ static plugin นั้นง่ายและปลอดภัยที่สุดเพราะใช้กลไก `Plugin` ของ Bevy ตรงๆ แลกมาด้วยการที่ extension ใหม่ต้อง recompile editor ทุกครั้ง (ยอมรับได้ตามที่เลือก)

## 2. แนวคิดสถาปัตยกรรมระดับบนสุด

```
Host Game App (ของผู้ใช้ editor)
 └─ app.add_plugins(GamePlugin)      // โค้ดเกมปกติ ไม่ต้องรู้จัก editor
 └─ app.add_plugins(EditorPlugin)    // บรรทัดเดียวที่เพิ่มเข้ามา
      ├─ EditorCorePlugin            // state, schedule set, resources กลาง
      ├─ EditorUiPlugin              // docking/shell framework (bevy_ui)
      ├─ ScenePanelPlugin            // built-in extension #1
      ├─ InspectorPanelPlugin        // built-in extension #2
      ├─ AssetBrowserPanelPlugin     // built-in extension #3
      ├─ ViewportGizmoPlugin         // built-in extension #4
      └─ UndoRedoPlugin
 └─ app.add_plugins(MyCustomToolPlugin) // extension ของบุคคลที่ 3 (คนละ crate)
```

หลักการสำคัญ: **built-in panel ทุกตัวถูก implement ผ่าน API ตัวเดียวกับที่ extension ภายนอกใช้** (dogfooding) เพื่อบังคับให้ extension API ดีพอจริงๆ ตั้งแต่ต้น ไม่ใช่ core panel มี "ทางลัด" พิเศษที่ extension เข้าไม่ถึง

Editor ทำงานเป็น **overlay ที่สลับเปิด/ปิดได้ในโปรเซสเดียวกับเกม** (in-process, ตาม `add_plugins`) ไม่ใช่โปรแกรม editor แยกที่คุยกับเกมผ่าน IPC — ตรงตาม requirement ข้อ 2 อยู่แล้ว และเป็นโมเดลเดียวกับ `bevy_editor_pls` / เครื่องมือแนว Jackdaw ในภาพตัวอย่าง

### EditorState

ใช้ Bevy `States`:

```
enum EditorState { Editing, Playing, Paused }
```

- `Editing` = UI ของ editor แสดง, เกม logic (game-only systems) หยุด/ไม่รัน
- `Playing` = UI ของ editor ซ่อน (หรือแสดงบางส่วน เช่นปุ่ม stop), เกมรันปกติ
- `Paused` = เกมหยุดกลางอากาศ แต่ inspector ยังเปิดดู state ได้ (debug ระหว่างเล่น)

เกมฝั่ง host แยก system ของตัวเองด้วย run condition `.run_if(in_state(EditorState::Playing))` หรือใส่ set ที่ editor กำหนดให้ — เอกสารต้องระบุ convention นี้ชัดเจนใน phase 9

### สิ่งที่ตั้งใจ "ไม่ทำ" ใน v1 (out of scope เพื่อกันงานบานปลาย)

- **Prefab system** (บันทึก subtree เป็น reusable asset ที่ link กับ instance หลายตัว) — ฟีเจอร์นี้ใหญ่และมีผลต่อ data model ของ scene ทั้งระบบ ถ้าจะทำควรทำหลัง v1 เสถียรแล้ว (แตะ backlog ไว้ในข้อ 9)
- **Dynamic/hot-reload extension** — ตัดสินใจแล้วในข้อ 6 (static plugin เท่านั้น)
- **Multi-window editor** (แยกหน้าต่าง inspector ออกจากหน้าต่างหลัก) — เริ่มจากหน้าต่างเดียวก่อน
- **Cross-application clipboard** (copy entity ข้ามโปรแกรม/เครื่อง) — ทำ copy/paste ภายใน editor เดียวกันพอสำหรับ v1 (ใช้ resource ธรรมดา ไม่ต้องพึ่ง OS clipboard)
- **WASM/web build ของตัว editor เอง** — ยังไม่ตั้งเป้า target นี้ ถ้าต้องการในอนาคตค่อยตรวจ compatibility ของแต่ละ crate ทีหลัง

การระบุ "ไม่ทำ" เหล่านี้ตั้งแต่ต้น มีไว้กันไม่ให้ scope ของ v1 บวมจนไม่จบสักที ไม่ใช่การปฏิเสธถาวร

## 3. โครงสร้าง Workspace / Crate

ใช้ Cargo workspace แยก crate ตามความรับผิดชอบ เพื่อให้ extension ผูกเฉพาะส่วนที่ต้องใช้ และแยก compile time:

```
bv-editor/
├── Cargo.toml                (workspace)
├── crates/
│   ├── bv_editor_core/        # state, schedule labels, EditorPlugin entrypoint, Selection resource, HotkeyRegistry, EditorOnly marker
│   ├── bv_editor_ui/          # docking framework, splitter, tab bar, tree widget, property grid (bevy_ui)
│   ├── bv_editor_reflect_ui/  # generic field editor จาก bevy_reflect + TypeRegistry รวมถึง asset-reference/Handle<T> picker widget
│   ├── bv_editor_extension/   # trait EditorExtension, EditorAppExt (register_panel ฯลฯ), registry Resource
│   ├── bv_editor_undo/        # Command trait, UndoStack resource
│   ├── bv_editor_scene_panel/     # built-in: Scene Tree
│   ├── bv_editor_inspector_panel/ # built-in: Components/Materials/Resources tab
│   ├── bv_editor_assets_panel/    # built-in: Project Files browser
│   ├── bv_editor_console_panel/   # built-in: Console/Log panel (อ่าน tracing log ของ Bevy)
│   ├── bv_editor_gizmo_api/       # trait/registry ของระบบ gizmo (ดูข้อ 7) — ไม่มี manipulator จริงอยู่ในนี้
│   ├── bv_editor_gizmos_builtin/  # built-in: Translate/Rotate/Scale manipulator + component gizmo ตัวอย่าง (light, camera frustum)
│   ├── bv_editor_viewport/        # editor camera (orbit/pan/zoom), render-to-texture, picking, ต่อ gizmo_api เข้ากับ frame loop
│   ├── bv_editor_test_utils/      # headless test harness: build App แบบไม่มีหน้าต่าง, ยิง input จำลอง, step frame — ใช้ทดสอบทุก phase แบบอัตโนมัติ
│   └── bv_editor/             # facade crate — re-export ทุกอย่าง, ประกอบเป็น EditorPlugin เดียว
├── examples/
│   ├── minimal_game/          # เกมเปล่าๆ + add_plugins(EditorPlugin) — ใช้ทดสอบทุก phase
│   └── third_party_extension/ # ตัวอย่าง extension นอก workspace หลัก จำลองผู้ใช้จริง
└── docs/
    └── DESIGN.md              (ไฟล์นี้)
```

เหตุผลที่แยกละเอียดขนาดนี้:
- ผู้ใช้ที่อยากได้แค่ scene tree เบาๆ ไม่ต้อง pull inspector/asset browser เข้ามาด้วย
- บังคับ boundary ระหว่าง "framework" (`bv_editor_ui`, `bv_editor_extension`) กับ "content" (panel ต่างๆ) ให้ชัด — panel ต้องอยู่ได้ด้วย public API ของ framework เท่านั้น
- `bv_editor` facade คือสิ่งเดียวที่ผู้ใช้ทั่วไป `cargo add` แล้วได้ทุกอย่างแบบ default (คล้าย `bevy` ที่ re-export subcrate)

## 4. เทคโนโลยีที่เลือกใช้ (Bevy built-in ก่อนเสมอ)

**Bevy version ที่ล็อกไว้ทั้งโปรเจกต์ (Phase 0): `0.19.1`** (เวอร์ชัน stable ล่าสุดตอนเริ่ม, pin แบบ exact `=0.19.1` ทั้ง workspace ผ่าน `[workspace.dependencies]` ใน root `Cargo.toml` + คอมมิต `Cargo.lock`)

| ความต้องการ | เลือกใช้ | หมายเหตุ |
|---|---|---|
| UI layout/rendering | `bevy_ui` | ตามที่เลือก — native, ไม่มี docking/tree/property widget สำเร็จรูป ต้องสร้างเอง (phase 1) |
| Widget interaction พื้นฐาน (button, slider, checkbox state) | `bevy_ui_widgets` (headless state machine) + `bevy_feathers` (styled editor widgets) | **ตรวจแล้วใน Phase 0:** ชื่อ `bevy_core_widgets` ที่ตั้งเป้าไว้ตอนแรกเป็นแค่ placeholder crate (`0.0.0`, ไม่มีโค้ดจริง) — ของจริงคือ `bevy_ui_widgets` ("Unstyled common widgets for Bevy Engine") ซึ่ง `bevy_feathers` ("A collection of UI widgets for building editors and utilities in Bevy") พึ่งอยู่ ทั้งคู่ stable ที่ `0.19.1` ตรงกับ Bevy version ที่ล็อกไว้ทั้งโปรเจกต์ (ดูข้อ 4 บรรทัดแรก) ไม่ต้อง fallback เขียน state machine เองบน `Interaction` |
| Reflection สำหรับ inspector อัตโนมัติ | `bevy_reflect` + `AppTypeRegistry` | หัวใจของ inspector — component ไหน derive `Reflect` แล้วจะโผล่ให้แก้ได้อัตโนมัติโดยไม่ต้องเขียน UI เฉพาะทีละ type |
| Selection ใน viewport | `bevy_picking` (mesh picking backend) | built-in ใน Bevy แล้ว ไม่ต้องพึ่ง `bevy_mod_picking` ภายนอก |
| Gizmo วาดเส้น/ลูกศร transform handle | `bevy_gizmos` | built-in, ใช้วาด outline ของ selection และแกน translate/rotate/scale — สถาปัตยกรรมแบบขยายได้อยู่ในข้อ 7 |
| Ray casting สำหรับ hit-test gizmo | `bevy_math::Ray3d` + `Camera::viewport_to_world` | ทั้งคู่ built-in อยู่แล้ว ไม่ต้องเขียน ray-cast math เอง แค่ประกอบ |
| กล้อง editor แยกจากกล้องเกม (orbit/pan/zoom) + render 3D ลงใน panel bevy_ui | `Camera` ตัวที่สอง + `RenderTarget::Image` + `ImageNode` | built-in ทั้งหมด: เรนเดอร์ฉากลง texture แล้วเอา texture ไปแปะใน `bevy_ui` panel — วิธีมาตรฐานของ Bevy เองสำหรับฝัง 3D viewport ใน UI |
| Scene hierarchy | `Parent`/`Children` (bevy_ecs) ปกติของเกม | scene tree panel แค่ "อ่าน" ECS จริง ไม่มี hierarchy คู่ขนานของตัวเอง |
| Save/Load scene | `bevy_scene` (`DynamicScene`) + RON | serialization format ใช้ของ Bevy เอง |
| Asset listing | `bevy_asset::AssetServer` + `std::fs` | project files panel อ่านโฟลเดอร์จริงบน disk |
| Project manifest (`project.json`) | `serde` + `serde_json` | เก็บ list scene, path assets, settings — ไม่ใช่ ECS scene จึงไม่ใช้ bevy_scene ตรงนี้ |
| Native "Open Folder" dialog (ถ้าต้องการ) | *ยังไม่ใช้* — ใช้ file browser ในตัว editor เอง (phase 5) แทน | เพื่อคง "built-in only" ไว้ได้จริง ถ้าอนาคตอยากได้ native dialog ค่อยพิจารณา `rfd` เป็น dependency นอกวงเดียวที่ยอมรับ |
| Undo/Redo | เขียนเอง (`Command` trait + `Vec<Box<dyn Command>>`) | ไม่มี built-in ของ Bevy โดยตรง แต่ implement บน `World`/Commands ปกติได้ |
| Console/Log panel | `tracing` + `tracing-subscriber` (ที่ `bevy_log` ใช้อยู่แล้ว) | เพิ่ม custom `Layer` ดัก log event เก็บลง resource แสดงใน panel — ของที่ Bevy มีอยู่แล้วในวง ไม่ต้องเพิ่ม dependency ใหม่ |
| Headless test harness | `App` + `MinimalPlugins` (bevy_app/bevy_ecs) รันแบบไม่เปิดหน้าต่างจริง | built-in ทั้งหมด: `App::update()` เรียกเป็น loop แทนการรอ event loop จริง, ยิง `ButtonInput<MouseButton>`/`CursorMoved` เป็น resource/event ตรงๆ เพื่อจำลอง input |

`serde`/`ron`/`serde_json` ถือเป็น dependency มาตรฐานที่ Bevy เองก็ใช้อยู่แล้ว (bevy_scene พึ่ง serde) จึงนับเป็น "อยู่ในวง built-in" ไม่ใช่ของนอก

## 5. Extension API (โครงร่าง แนวคิด ไม่ใช่โค้ดจริง)

```rust
// bv_editor_extension — แนวคิดของ trait/สัญญา ไม่ใช่ implementation
pub trait EditorExtension: Plugin {
    fn panels(&self) -> Vec<PanelDescriptor> { Vec::new() }
    fn menu_items(&self) -> Vec<MenuItemDescriptor> { Vec::new() }
    fn inspector_widgets(&self) -> Vec<InspectorWidgetDescriptor> { Vec::new() }
}

pub trait EditorAppExt {
    fn register_editor_panel<P: Panel>(&mut self, desc: PanelDescriptor) -> &mut Self;
    fn register_inspector_widget<T: Reflect>(&mut self, widget: fn(&mut T, &mut UiBuilder)) -> &mut Self;
}
```

แนวทาง:
- extension = Rust crate ธรรมดา ที่ `impl Plugin` แล้วใน `build()` เรียก `app.register_editor_panel(...)`
- ผู้ใช้เพิ่ม extension ด้วยการ `cargo add my_extension` แล้ว `app.add_plugins((EditorPlugin, MyExtensionPlugin))` — ตรงกับ idiom `add_plugins` ที่ requirement ระบุไว้พอดี
- panel ที่ลงทะเบียนแล้วเก็บใน `Resource` กลาง (`PanelRegistry`) ที่ docking framework (`bv_editor_ui`) อ่านตอน build layout — panel ไม่รู้จัก docking framework โดยตรง มีแต่ descriptor + system ที่ render เนื้อหา
- Built-in panels ทั้งหมด (Scene Tree, Inspector, Asset Browser) เป็น "extension" ที่มากับ `bv_editor` facade โดย default — พิสูจน์ว่า API พอสำหรับ 3rd party จริง เพราะตัวเองก็ใช้ API เดียวกัน

## 6. การไหลของข้อมูล (data flow) หลัก

1. `bevy_picking` ยิง pointer event บน mesh ในโลก → observer เขียนลง `Resource<Selection>` (หรือคลิกจาก Scene Tree panel ก็เขียน resource เดียวกัน — สอง entry point, source of truth เดียว)
2. Inspector panel เป็น system ที่ `read` `Selection` + query component ของ entity นั้น + `AppTypeRegistry` → generate UI ต่อ field แบบ generic ผ่าน `bv_editor_reflect_ui`
3. แก้ค่าใน UI → เขียนกลับผ่าน `bevy_reflect::ReflectMut` เข้า component จริงทันที (ไม่มี buffer พิเศษ) — ทำให้ viewport/gizmo เห็นผลทันทีเพราะอ่าน component เดียวกัน
4. operation ที่ "แก้ไขจริงจัง" (ลบ entity, เพิ่ม component, ย้าย transform เสร็จสิ้นหนึ่งครั้ง) ห่อเป็น `Command` แล้ว push เข้า `UndoStack` แทนการแก้ world ตรงๆ

## 7. ระบบ Gizmo แบบขยายได้ (3D viewport, transform manipulator, custom gizmo)

เป้าหมายของหัวข้อนี้: ให้ **ใครก็ตามเพิ่ม gizmo ใหม่เข้ามาได้ในอนาคต** โดยไม่ต้องแก้ core — เหมือนกับที่ panel ทำได้ในข้อ 5 หลักการเดียวกัน คือแยก "framework/registry" (`bv_editor_gizmo_api`) ออกจาก "ของจริง" (`bv_editor_gizmos_builtin`) แล้วบังคับให้ built-in ใช้ registry เดียวกับ 3rd party

Gizmo ในระบบนี้แบ่งเป็น 2 ประเภทที่ independent กัน:

1. **Manipulator** — เครื่องมือแบบเลือกทีละอันจาก toolbar (Translate/Rotate/Scale คือ 3 ตัวแรกที่มากับ built-in) มี handle ให้ลากด้วยเมาส์ แก้ transform ของ selection
2. **Component gizmo** — ภาพประกอบที่ผูกกับ component type (เช่น วาดทรงกลมแสดงระยะของ `PointLight`, กรวยของ `SpotLight`, กรอบของ camera frustum) วาดอัตโนมัติเมื่อ entity มี component นั้น ไม่ต้องมี manipulator มา interact ด้วยก็ได้ (เป็นแค่ visualization) — เพิ่มได้อิสระโดยไม่กระทบ manipulator เลย

### 7.1 Editor camera & การฝัง 3D viewport ใน bevy_ui

เนื่องจาก UI ทั้งระบบเป็น `bevy_ui` (retained, 2D layout) แต่ viewport ต้องเป็นภาพ 3D จริง วิธีมาตรฐานของ Bevy คือ:

- เพิ่ม `EditorCamera` (marker component) เป็นกล้องที่สองแยกจากกล้องของเกม, เรนเดอร์ลง `RenderTarget::Image` (offscreen texture) แทนหน้าจอโดยตรง
- panel "Scene" ใน dock layout เป็นแค่ `ImageNode` ที่โชว์ texture นั้น — resize panel แล้ว resize texture ตาม (`Image` แก้ size ได้ runtime)
- `EditorCamera` มี controller ของตัวเอง: right-drag = orbit, middle-drag = pan, scroll = zoom (ทำเป็น system ธรรมดา อ่าน mouse input, active เฉพาะตอน pointer อยู่เหนือ panel viewport)
- `EditorCamera` ทำงานเฉพาะ `EditorState::Editing` / `Paused` เท่านั้น เมื่อเข้า `Playing` แนะนำให้มี 2 แท็บใน viewport แบบ Unity: **Scene** (มุมมอง editor + gizmo) กับ **Game** (สิ่งที่กล้องจริงของเกมเห็น) — ผู้เล่นทดสอบเกมได้โดยไม่เสีย gizmo/selection ระหว่างพัฒนา (รายละเอียด toggle นี้ผูกกับ Phase 9)

### 7.2 Core types (โครงร่างแนวคิด)

```rust
// bv_editor_gizmo_api — สัญญา ไม่ใช่ implementation จริง

/// อ่านอย่างเดียวต่อเฟรม ส่งให้ manipulator/component gizmo ทุกตัวใช้ร่วมกัน
pub struct GizmoDrawContext<'w> {
    pub camera: &'w Camera,
    pub camera_transform: &'w GlobalTransform,
    pub selection: &'w [Entity],
    pub pivot: Vec3,              // จุดหมุน/อ้างอิงของ selection ปัจจุบัน
    pub space: GizmoSpace,        // Local | World
    pub snap: SnapSettings,       // grid step, angle step, on/off
}

pub enum GizmoSpace { Local, World }

/// token ระบุ handle ที่ hover/drag อยู่ — manipulator แต่ละตัวกำหนดความหมายเอง
pub struct HandleId(pub u32);

pub trait Manipulator: Send + Sync + 'static {
    fn descriptor(&self) -> ManipulatorDescriptor; // ชื่อ, ไอคอน, hotkey สำหรับ toolbar

    /// วาด handle ของ manipulator (เรียกทุกเฟรมที่มี selection ไม่ว่าง)
    fn draw(&self, ctx: &GizmoDrawContext, gizmos: &mut Gizmos);

    /// ทดสอบว่า ray จากเมาส์โดน handle ไหนไหม
    fn hit_test(&self, ctx: &GizmoDrawContext, ray: Ray3d) -> Option<HandleId>;

    /// เรียกทุกเฟรมระหว่างลาก handle — คืนค่า transform ใหม่ที่ manipulator "เสนอ" ให้ selection
    fn on_drag(&mut self, ctx: &GizmoDrawContext, handle: HandleId, pointer_delta: Vec2) -> ManipulatorDelta;

    /// ปล่อยเมาส์ — ห่อ operation ทั้งหมดเป็น Command เดียวเพื่อ push เข้า UndoStack (ข้อ 8, Phase 8)
    fn on_drag_end(&mut self) -> Option<Box<dyn EditorCommand>>;
}

pub trait ComponentGizmo<C: Component>: Send + Sync + 'static {
    fn draw(&self, entity: Entity, component: &C, ctx: &GizmoDrawContext, gizmos: &mut Gizmos);
    /// ค่า default: วาดเฉพาะตอน entity ถูกเลือก — ปรับเป็น "วาดตลอด" ได้ต่อ gizmo
    fn visibility(&self) -> GizmoVisibility { GizmoVisibility::WhenSelected }
}

pub trait GizmoAppExt {
    fn register_manipulator<M: Manipulator + FromWorld>(&mut self) -> &mut Self;
    fn register_component_gizmo<C: Component, G: ComponentGizmo<C> + FromWorld>(&mut self) -> &mut Self;
}
```

จุดสำคัญของ API นี้:

- **มี shared math helper ให้ใช้ร่วมกัน** ใน `bv_editor_gizmo_api::math` (ray-plane intersection, screen-space axis projection, arcball rotation) เพื่อไม่ให้คนเขียน manipulator ใหม่ต้องคำนวณ ray/plane เองใหม่ทุกครั้ง — จุดนี้คือสิ่งที่ทำให้ "เพิ่ม gizmo เอง" ทำได้จริงโดยไม่ต้องเชี่ยวชาญ 3D math มาก
- `on_drag` **ไม่แก้ world ตรงๆ** แต่คืน `ManipulatorDelta` (translation/rotation/scale) ให้ระบบกลางใน `bv_editor_viewport` เป็นคนเขียนลง `Transform` จริง — ทำให้ manipulator ทดสอบ (unit test) ได้โดยไม่ต้องมี ECS world เลย เพราะเป็น pure function ของ input
- toolbar ในข้อ 5/section 1 **ไม่ hardcode ปุ่ม Translate/Rotate/Scale** แต่ query `ManipulatorRegistry` แล้ว render ปุ่มตาม `descriptor()` ของทุกตัวที่ลงทะเบียนไว้ — เพิ่ม manipulator ใหม่แล้วโผล่ในแถบเครื่องมือเองอัตโนมัติ ไม่ต้องแก้ `bv_editor_ui`
- component gizmo แยก pass ตัวเอง วน `ComponentGizmoRegistry` (เก็บ `TypeId -> Vec<Box<dyn ErasedComponentGizmo>>`) ทุกเฟรม ไม่ผูกกับ manipulator ที่เลือกอยู่เลย — เพิ่ม gizmo ใหม่ (เช่น "แสดงเส้น path ของ spline component") ไม่กระทบ Translate/Rotate/Scale ที่มีอยู่

### 7.3 ตัวอย่างการเพิ่ม gizmo ใหม่ (สำหรับ 3rd party ในอนาคต)

- Manipulator ใหม่ เช่น "Vertex Snap" หรือ "Duplicate-along-path": `impl Manipulator for MyTool { ... }` แล้ว `app.register_manipulator::<MyTool>()` — โผล่ในแถบเครื่องมือทันที ใช้ hotkey ที่ตั้งใน descriptor
- Component gizmo ใหม่ เช่น แสดง bounding box ของ collider component ที่ผู้ใช้นิยามเอง: `impl ComponentGizmo<MyCollider> for MyColliderGizmo { ... }` แล้ว `app.register_component_gizmo::<MyCollider, MyColliderGizmo>()`
- ทั้งสองกรณีเป็น Rust crate ปกติที่ `add_plugins` เข้าไป — ใช้กลไก extension เดียวกับข้อ 5 ทุกประการ (static plugin ผ่าน Cargo)

### 7.4 Built-in ที่มากับ `bv_editor_gizmos_builtin`

- `TranslateManipulator`, `RotateManipulator`, `ScaleManipulator` (3 ปุ่มแรกในแถบเครื่องมือ ตามภาพตัวอย่างต้นฉบับ)
- `PointLightRangeGizmo`, `SpotLightConeGizmo` (component gizmo ตัวอย่าง — พิสูจน์ว่า API พอสำหรับ 3rd party จริง เพราะตัวมันเองก็ผ่าน `register_component_gizmo` เหมือนกัน)
- ปุ่ม toggle `GizmoSpace` (Local/World) และ `SnapSettings` (grid/angle snap on-off) เป็นส่วนหนึ่งของ toolbar เดียวกัน อ่าน/เขียน `GizmoDrawContext` ที่ manipulator ทุกตัวเห็นร่วมกัน

## 8. ประเด็นข้ามระบบ (cross-cutting concerns) ที่ควรวางไว้ตั้งแต่ต้น

ส่วนนี้คือช่องโหว่ที่มักถูกมองข้ามตอนออกแบบ panel/gizmo แยกกันทีละอัน แต่กระทบทุก phase ถ้าไม่วางไว้ก่อน

### 8.1 Headless testing infrastructure

หลาย phase ก่อนหน้าเขียนว่า "เทส" แบบดูด้วยตา ซึ่งไม่ยั่งยืนพอจะเรียกว่า "เทสได้จริงทีละขั้น" ตามที่ตั้งเป้าไว้ แนะนำสร้าง `bv_editor_test_utils` ตั้งแต่ **Phase 0** ให้เป็นของกลาง มีฟังก์ชันมาตรฐานเช่น:

```rust
// bv_editor_test_utils — สัญญา ไม่ใช่ implementation จริง
pub fn headless_app() -> App;                       // App + MinimalPlugins, ไม่เปิดหน้าต่าง
pub fn step(app: &mut App, frames: u32);             // เรียก app.update() ซ้ำ n ครั้ง
pub fn simulate_click(app: &mut App, world_pos: Vec2);
pub fn simulate_key(app: &mut App, key: KeyCode);
```

มีของนี้แล้ว หลาย milestone ที่เขียนว่า "ดูด้วยตา" ในข้อ 9 (เช่น phase 2 tree ตรงกับ hierarchy, phase 8 undo/redo snapshot) เปลี่ยนเป็น automated test จริงได้ทันที และรัน CI ได้ (GitHub Actions รัน `cargo test` แบบไม่ต้องมี GPU/หน้าต่างจริงสำหรับ logic ล้วนๆ)

### 8.2 Hotkey registry ส่วนกลาง

เมื่อ extension เพิ่มขึ้นเรื่อยๆ (manipulator ใหม่ในข้อ 7, panel ใหม่ในข้อ 5) แต่ละตัวอาจอยากได้ hotkey เป็นของตัวเอง ถ้าปล่อยให้ทุก panel/manipulator เช็ค `KeyCode` เองตรงๆ จะชนกันโดยไม่รู้ตัว แนะนำมี `HotkeyRegistry` resource กลางใน `bv_editor_core`:

- extension "จอง" hotkey ผ่าน `app.register_hotkey(HotkeyDescriptor { key, when: EditorState::Editing, id })` แทนการอ่าน `ButtonInput<KeyCode>` ตรงๆ
- ถ้ามีการจองซ้ำ ให้ log warning ตอน startup (dev-time feedback) แทนที่จะเงียบแล้วมีตัวใดตัวหนึ่งไม่ทำงาน
- ทำให้ในอนาคตทำหน้า "Keybindings" ให้ผู้ใช้ปรับเองได้ฟรีๆ เพราะทุกอย่างผ่าน registry เดียว

### 8.3 `EditorOnly` marker — กัน entity ของ editor เองไม่ให้หลุดเข้า scene ที่ save

`EditorCamera` (ข้อ 7.1), เส้น grid บนพื้น, mesh outline ของ selection ฯลฯ เป็น entity ที่ editor สร้างขึ้นเอง **ต้องไม่ถูกรวมตอน export scene** (phase 6) ไม่งั้นเปิดเกมจริงแล้วจะมีกล้อง/เส้น grid หลุดเข้ามาด้วย แนะนำ:

- component มาตรฐาน `EditorOnly` ใน `bv_editor_core`, ใส่กับทุก entity ที่ editor เองสร้าง (กล้อง editor, gizmo mesh, grid)
- `DynamicSceneBuilder` (phase 6) filter entity ที่มี `EditorOnly` ออกก่อน serialize เสมอ — ระบุเป็นเงื่อนไข **บังคับ** ของ Phase 6 ไม่ใช่ nice-to-have

### 8.4 Asset-reference field ใน Inspector ↔ Asset Browser

field แบบ `Handle<Mesh>`, `Handle<StandardMaterial>` ที่ reflect เห็นเป็นแค่ opaque handle/UUID ไม่สามารถใช้ field editor ทั่วไป (เลข/สตริง) ได้ ต้องมี widget เฉพาะ ("asset picker") ใน `bv_editor_reflect_ui` ที่:

- แสดงชื่อ/thumbnail ของ asset ที่ผูกอยู่ตอนนี้
- รับ drag-and-drop จาก Asset Browser panel (phase 5) มาวางทับเพื่อเปลี่ยน handle
- ลงทะเบียนแบบ extensible เหมือน field editor อื่นๆ (`register_asset_picker::<T: Asset>()`) เผื่อ asset type ของเกมเอง (ไม่ใช่ built-in ของ Bevy) ก็ใช้ระบบเดียวกันได้

ผลคือ Phase 3 (Inspector) กับ Phase 5 (Asset Browser) มีจุดเชื่อมกันชัดเจนที่ต้องออกแบบคู่กัน ไม่ใช่ทำแยกอิสระแล้วมาต่อทีหลัง

### 8.5 Drag-and-drop เป็น framework เดียว ไม่ใช่ต่างคนต่างทำ

Drag asset เข้า viewport (phase 5), ลาก field เข้า inspector (8.4), ลากแถวใน Scene Tree เพื่อ reparent (phase 2) — ทั้งหมดนี้คือ "drag-and-drop" ที่ `bevy_ui` **ไม่มี built-in ให้เช่นกัน** (เหมือน docking ในข้อ 9 ความเสี่ยงเดิม) แนะนำทำ drag-and-drop เป็น sub-framework เล็กๆ ใน `bv_editor_ui` ตัวเดียว (drag source / drop target แบบ generic, ส่ง payload เป็น `Box<dyn Any>` หรือ enum กลาง) แล้วให้ทุก panel ประกอบทับ แทนที่แต่ละ panel จะเขียน mouse-drag state machine ของตัวเองซ้ำๆ

### 8.6 ต้นทุนของ editor ตอนซ่อน (Playing state)

เมื่อ `EditorState::Playing` (UI ซ่อน) system ของ panel/gizmo ต่างๆ (เดิน tree ทุกเฟรม, สแกน reflect ทุก component) ไม่ควรยังรันเปล่าๆ โดยไม่จำเป็น — กำหนด convention ให้ระบบของ editor เองอยู่ใต้ `SystemSet` กลาง (`EditorUiSet`) ที่มี `.run_if(editor_ui_visible)` ครอบอัตโนมัติ เพื่อไม่ให้ editor ที่ compile ติดไปกับ build ทดสอบกลายเป็นภาระ frame time ตอนเล่นจริง

## 9. Roadmap แบ่งเป็น Phase

หลักการ: แต่ละ phase จบด้วย **milestone ที่รันแล้วเห็นผลจริง/เทสได้จริง** ก่อนเริ่ม phase ถัดไป และ built ทับบน `examples/minimal_game` ตลอด (เกมเปล่าที่มี cube/light ไม่กี่ตัวสำหรับทดสอบ)

### Phase 0 — Workspace scaffolding
- ตั้ง Cargo workspace, crate skeleton ทั้งหมดตามข้อ 3 (ยังว่างเปล่า/ยังไม่มี logic)
- ล็อก Bevy version ที่จะใช้ทั้งโปรเจกต์ + เช็กว่า `bevy_core_widgets`/`bevy_feathers` มีจริงในเวอร์ชันนั้นหรือไม่ (กระทบตาราง section 4)
- ทำ `bv_editor_test_utils` (headless app + step + simulate input ตามข้อ 8.1) ให้ใช้งานได้ตั้งแต่ต้น — phase ถัดไปทุกอันอิงเครื่องมือนี้เป็นค่าเริ่มต้นของคำว่า "เทส"
- `EditorPlugin` ว่างเปล่าที่แค่ log ว่าโหลดสำเร็จ
- **เทส:** `cargo run --example minimal_game` เปิดหน้าต่าง Bevy ปกติ ไม่ crash เมื่อเพิ่ม `EditorPlugin` เข้าไป + headless test เรียก `headless_app()` แล้ว `step()` ไม่ panic

### Phase 1 — Shell & Docking framework (`bv_editor_ui`)
- สร้าง layout skeleton บน `bevy_ui`: top toolbar, left panel, center viewport area, right panel, bottom status bar, resizable splitter ระหว่างโซน (ยังไม่มี tab/dock ย้ายที่ได้ก็ได้ในรอบแรก — fixed layout ก่อน)
- panel ยังเป็นกล่องว่างมี title เฉยๆ ให้ตรงผังในภาพตัวอย่าง (Scene Tree ซ้าย, Components ขวา, Project Files ล่าง, จองที่ Console ไว้ล่างสุดตามข้อ 8 ด้วยแม้ยังไม่มีเนื้อหา)
- ทำ `HotkeyRegistry` เวอร์ชันแรก (ข้อ 8.2) ในตอนนี้เลย แม้ยังไม่มี hotkey จริงให้จอง เพื่อบังคับให้ทุก phase ถัดไปจองผ่าน registry ตั้งแต่ต้น ไม่มีใครลัดไปเช็ค `KeyCode` ตรงๆ
- **เทส:** headless test ตรวจ layout tree/ขนาด panel ตาม breakpoint ที่กำหนด + unit test คำนวณ splitter/resize logic ล้วนๆ (ไม่พึ่ง render), เทียบภาพด้วยตาเป็นส่วนเสริมเท่านั้น

### Phase 2 — Scene Tree + Selection + แก้ไข hierarchy พื้นฐาน
- เดิน `Parent`/`Children` + `Name` ของ world จริงสร้างเป็น tree ใน panel ซ้าย, entity ที่มี `EditorOnly` (ข้อ 8.3) ไม่ต้องแสดงใน tree เลยตั้งแต่ phase นี้
- คลิกแถวใน tree → เขียน `Selection` resource, highlight แถวที่เลือก
- ปุ่ม "Add Entity" / เมนูคลิกขวา "Delete" ตามภาพตัวอย่างต้นฉบับ, ลากแถวใน tree เพื่อ reparent (ใช้ drag-and-drop framework กลางตามข้อ 8.5)
- **เทส:** headless test ใส่ entity มีลูกหลาน 2-3 ชั้นใน `minimal_game` แล้วตรวจว่า tree ตรงกับ hierarchy จริง, จำลองคลิก/ลากแล้วตรวจ `Selection`/hierarchy เปลี่ยนตามที่คาด, ตรวจว่า entity ที่มี `EditorOnly` ไม่โผล่ใน tree

### Phase 3 — Inspector ผ่าน `bevy_reflect`
- panel ขวา: list ชื่อ component ของ entity ที่ถูกเลือก (มาจาก `AppTypeRegistry` + entity's `Archetype`)
- generic field editor สำหรับ type พื้นฐาน: `f32`, `bool`, `String`, `Vec3`, `Color`, `Entity` (reference)
- แก้ค่าใน UI แล้วเขียนกลับ component จริงผ่าน reflect
- ยังไม่ต้องทำ asset-reference picker (ข้อ 8.4) ใน phase นี้ — ปล่อย field แบบ `Handle<T>` เป็น read-only placeholder ไปก่อน แล้วมาทำคู่กับ Phase 5
- **เทส:** headless test เลือก entity ที่มี `Transform`, จำลองแก้ `translation.x` ผ่าน inspector แล้วตรวจ component จริงในเวิลด์เปลี่ยนค่าตรงตามที่ set

### Phase 4 — 3D viewport, editor camera & extensible gizmo (ดูรายละเอียดเต็มในข้อ 7)
- ทำ `EditorCamera` เรนเดอร์ลง `RenderTarget::Image` + แปะใน panel กลางด้วย `ImageNode`, เพิ่ม orbit/pan/zoom controller — ใส่ `EditorOnly` (ข้อ 8.3) ให้กล้องนี้ตั้งแต่สร้างเลย เพื่อไม่ให้หลุดเข้า scene ที่ save ใน phase 6
- สร้าง `bv_editor_gizmo_api` (trait `Manipulator`/`ComponentGizmo`, registry, shared ray/plane math helper) ก่อน แล้วค่อย implement `bv_editor_gizmos_builtin` (Translate/Rotate/Scale) บน API นั้น — **ห้าม built-in ลัดเข้า viewport ตรงๆ โดยไม่ผ่าน registry**
- toolbar อ่าน `ManipulatorRegistry` มา render ปุ่ม (ไม่ hardcode Translate/Rotate/Scale)
- ใช้ `bevy_picking`/`Ray3d` หา handle ที่โดนคลิก → `on_drag` คืน delta → ระบบกลางเขียนลง `Transform` ของ selection จริง
- sync สองทาง: ลาก gizmo แล้ว inspector (phase 3) ต้องอัปเดตตามทันที
- **เทส (การทำงาน):** ลาก gizmo แล้วเลข inspector วิ่งตาม, พิมพ์เลขใน inspector แล้ว gizmo ขยับตาม
- **เทส (ความสามารถขยาย):** เขียน manipulator เสริมง่ายๆ (เช่น ตัวแปร Translate ที่ snap 45° แทน grid ปกติ) เป็น plugin แยกนอก `bv_editor_gizmos_builtin` แล้ว `register_manipulator` — ต้องโผล่ในแถบเครื่องมือเองโดยไม่แก้โค้ด core viewport เลย

### Phase 5 — Project Files / Asset Browser panel
- panel ล่าง: อ่านโฟลเดอร์ assets จริงบน disk (breadcrumb, folder, ไฟล์ ตามภาพตัวอย่าง), navigate เข้า-ออกโฟลเดอร์
- แสดง thumbnail พื้นฐานสำหรับไฟล์ภาพ (โหลดผ่าน `AssetServer`)
- ทำ asset-reference picker widget (ข้อ 8.4) ในรอบนี้ — เชื่อม drag จาก panel นี้เข้ากับ field แบบ `Handle<T>` ใน Inspector (phase 3) ที่ปล่อย placeholder ไว้ก่อนหน้านี้
- **เทส:** ชี้ไปที่ `examples/minimal_game/assets`, ตรวจ breadcrumb/navigation ถูกต้อง, thumbnail ของไฟล์ png ขึ้นจริง, ลาก mesh asset ไปวางบน field ใน inspector แล้ว `Handle` ของ component เปลี่ยนค่าตรงตามไฟล์ที่ลาก

### Phase 6 — Scene Save/Load + Project manifest
- Export world (หรือ subtree ที่เลือก) เป็น `DynamicScene` → RON ผ่าน `bevy_scene` — **filter entity ที่มี `EditorOnly` (ข้อ 8.3) ออกก่อนเสมอ เป็นเงื่อนไขบังคับของ phase นี้**
- Import กลับ ผูกกับ Scene Tree (phase 2) ให้ tree รีเฟรชหลังโหลด
- นิยาม `project.json` (list ของ scene file, asset root, ค่า config) ด้วย `serde_json`
- **เทส:** สร้าง scene ใน editor → save → ปิดโปรแกรม → เปิดใหม่ → load → hierarchy/component ค่าตรงเดิมทุก field, ตรวจว่าไฟล์ RON ที่ save ออกมาไม่มี `EditorCamera`/entity ที่มี `EditorOnly` ปนอยู่เลย

### Phase 7 — Extension API (`bv_editor_extension`) + dogfooding
- Finalize `EditorExtension` trait, `EditorAppExt` (`register_editor_panel`, `register_menu_item`, `register_inspector_widget`)
- **Refactor panel ที่ทำใน phase 2/3/5 ให้กลายเป็น extension ที่ผ่าน API นี้เท่านั้น** (ห้ามมี backdoor เฉพาะ built-in)
- เขียน `examples/third_party_extension` เป็น crate แยกจริงๆ (เช่น panel "Notes" ง่ายๆ) จำลองนักพัฒนาภายนอก
- **เทส:** `minimal_game` เพิ่ม `add_plugins((EditorPlugin, ThirdPartyExtensionPlugin))` แล้ว panel ใหม่โผล่ โดยไม่แก้ core crate ใดๆ เลย

### Phase 8 — Undo/Redo
- `Command` trait + `UndoStack` resource, ห่อ operation หลักทั้งหมดจาก phase 3/4/6 (แก้ field, ลบ entity, เพิ่ม/ลบ component, load scene)
- คีย์ลัด Ctrl+Z / Ctrl+Y จองผ่าน `HotkeyRegistry` (ข้อ 8.2) เหมือนฟีเจอร์อื่น ไม่เช็ค `KeyCode` ตรงๆ
- **เทส:** headless test ทำชุด edit หลายแบบ → undo กลับทีละก้าว → เทียบ snapshot ของ world กับค่าที่คาดไว้ทุกก้าว รวมเคส undo การลบ entity ที่มีอะไรอ้างอิงถึงมันอยู่ (เช่น `Parent` ของลูก) ว่า restore กลับมาถูกความสัมพันธ์เดิม

### Phase 9 — Play/Pause state + Embedding polish
- ใช้ `EditorState` (Editing/Playing/Paused) ตามข้อ 2, ปุ่ม/คีย์ลัด toggle
- กำหนด convention ให้เกม host ใส่ system เกมของตัวเองใน `run_if(in_state(EditorState::Playing))`
- ทำ feature flag `editor` ที่ปิดได้ทั้งหมดสำหรับ release build (`#[cfg(feature = "editor")]` รอบจุดที่ add_plugins)
- **เทส:** เอา `EditorPlugin` ไปเสียบกับเกมตัวอย่าง "คนละโปรเจกต์" ที่ไม่ได้เขียนเพื่อรองรับ editor มาก่อน โดยแก้แค่บรรทัดเดียว (`add_plugins`) แล้วใช้งานได้ครบ Scene Tree/Inspector/Gizmo/Play-Pause

### Phase 10 — Materials / Resources / Systems tab + Polish
- Materials tab: list `StandardMaterial` (หรือ material handle ที่ entity ใช้) แก้ผ่าน reflect เหมือน component
- Resources tab: list global ECS `Resource` ที่ derive `Reflect` แล้วแก้ค่าได้
- Systems tab: แสดงรายชื่อ system ที่ลงทะเบียนไว้ — **ระบุไว้ชัดว่าเป็น best-effort/stretch** เพราะ Bevy ยังไม่มี reflection API มาตรฐานสำหรับ schedule graph เต็มรูปแบบ ต้องสำรวจ `bevy_ecs::schedule` internals ก่อนว่าดึงข้อมูลระดับไหนได้จริง
- ปรับ docking framework ให้ลาก tab ย้ายตำแหน่งได้ (ถ้ายังไม่ทำใน phase 1), เก็บ layout ลง `editor_layout.ron` ต่อโปรเจกต์
- **เทส:** ครบ tab ตามภาพตัวอย่างต้นฉบับ, layout จำตำแหน่งข้ามการเปิดโปรแกรมใหม่ได้

## 10. ความเสี่ยงที่ต้องจับตา

- **bevy_ui ไม่มี docking/tree/property widget สำเร็จรูป** → phase 1 คือส่วนที่ใช้เวลานานสุดและเสี่ยงบานปลายสุด ควรทำ scope ให้เล็กที่สุดก่อน (fixed layout ไม่มี drag-to-dock) แล้วค่อยเพิ่มใน phase 10
- **`bevy_ui` ไม่มี generic drag-and-drop เช่นกัน** (ข้อ 8.5) — เป็นต้นทุนแฝงอีกก้อนที่แยกจาก docking แต่ประเภทความเสี่ยงเดียวกัน (custom framework ที่ไม่มีของสำเร็จรูปให้ใช้) ต้องกันเวลาไว้ตั้งแต่ phase 2 ไม่ใช่คิดว่าเป็นของเล็กที่ทำตอนไหนก็ได้
- **`bevy_core_widgets`/`bevy_feathers`** อาจยังไม่เสถียรพอในเวอร์ชัน Bevy ที่ล็อกไว้ — phase 0 ต้องตรวจสอบก่อนวางแผนละเอียดของ phase 1
- **Systems tab** (phase 10) พึ่ง internal ของ `bevy_ecs::schedule` ที่ไม่ใช่ public reflection API มาตรฐาน — อาจต้องลดสโคปเหลือแค่ list ชื่อ system แบบ manual registration แทนการ introspect อัตโนมัติ
- Static-plugin extension model แปลว่า **ไม่มี hot-reload extension** — เพิ่ม/แก้ extension ต้อง recompile ทุกครั้ง (ยอมรับแล้วตามคำตอบข้อ 2)
- **Render-to-texture สำหรับ viewport (ข้อ 7.1)** เพิ่มต้นทุนเรนเดอร์ (กล้อง editor เรนเดอร์ซ้ำนอกเหนือจากกล้องเกม) และต้อง handle resize ของ `Image` ให้ตรงกับขนาด panel ทุกเฟรมที่ panel เปลี่ยนขนาด — ต้องทำ benchmark ใน phase 4 ว่ากระทบ frame time แค่ไหนบนเกมที่หนักจริง
- **ความแม่นยำของ hit-test gizmo** (ray กับเส้น/วงแหวนบางๆ) ต้องมี "fudge factor" ระยะรัศมีรับคลิกที่ไม่ผูกกับขนาดพิกเซลจอ (screen-space constant, ไม่ใช่ world-space constant) ไม่งั้น handle จะเล็ก/ใหญ่ผิดตาเมื่อซูมกล้องเข้าออก — ต้องระบุเป็นเงื่อนไขทดสอบใน phase 4 ไม่ใช่ปล่อยผ่าน "ดูด้วยตา" อย่างเดียว
- **ลืมใส่ `EditorOnly`** ให้ entity ของ editor เองสักตัว (ข้อ 8.3) แล้วมันหลุดเข้า scene ที่ save เป็น regression ที่เงียบและอันตราย — ควรมี assertion/test อัตโนมัติเช็คไฟล์ RON ที่ save ออกมาทุกครั้งใน CI ไม่ใช่พึ่งคนตรวจด้วยตา (ผูกกับ headless test harness ข้อ 8.1)

## 11. Definition of Done ของทั้งโปรเจกต์ (เทียบกับ requirement เดิม)

- [ ] เอาไป `add_plugins(EditorPlugin)` เข้าเกมอื่นได้จริงโดยไม่ต้องแก้โค้ดเกม (ยืนยันใน Phase 9)
- [ ] เพิ่ม panel/เครื่องมือใหม่ได้โดยไม่แตะ core crate (ยืนยันใน Phase 7)
- [ ] เพิ่ม manipulator/component gizmo ใหม่ได้โดยไม่แตะ `bv_editor_gizmo_api`/`bv_editor_viewport` (ยืนยันใน Phase 4, ดูข้อ 7)
- [ ] UI หน้าตา/ฟีเจอร์ครบตามภาพตัวอย่าง (Scene Tree, Components, Materials, Resources, Systems, Project Files, viewport gizmo) (ยืนยันใน Phase 10)
- [ ] ใช้ built-in lib ของ Bevy เป็นหลัก, dependency นอกวงมีเท่าที่จำเป็นและระบุเหตุผลชัดเจน (ตาราง section 4)
- [ ] ทุก phase (0–10) มี headless automated test ผ่าน `bv_editor_test_utils` อย่างน้อย 1 เคสต่อ milestone หลัก ไม่ใช่ "ดูด้วยตา" ล้วนๆ (ยืนยันตั้งแต่ Phase 0, ดูข้อ 8.1)
- [ ] scene ที่ save ออกมาไม่มี entity ที่มี `EditorOnly` ปนอยู่เลยไม่ว่ากรณีใด (ยืนยันใน Phase 6, ดูข้อ 8.3)
