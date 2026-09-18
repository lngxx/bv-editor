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

## 3. โครงสร้าง Workspace / Crate

ใช้ Cargo workspace แยก crate ตามความรับผิดชอบ เพื่อให้ extension ผูกเฉพาะส่วนที่ต้องใช้ และแยก compile time:

```
bv-editor/
├── Cargo.toml                (workspace)
├── crates/
│   ├── bv_editor_core/        # state, schedule labels, EditorPlugin entrypoint, Selection resource
│   ├── bv_editor_ui/          # docking framework, splitter, tab bar, tree widget, property grid (bevy_ui)
│   ├── bv_editor_reflect_ui/  # generic field editor จาก bevy_reflect + TypeRegistry
│   ├── bv_editor_extension/   # trait EditorExtension, EditorAppExt (register_panel ฯลฯ), registry Resource
│   ├── bv_editor_undo/        # Command trait, UndoStack resource
│   ├── bv_editor_scene_panel/     # built-in: Scene Tree
│   ├── bv_editor_inspector_panel/ # built-in: Components/Materials/Resources tab
│   ├── bv_editor_assets_panel/    # built-in: Project Files browser
│   ├── bv_editor_viewport/        # built-in: gizmo, picking, selection outline
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

| ความต้องการ | เลือกใช้ | หมายเหตุ |
|---|---|---|
| UI layout/rendering | `bevy_ui` | ตามที่เลือก — native, ไม่มี docking/tree/property widget สำเร็จรูป ต้องสร้างเอง (phase 1) |
| Widget interaction พื้นฐาน (button, slider, checkbox state) | `bevy_core_widgets` / `bevy_feathers` (ถ้ามีใน Bevy เวอร์ชันที่ใช้) | Bevy กำลังพัฒนา headless widget set สำหรับงานสาย tool/editor โดยเฉพาะ — **ต้องเช็กว่ามีจริงและ API หน้าตาแบบไหนใน Bevy version ที่ล็อกไว้ตอนเริ่ม Phase 0** ถ้ายังไม่พร้อมค่อย fallback เขียน state machine เองบน `Interaction` component ปกติ |
| Reflection สำหรับ inspector อัตโนมัติ | `bevy_reflect` + `AppTypeRegistry` | หัวใจของ inspector — component ไหน derive `Reflect` แล้วจะโผล่ให้แก้ได้อัตโนมัติโดยไม่ต้องเขียน UI เฉพาะทีละ type |
| Selection ใน viewport | `bevy_picking` (mesh picking backend) | built-in ใน Bevy แล้ว ไม่ต้องพึ่ง `bevy_mod_picking` ภายนอก |
| Gizmo วาดเส้น/ลูกศร transform handle | `bevy_gizmos` | built-in, ใช้วาด outline ของ selection และแกน translate/rotate/scale |
| Scene hierarchy | `Parent`/`Children` (bevy_ecs) ปกติของเกม | scene tree panel แค่ "อ่าน" ECS จริง ไม่มี hierarchy คู่ขนานของตัวเอง |
| Save/Load scene | `bevy_scene` (`DynamicScene`) + RON | serialization format ใช้ของ Bevy เอง |
| Asset listing | `bevy_asset::AssetServer` + `std::fs` | project files panel อ่านโฟลเดอร์จริงบน disk |
| Project manifest (`project.json`) | `serde` + `serde_json` | เก็บ list scene, path assets, settings — ไม่ใช่ ECS scene จึงไม่ใช้ bevy_scene ตรงนี้ |
| Native "Open Folder" dialog (ถ้าต้องการ) | *ยังไม่ใช้* — ใช้ file browser ในตัว editor เอง (phase 5) แทน | เพื่อคง "built-in only" ไว้ได้จริง ถ้าอนาคตอยากได้ native dialog ค่อยพิจารณา `rfd` เป็น dependency นอกวงเดียวที่ยอมรับ |
| Undo/Redo | เขียนเอง (`Command` trait + `Vec<Box<dyn Command>>`) | ไม่มี built-in ของ Bevy โดยตรง แต่ implement บน `World`/Commands ปกติได้ |

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

## 7. Roadmap แบ่งเป็น Phase

หลักการ: แต่ละ phase จบด้วย **milestone ที่รันแล้วเห็นผลจริง/เทสได้จริง** ก่อนเริ่ม phase ถัดไป และ built ทับบน `examples/minimal_game` ตลอด (เกมเปล่าที่มี cube/light ไม่กี่ตัวสำหรับทดสอบ)

### Phase 0 — Workspace scaffolding
- ตั้ง Cargo workspace, crate skeleton ทั้งหมดตามข้อ 3 (ยังว่างเปล่า/ยังไม่มี logic)
- ล็อก Bevy version ที่จะใช้ทั้งโปรเจกต์ + เช็กว่า `bevy_core_widgets`/`bevy_feathers` มีจริงในเวอร์ชันนั้นหรือไม่ (กระทบตาราง section 4)
- `EditorPlugin` ว่างเปล่าที่แค่ log ว่าโหลดสำเร็จ
- **เทส:** `cargo run --example minimal_game` เปิดหน้าต่าง Bevy ปกติ ไม่ crash เมื่อเพิ่ม `EditorPlugin` เข้าไป

### Phase 1 — Shell & Docking framework (`bv_editor_ui`)
- สร้าง layout skeleton บน `bevy_ui`: top toolbar, left panel, center viewport area, right panel, bottom status bar, resizable splitter ระหว่างโซน (ยังไม่มี tab/dock ย้ายที่ได้ก็ได้ในรอบแรก — fixed layout ก่อน)
- panel ยังเป็นกล่องว่างมี title เฉยๆ ให้ตรงผังในภาพตัวอย่าง (Scene Tree ซ้าย, Components ขวา, Project Files ล่าง)
- **เทส:** เปิดแล้วเทียบ layout กับภาพตัวอย่างด้วยตา + unit test คำนวณ splitter/resize logic ล้วนๆ (ไม่พึ่ง render)

### Phase 2 — Scene Tree (read-only) + Selection
- เดิน `Parent`/`Children` + `Name` ของ world จริงสร้างเป็น tree ใน panel ซ้าย
- คลิกแถวใน tree → เขียน `Selection` resource, highlight แถวที่เลือก
- **เทส:** ใส่ entity มีลูกหลาน 2-3 ชั้นใน `minimal_game` แล้วตรวจว่า tree ตรงกับ hierarchy จริง, คลิกแล้ว resource เปลี่ยนค่าถูกต้อง

### Phase 3 — Inspector ผ่าน `bevy_reflect`
- panel ขวา: list ชื่อ component ของ entity ที่ถูกเลือก (มาจาก `AppTypeRegistry` + entity's `Archetype`)
- generic field editor สำหรับ type พื้นฐาน: `f32`, `bool`, `String`, `Vec3`, `Color`, `Entity` (reference)
- แก้ค่าใน UI แล้วเขียนกลับ component จริงผ่าน reflect
- **เทส:** เลือก entity ที่มี `Transform`, แก้ `translation.x` ใน inspector แล้วเห็น object ขยับในหน้าต่างเกม (ยังไม่มี gizmo ก็ยืนยันด้วยตาตรงๆ ได้)

### Phase 4 — Viewport gizmo & manipulation
- ใช้ `bevy_gizmos` วาด outline รอบ selection + แกน translate (ตามภาพตัวอย่างที่มีลูกศรสี)
- ใช้ `bevy_picking` ต่อ drag บนแกน → คำนวณ delta แล้วแก้ `Transform` ของ selection
- sync สองทาง: ลาก gizmo แล้ว inspector (phase 3) ต้องอัปเดตตามทันที
- **เทส:** ลาก gizmo แล้วเลข inspector วิ่งตาม, พิมพ์เลขใน inspector แล้ว gizmo ขยับตาม

### Phase 5 — Project Files / Asset Browser panel
- panel ล่าง: อ่านโฟลเดอร์ assets จริงบน disk (breadcrumb, folder, ไฟล์ ตามภาพตัวอย่าง), navigate เข้า-ออกโฟลเดอร์
- แสดง thumbnail พื้นฐานสำหรับไฟล์ภาพ (โหลดผ่าน `AssetServer`)
- **เทส:** ชี้ไปที่ `examples/minimal_game/assets`, ตรวจ breadcrumb/navigation ถูกต้อง, thumbnail ของไฟล์ png ขึ้นจริง

### Phase 6 — Scene Save/Load + Project manifest
- Export world (หรือ subtree ที่เลือก) เป็น `DynamicScene` → RON ผ่าน `bevy_scene`
- Import กลับ ผูกกับ Scene Tree (phase 2) ให้ tree รีเฟรชหลังโหลด
- นิยาม `project.json` (list ของ scene file, asset root, ค่า config) ด้วย `serde_json`
- **เทส:** สร้าง scene ใน editor → save → ปิดโปรแกรม → เปิดใหม่ → load → hierarchy/component ค่าตรงเดิมทุก field

### Phase 7 — Extension API (`bv_editor_extension`) + dogfooding
- Finalize `EditorExtension` trait, `EditorAppExt` (`register_editor_panel`, `register_menu_item`, `register_inspector_widget`)
- **Refactor panel ที่ทำใน phase 2/3/5 ให้กลายเป็น extension ที่ผ่าน API นี้เท่านั้น** (ห้ามมี backdoor เฉพาะ built-in)
- เขียน `examples/third_party_extension` เป็น crate แยกจริงๆ (เช่น panel "Notes" ง่ายๆ) จำลองนักพัฒนาภายนอก
- **เทส:** `minimal_game` เพิ่ม `add_plugins((EditorPlugin, ThirdPartyExtensionPlugin))` แล้ว panel ใหม่โผล่ โดยไม่แก้ core crate ใดๆ เลย

### Phase 8 — Undo/Redo
- `Command` trait + `UndoStack` resource, ห่อ operation หลักทั้งหมดจาก phase 3/4/6 (แก้ field, ลบ entity, เพิ่ม/ลบ component, load scene)
- คีย์ลัด Ctrl+Z / Ctrl+Y
- **เทส:** ทำชุด edit หลายแบบ → undo กลับทีละก้าว → เทียบ snapshot ของ world กับค่าที่คาดไว้ทุกก้าว

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

## 8. ความเสี่ยงที่ต้องจับตา

- **bevy_ui ไม่มี docking/tree/property widget สำเร็จรูป** → phase 1 คือส่วนที่ใช้เวลานานสุดและเสี่ยงบานปลายสุด ควรทำ scope ให้เล็กที่สุดก่อน (fixed layout ไม่มี drag-to-dock) แล้วค่อยเพิ่มใน phase 10
- **`bevy_core_widgets`/`bevy_feathers`** อาจยังไม่เสถียรพอในเวอร์ชัน Bevy ที่ล็อกไว้ — phase 0 ต้องตรวจสอบก่อนวางแผนละเอียดของ phase 1
- **Systems tab** (phase 10) พึ่ง internal ของ `bevy_ecs::schedule` ที่ไม่ใช่ public reflection API มาตรฐาน — อาจต้องลดสโคปเหลือแค่ list ชื่อ system แบบ manual registration แทนการ introspect อัตโนมัติ
- Static-plugin extension model แปลว่า **ไม่มี hot-reload extension** — เพิ่ม/แก้ extension ต้อง recompile ทุกครั้ง (ยอมรับแล้วตามคำตอบข้อ 2)

## 9. Definition of Done ของทั้งโปรเจกต์ (เทียบกับ requirement เดิม)

- [ ] เอาไป `add_plugins(EditorPlugin)` เข้าเกมอื่นได้จริงโดยไม่ต้องแก้โค้ดเกม (ยืนยันใน Phase 9)
- [ ] เพิ่ม panel/เครื่องมือใหม่ได้โดยไม่แตะ core crate (ยืนยันใน Phase 7)
- [ ] UI หน้าตา/ฟีเจอร์ครบตามภาพตัวอย่าง (Scene Tree, Components, Materials, Resources, Systems, Project Files, viewport gizmo) (ยืนยันใน Phase 10)
- [ ] ใช้ built-in lib ของ Bevy เป็นหลัก, dependency นอกวงมีเท่าที่จำเป็นและระบุเหตุผลชัดเจน (ตาราง section 4)
