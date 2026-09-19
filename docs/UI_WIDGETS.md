# bv-editor — รายการ UI widget เฉพาะของโปรเจกต์

สถานะ: อัปเดตตามโค้ดจริง ณ วันที่ 2026-09-19
ไม่รวม widget มาตรฐานของ `bevy_ui`/`bevy_ui_widgets` (เช่น `Node`, `Text`, ปุ่มที่ยังไม่มี state พิเศษ) — เอกสารนี้ลิสต์เฉพาะสิ่งที่ bv-editor **สร้างเอง** เพราะ `bevy_ui` ไม่มีสำเร็จรูปให้ (ดูเหตุผลใน [`docs/DESIGN.md`](./DESIGN.md) section 4/10)

ดูตัวอย่างที่รันได้จริงของ widget เกือบทั้งหมดในลิสต์นี้ได้ที่ `examples/ui_gallery` (`cargo run -p ui_gallery`) — สเปกของฟีเจอร์ที่ยัง "ต้องทำ" อยู่ใน [`docs/UI_FEATURES.md`](./UI_FEATURES.md)

| Widget | อยู่ใน crate/ไฟล์ | ทำอะไร | สถานะ |
|---|---|---|---|
| **Splitter** | [`bv_editor_ui::splitter`](../crates/bv_editor_ui/src/splitter.rs) | แถบบางๆ ลากด้วยเมาส์ซ้ายค้างเพื่อ resize panel ข้างเคียง (`Horizontal` = ปรับ width, `Vertical` = ปรับ height) มี min/max px กันลากเกิน, hover/ลากแล้วเปลี่ยน mouse cursor เป็นลูกศร resize (`↔`/`↕` ตามแกน) ผ่าน `bevy_window::CursorIcon` บน primary window | ใช้งานได้ (F4 ✅ รวม cursor feedback) |
| **Panel min/max size** | [`bv_editor_ui::shell`](../crates/bv_editor_ui/src/shell.rs) | `Node.min_width`/`max_width`/`min_height`/`max_height` จริงบนทุก panel (Scene Tree, Components, viewport, bottom row, Project Files, Console) — บังคับโดย flexbox layout engine เองทุกเฟรม ไม่ใช่แค่ตอนลาก splitter (ทนต่อ resize หน้าต่างด้วย) ไม่ใช่ widget แยก แค่ config บน `Node` ที่มีอยู่แล้ว | ใช้งานได้ (F7 ✅) |
| **Scrollbar** | [`bv_editor_ui::scrollbar`](../crates/bv_editor_ui/src/scrollbar.rs) | `ScrollbarThumb { target, axis }` ลากด้วยเมาส์ (แนวตั้งหรือแนวนอนตาม `axis`) + wheel scroll (แนวตั้งอย่างเดียว) เมื่อ hover container ที่ตั้ง `Overflow` แกนนั้นเป็น scroll ไว้ — ซ่อนทั้ง track และ thumb อัตโนมัติเมื่อเนื้อหาพอดีพื้นที่, hover/ลากเปลี่ยน mouse cursor เป็น grab/grabbing ผ่าน `bevy_window::CursorIcon`, track หนา 12px ไม่ใช้ `bevy_ui_widgets::scrollbar` (ของจริงมีในเวอร์ชันนี้ แต่สร้างบน `bevy_picking`, คนละ paradigm กับ widget อื่นในนี้) | ใช้งานได้ (F2/F3 ✅) — Scene Tree แนวตั้งอย่างเดียว, Components panel มีทั้งแนวตั้ง+แนวนอน |
| **Drag-and-drop framework** | [`bv_editor_ui::dnd`](../crates/bv_editor_ui/src/dnd.rs) | `DragSource`/`DropTarget`/`DragState`/`DragDropped` — กรอบกลางสำหรับ "หยิบของ (payload) มาลากไปปล่อยบนเป้าหมาย" ปัจจุบันมี payload แบบเดียวคือ `DragPayload::Entity` | ใช้งานได้ (ผู้บริโภครายแรก: Scene Tree reparent) |
| **Scene Tree row** | [`bv_editor_scene_panel`](../crates/bv_editor_scene_panel/src/lib.rs) | แถวหนึ่งของ hierarchy — คลิกเพื่อเลือก (`Selection`), เป็นทั้ง `DragSource` และ `DropTarget` ในตัว (ลากไปปล่อยแถวอื่นเพื่อ reparent), เยื้องซ้ายตาม depth | ใช้งานได้ + highlight (F1 ✅) |
| **Row hover/selection highlight** | [`bv_editor_scene_panel::row_background`](../crates/bv_editor_scene_panel/src/lib.rs) | pure function ให้สีพื้นหลังแถวตามลำดับ `selected` > `hover` > ปกติ, sync ทุกเฟรมจาก `Interaction`+`Selection` จริง | ใช้งานได้ (F1 ✅) |
| **Scene Tree toolbar button** | [`bv_editor_scene_panel::button`](../crates/bv_editor_scene_panel/src/lib.rs) | ปุ่มข้อความมี background กดได้ ("+ Add Entity", "Delete") — ปุ่มธรรมดาที่ผูก `Interaction` เอง ไม่ใช่ widget จาก `bevy_ui_widgets` | ใช้งานได้ |
| **Reflect field editor (leaf)** | [`bv_editor_reflect_ui::spawn_component_fields`](../crates/bv_editor_reflect_ui/src/lib.rs) | แถวเดียวต่อ field หนึ่งของ component, "คลิกเพื่อโฟกัส แล้วพิมพ์ทับ" (ไม่ prefill ค่าเดิม), Enter commit, Escape cancel — เขียนกลับ component จริงผ่าน `bevy_reflect` | ใช้งานได้ รองรับ: |
| — ค่า `f32` | เหมือนบรรทัดบน | กล่องตัวเลข พิมพ์เลข/`-`/`.` | ✅ |
| — ค่า `bool` | เหมือนบรรทัดบน | คลิกแล้ว toggle ทันที ไม่ต้องพิมพ์/Enter | ✅ |
| — ค่า `String` | เหมือนบรรทัดบน | พิมพ์ทับได้ (ตัวพิมพ์เล็กเท่านั้น ยังไม่รองรับ shift) | ✅ |
| — ค่า `Vec3` | เหมือนบรรทัดบน | ขยายเป็น 3 กล่อง `f32` ย่อย (x/y/z) ต่อแถว | ✅ |
| — ค่า `Color` | เหมือนบรรทัดบน | ขยายเป็น 4 กล่อง `f32` ย่อย (r/g/b/a), round-trip ผ่าน `Srgba` | ✅ |
| — ชนิดอื่นที่ยังไม่มี editor | เหมือนบรรทัดบน | แสดงเป็นตัวหนังสือ read-only `{:?}` (สีจาง, คลิกไม่ได้) แทนการซ่อนทิ้ง | fallback เจตนา ไม่ใช่ของค้าง |
| **Editor shell (panel shell)** | [`bv_editor_ui::shell`](../crates/bv_editor_ui/src/shell.rs) | fixed layout: toolbar บน, Scene Tree/Viewport/Components แถวกลาง, Project Files/Console แถวล่าง, status bar ล่างสุด — panel แต่ละกล่องมี `*Slot` marker ให้ panel อื่น mount เนื้อหาเข้าไป | ใช้งานได้ (fixed, ยังไม่ dock/tab — ดู F5) |
| **Panel title label** | เหมือนบรรทัดบน (`title_text`) | ข้อความหัวกล่อง panel — ยังเป็นแค่ label ลอย ไม่ใช่ title bar ที่ลากได้ | placeholder จนกว่าจะถึง F5 |

## ยังไม่มี (ตามสเปกใน `docs/UI_FEATURES.md`)

- **Docking/tab merge ของ side panel** (F5) — ลากรวม panel เป็น tab group ยังทำไม่ได้ (`splitter.rs` ทำได้แค่ resize)
- **Icon** (F6) — ไม่มี icon asset/registry ในโปรเจกต์เลยตอนนี้ ทุกอย่างเป็นตัวหนังสือล้วน

## กติกาการอัปเดตเอกสารนี้

เพิ่ม/แก้แถวในตารางนี้ทุกครั้งที่ widget เฉพาะของ bv-editor ตัวใหม่ถูก implement จริง (ไม่ใช่แค่ตอนเขียนสเปกใน `UI_FEATURES.md`) เพื่อให้เอกสารนี้สะท้อนโค้ดปัจจุบันเสมอ ไม่ใช่แผนที่ยังไม่ได้ทำ
