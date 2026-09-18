# bv-editor — ฟีเจอร์ UI เพิ่มเติม (v0.2 draft)

สถานะ: ฉบับร่างสำหรับคุยกัน — **ยังไม่มีโค้ด implementation ใดๆ** (ตามที่ขอ — เขียนสเปกให้ตกลงกันก่อน แล้วค่อย implement เป็น phase ทีละอัน)
วันที่: 2026-09-18
อ้างอิง: [`docs/DESIGN.md`](./DESIGN.md) (สถาปัตยกรรมหลัก, roadmap phase 0–10) — เอกสารนี้เป็นสเปกละเอียดของฟีเจอร์ UI 6 ข้อที่ถูกขอเพิ่ม เสริมจากของที่ระบุไว้ในนั้นแล้ว

รายการที่ขอ (สรุปจากคำขอเดิม):

1. Scene Tree ต้อง highlight แถวที่เลือกได้
2. Scene Tree ถ้ารายการยาวเกินพื้นที่ ต้องมี scrollbar
3. Components (Inspector) panel ถ้ามี field/component เยอะ ต้องมี scrollbar เหมือนกัน
4. Side panel ต้องลากปรับขนาดด้วยเมาส์ได้
5. Side panel ต้องลาก-วาง (drag & drop) เพื่อรวม panel เข้าด้วยกันได้ (docking)
6. ถ้าต้องใช้ icon ต้องมีทางเพิ่ม icon มาใช้ใน UI ได้

ก่อนเขียนสเปก ได้ไล่โค้ดปัจจุบันดูแล้วว่าอะไรมีอยู่แล้ว/อะไรยังไม่มี สรุปสถานะปัจจุบันต่อข้อไว้ในแต่ละหัวข้อด้านล่าง เพื่อไม่ให้สเปกซ้ำของที่มีอยู่แล้วโดยไม่รู้ตัว

**เพิ่มภายหลัง (2026-09-18):**

7. Panel ต้องมี min/max size ที่ "ยึดอยู่จริง" ไม่ใช่แค่ตอนลาก splitter — ดู F7

---

## F1 — Scene Tree: Row highlight ✅ implemented

**สถานะ:** ทำเสร็จแล้ว — [`bv_editor_scene_panel/src/lib.rs`](../crates/bv_editor_scene_panel/src/lib.rs):
- `row_background(selected, hovered) -> Color` เป็น pure function เล็กๆ (ไม่พึ่ง ECS) คืนสีตามลำดับความสำคัญ `selected` > `hover` > ปกติ (`ROW_SELECTED_BACKGROUND` > `ROW_HOVER_BACKGROUND` > `ROW_BACKGROUND`) มี unit test ตรงๆ ของ priority นี้แยกจาก ECS (`row_background_priority_selected_beats_hovered_beats_normal`)
- `sync_row_highlight` เป็น system ที่รันทุกเฟรม (ไม่ผูกกับ `SceneTreeDirty`) อ่าน `Interaction` ของแต่ละแถว (นับ `Hovered`/`Pressed` เป็น hover) + `Selection` แล้วเขียน `BackgroundColor` — hover จึงตามเคอร์เซอร์ได้ทุกเฟรมแม้ tree ไม่ได้ rebuild, และแถวที่เพิ่ง rebuild ใหม่ไม่มีสีค้าง เพราะ `sync_row_highlight` รันต่อท้าย `rebuild_scene_tree_ui` ใน chain เดียวกันเสมอ
- multi-select highlight ได้ฟรีอยู่แล้ว เพราะ `selection.contains(entity)` เช็คทุก entity ใน `Selection` ไม่ใช่แค่ primary
- เทสครอบคลุม: hover แถวที่ไม่ได้เลือกแล้วเปลี่ยนสี + เอาเมาส์ออกแล้วสีกลับปกติ, และแถวที่เลือกอยู่สีไม่เปลี่ยนแม้ hover ทับ (`hovering_an_unselected_row_highlights_it`, `selected_row_stays_selected_color_even_when_hovered`)

---

## F2 — Scene Tree: Scrollbar เมื่อรายการยาวเกิน ✅ implemented

**สถานะ:** ทำเสร็จแล้ว — widget กลางใน [`bv_editor_ui::scrollbar`](../crates/bv_editor_ui/src/scrollbar.rs) + ต่อเข้ากับ Scene Tree ใน [`bv_editor_scene_panel/src/lib.rs`](../crates/bv_editor_scene_panel/src/lib.rs):
- ใช้ `Overflow::scroll_y()` + `ScrollPosition` ของ `bevy_ui` ตรงๆ สำหรับ clip/content-window (built-in, ไม่เขียนเอง) — `SceneTreeRowsContainer` ได้ `flex_grow: 1.0` + `min_height: Val::Px(0.0)` ใหม่ เพื่อให้มันถูก "บีบ" ให้เต็มพื้นที่ที่เหลือของ panel แทนที่จะโตตามเนื้อหา (ไม่งั้น overflow ไม่มีอะไรให้ clip)
- `ScrollbarThumb`/`scrollbar_drag_system`/`wheel_scroll_system`/`sync_scrollbar_thumb_system` เขียนเองบน `Interaction`+`ButtonInput`+`AccumulatedMouseMotion`/`AccumulatedMouseScroll` (**ไม่ใช้** `bevy_ui_widgets::scrollbar` ที่มีมาให้ในเวอร์ชันนี้จริง เพราะมันสร้างบน `bevy_picking`'s pointer events คนละ paradigm กับ splitter/dnd ที่มีอยู่แล้ว และใช้ไม่ได้ใน headless test ของโปรเจกต์นี้โดยไม่มี picking backend — ดูเหตุผลเต็มในโค้ดคอมเมนต์ของ `scrollbar.rs`)
- thumb ซ่อนอัตโนมัติ (`Display::None`) เมื่อเนื้อหาพอดีกับพื้นที่ ไม่โชว์ thumb เกะกะแบบที่สเปกขอ
- scroll position "คงอยู่" ข้าม tree rebuild ได้ฟรีอยู่แล้ว เพราะ `ScrollPosition` อยู่บน container entity ที่ไม่เคยถูก despawn (`rebuild_scene_tree_ui` despawn เฉพาะแถว ไม่แตะ container)
- เทสครอบคลุมทั้ง pure math (`thumb_geometry`/`clamp_scroll`) และ ECS-level (wheel scroll เฉพาะ container ที่ hover, ลาก thumb แล้ว target scroll ตาม, thumb ซ่อน/โผล่ตามเนื้อหา) รวม 12 เทสใหม่ใน `bv_editor_ui` + 1 เทส integration ใน `bv_editor_scene_panel` ที่ยืนยันว่า thumb ที่ spawn จริงชี้ไปที่ `SceneTreeRowsContainer` ที่ถูกต้อง

---

## F3 — Inspector (Components) panel: Scrollbar เมื่อ field/component เยอะ

**สถานะปัจจุบัน:** ไม่มีเช่นกัน — [`bv_editor_inspector_panel/src/lib.rs`](../crates/bv_editor_inspector_panel/src/lib.rs) เดิน component ของ entity ที่เลือกแล้วเรนเดอร์ field เรียงต่อกันลง panel เดียว entity ที่มีหลาย component ที่มี field เยอะๆ (เช่น `Transform` + `PointLight` + custom component หลายตัว) จะล้นจอเหมือน F2

**สเปก:** เหมือน F2 ทุกข้อ (clip + wheel scroll + draggable thumb + คง scroll position เวลาสลับ field แต่ไม่สลับ entity) เพิ่มเติมเฉพาะของ Inspector:
- scroll position **ต้อง reset กลับบนสุด** เมื่อเปลี่ยน entity ที่เลือก (สลับ selection แล้วเห็น component แรกก่อนเสมอ ไม่ใช่ scroll ค้างตำแหน่งเดิมของ entity ก่อนหน้า) — ตรงข้ามกับ F2 ที่อยากให้ scroll "คงอยู่" ตอน tree rebuild เล็กน้อย เพราะ context ต่างกัน (สลับ entity ทั้งตัว vs. hierarchy ขยับนิดหน่อย)

**ทางทำ:** ใช้ widget กลางจาก F2 (`bv_editor_ui::scrollbar`) ตรงๆ ครอบพื้นที่ field list ของ Inspector panel ไม่ต้องเขียนกลไก scroll ใหม่

**Phase:** อยู่ในขอบเขต Phase 3 เดิม (Inspector ผ่าน bevy_reflect) — เป็นของที่ควรเข้าไปพร้อมกับตอนที่ inspector list ยาวขึ้นจริง ไม่จำเป็นต้องรอ

---

## F4 — Side panel: ปรับขนาดด้วยเมาส์ลาก

**สถานะปัจจุบัน: มีอยู่แล้วและใช้งานได้จริง** — [`splitter.rs`](../crates/bv_editor_ui/src/splitter.rs) + [`shell.rs`](../crates/bv_editor_ui/src/shell.rs) ทำ splitter ที่ลากปรับ width ของ Scene Tree (ซ้าย) และ Components (ขวา) ได้แล้ว รวมถึง splitter แนวนอนปรับความสูงแถวล่าง (Project Files/Console) มี min/max px กันลากจนพังด้วย (`SIDE_PANEL_MIN_PX`/`MAX_PX` เป็นต้น) และมี unit test ของ resize math (`resize_value`) อยู่แล้ว

**ช่องว่างที่ยังไม่ครบ (ของเดิมทำแค่ "resize" ไม่ใช่ "reposition"):**
- **Cursor feedback** — ตอน hover เหนือ splitter ควรเปลี่ยน mouse cursor เป็นลูกศร resize (`↔`/`↕` ตามแกน) เพื่อบอกว่าลากได้ ตอนนี้ splitter เป็นแค่แถบสีที่ interactive เฉยๆ ไม่มี cursor hint
- **จำขนาดข้ามเซสชัน** — ปัจจุบัน panel กลับไปใช้ `side_panel_width_px(breakpoint)` เริ่มต้นทุกครั้งที่เปิดโปรแกรมใหม่ ยังไม่ persist ขนาดที่ผู้ใช้ปรับไว้ (เกี่ยวโยงกับ `editor_layout.ron` ที่ระบุไว้แล้วใน DESIGN.md Phase 10 — แนะนำรวมเป็นงานเดียวกับ F5 เพราะทั้งคู่ต้อง save/load layout state เหมือนกัน)
- **min/max ไม่ทนต่อการ resize หน้าต่างหลัง startup** — รายละเอียดเต็มอยู่ที่ F7 ด้านล่าง เพราะกระทบมากกว่าแค่ splitter ตัวเดียว (รวม viewport ที่ยังไม่มี min เลย) แยกเป็นฟีเจอร์ของตัวเอง

**Phase:** ตัว resize หลักถือว่า **เสร็จแล้วจาก Phase 1** ส่วน cursor feedback เป็น polish เล็กๆ แทรกได้ทุกเมื่อ ส่วน persist ข้ามเซสชันผูกกับ F5/Phase 10, ส่วน min/max ที่ทนต่อการ resize หน้าต่างอยู่ใน F7

---

## F5 — Side panel: ลาก-วางเพื่อรวม panel เข้าด้วยกัน (docking)

**สถานะปัจจุบัน:** ยังไม่มี — คอมเมนต์ใน `splitter.rs` ระบุไว้ตรงๆ ว่า "moving/undocking panels ('docking' proper) is out of scope until Phase 10" ตรงกับที่ DESIGN.md section 10 (ความเสี่ยง) พูดถึงไว้ว่า docking framework เป็นความเสี่ยงหลักของโปรเจกต์เพราะ `bevy_ui` ไม่มีสำเร็จรูป นี่คือฟีเจอร์ก้อนใหญ่ที่สุดในคำขอทั้ง 6 ข้อ

**สเปก (พฤติกรรมที่ต้องการ):**
- ลาก **title bar ของ panel หนึ่ง** ไปวางทับ **title bar ของอีก panel หนึ่ง** (เช่น ลาก Console ไปวางบน Project Files) → สอง panel นั้นรวมกันเป็น **tab group เดียว** ในตำแหน่งของ panel ปลายทาง แสดงเป็นแถบ tab ให้สลับดูทีละอันได้ ไม่ใช่แสดงพร้อมกันสองแผงอีกต่อไป
- ลาก tab ออกจาก tab group กลับไปวางที่ **ขอบซ้าย/ขวา/บน/ล่างของโซนว่าง** → แยกกลับเป็น panel เดี่ยวที่มี splitter ของตัวเอง (ตรงข้ามกับข้อบน)
- ระหว่างลาก ต้องมี **drop-zone indicator** ให้เห็นก่อนปล่อยเมาส์ว่าจะไปรวมเป็น tab (วางทับกลางแผงเป้าหมาย) หรือจะแยกเป็นแผงใหม่ด้านข้าง (วางที่ขอบแผงเป้าหมาย) — อย่างน้อยต้องแยกกรณี "ทับกลาง = รวม tab" กับ "ชิดขอบ = แยกแผงใหม่" ให้ชัดเจนด้วยสายตา ไม่ต้องซับซ้อนเท่า docking framework เต็มรูปแบบของ IDE ทั่วไป (ไม่มี floating window ลอยนอกหน้าต่างหลักในสโคป v1 — ตรงกับ "Multi-window editor" ที่ DESIGN.md section 2 ระบุว่าไม่ทำใน v1)
- panel ที่ถูกรวมเป็น tab group แล้ว ต้อง **resize ได้เหมือนเดิม** (ลาก splitter ของ tab group ทั้งกลุ่ม ไม่ใช่ของแต่ละ panel ย่อย เพราะตอนนี้มันคือกล่องเดียวที่มีหลาย tab อยู่ข้างใน)
- layout ที่ได้จากการจัด tab/dock ต้อง save/load ได้ ผูกกับ `editor_layout.ron` ที่ DESIGN.md Phase 10 กำหนดไว้แล้ว (รวมเป็นงานเดียวกับ "จำขนาด panel" ของ F4)

**ทางทำ (ระดับแนวคิด ไม่ใช่โค้ดจริง):**
- ใช้ **drag-and-drop framework ที่มีอยู่แล้ว** ([`dnd.rs`](../crates/bv_editor_ui/src/dnd.rs)) ตรงๆ — เพิ่ม variant ใหม่ให้ `DragPayload` เช่น `DragPayload::Panel(PanelSlotId)` แทนการสร้างกลไก drag ใหม่แยกต่างหาก title bar ของแต่ละ panel กลายเป็น `DragSource`, ทั้ง title bar และขอบ 4 ด้านของแต่ละ panel กลายเป็น `DropTarget` คนละแบบ (ทับกลาง vs ชิดขอบ ตามข้อด้านบน) — โครงสร้าง `DragState`/`DragDropped` ที่มีอยู่แล้วรองรับ pattern นี้ได้โดยไม่ต้องแก้ core ของ `dnd.rs` เลย เป็นตัวอย่างที่ดีว่า framework กลางที่ทำไว้ตั้งแต่ Phase 2 คุ้มกับ Phase 10 จริง
- โครงสร้างข้อมูล layout เปลี่ยนจาก "fixed slot ต่อ panel" (`ScenePanelSlot`, `InspectorPanelSlot`, ... ที่ `shell.rs` มีตอนนี้) เป็น **tree ของ dock node** (แนวคิดคล้าย `egui_dock`/VS Code: node เป็น `Split(axis, [child nodes], sizes)` หรือ `Tabs([panel ids], active index)`) — เป็นการ refactor `shell.rs` ที่ใหญ่พอสมควร เพราะตอนนี้ shell สร้างกล่อง panel แบบ hardcode ตำแหน่งไว้ในโค้ดตรงๆ
- แต่ละ panel (`PanelDescriptor` จาก extension API ข้อ 5 ของ DESIGN.md) ต้องมี **title bar ที่ render จริง** เป็นของตัวเอง (ตอนนี้มีแค่ label ข้อความลอยๆ บนสุดของกล่อง ไม่ใช่ title bar ที่เป็น drag handle ได้) — งานนี้เชื่อมกับ F6 (icon) ด้วย เพราะ IDE ทั่วไปจะมี icon เล็กๆ หน้าชื่อ tab

**Phase:** ตรงกับ **Phase 10 เดิมของ DESIGN.md** ("ปรับ docking framework ให้ลาก tab ย้ายตำแหน่งได้ ... เก็บ layout ลง `editor_layout.ron`") — เอกสารนี้ขยายบรรทัดเดียวในนั้นให้เป็นสเปกละเอียด ไม่ใช่ phase ใหม่ **คำเตือนเรื่อง scope:** นี่คือฟีเจอร์ที่เสี่ยงบานปลายที่สุดในบรรดา 6 ข้อ (ตรงกับความเสี่ยงข้อแรกที่ DESIGN.md section 10 เตือนไว้แล้ว) แนะนำทำหลัง Phase 1–9 เสถียรตามลำดับเดิม ไม่ดึงมาทำก่อนกำหนด

---

## F6 — Icon: ทางเพิ่ม icon มาใช้ใน UI

**สถานะปัจจุบัน:** ไม่มีเลย — ไม่มี icon asset, icon font, หรือ icon widget ในโค้ดตอนนี้ (ค้นทั้ง workspace ไม่เจอคำว่า "icon" เลยนอกจาก `docs/DESIGN.md` ที่พูดถึง `ManipulatorDescriptor { name, icon, hotkey }` เป็นแค่ field ที่ยังไม่ implement — ดู section 7.2 ของ DESIGN.md) Toolbar/panel title ปัจจุบันเป็นตัวหนังสือล้วน

**สเปก:**
- ต้องมีทาง "เพิ่ม icon ใหม่" ที่ **ไม่ต้องแก้ core crate** เหมือนหลักการ extension อื่นๆ ของโปรเจกต์นี้ (เข้ากับ requirement ข้อ 3 ของ DESIGN.md — extension เพิ่ม panel/manipulator ใหม่ได้ ถ้ามันมี icon ของตัวเอง ก็ต้องลงทะเบียน icon เองได้เหมือนกัน)
- ใช้ได้อย่างน้อยใน 3 จุดที่มีอยู่แล้วในโปรเจกต์: **toolbar** (ปุ่ม manipulator ตาม 7.2), **panel/tab title bar** (จะมาพร้อม F5), และ **Scene Tree row** (icon บอกประเภท entity เช่น มี `PointLight`/`Camera`/mesh หรือไม่ — nice-to-have ไม่ใช่ blocking)
- รองรับทั้ง **built-in icon set เล็กๆ** ที่มากับ `bv_editor` (ให้ panel ในตัว/manipulator ในตัวมี icon ใช้ตั้งแต่แรก) และ **icon ที่ extension ใส่เข้ามาเอง** (แนว asset-reference)

**ทางทำ (ตัวเลือก ไม่ใช่ตัดสินใจ):**

| ทางเลือก | ข้อดี | ข้อเสีย |
|---|---|---|
| **Icon font** (สร้าง/ใช้ font ที่ glyph แต่ละตัวคือ icon หนึ่งอัน) | เบา, สเกลคมทุกขนาดจอ (vector), วาดผ่าน `Text`/`TextFont` ที่ `bevy_ui` รองรับอยู่แล้วโดยตรง ไม่ต้องมี widget type ใหม่ | ต้องมี build step สร้าง font จาก SVG (เครื่องมือนอกวง Bevy), เพิ่ม icon ใหม่ = build font ใหม่ทั้งไฟล์ |
| **Sprite sheet / texture atlas** (`Image` + UV rect ต่อ icon ผ่าน `ImageNode`) | ใช้ `bevy_asset`/`ImageNode` ที่มีอยู่แล้วในวง built-in (เหมือนวิธีที่ viewport texture ใช้ตาม section 4 ของ DESIGN.md), เพิ่ม icon ใหม่ = เพิ่มไฟล์ภาพ+entry ไม่ต้อง rebuild อะไรที่ซับซ้อน | ต้อง maintain atlas layout เอง (ตำแหน่ง UV ต่อ icon), ไม่ scale คมเท่า vector ถ้า raster ต้นทางเล็ก |
| **ไฟล์ svg/png แยกไฟล์ต่อ icon** (โหลดผ่าน `AssetServer` ตรงๆ ไม่รวม atlas) | ง่ายสุด, เพิ่ม icon = วางไฟล์ใหม่ | เปลือง draw call/asset handle ถ้า icon เยอะ, ไม่มี batching |

**ข้อเสนอ:** เริ่มจาก **sprite sheet/texture atlas** เพราะสอดคล้องกับหลักการ "built-in lib ของ Bevy ก่อนเสมอ" (section 4 ของ DESIGN.md) มากที่สุด ไม่ต้องมี build tool นอกวงแบบ icon font และรูปแบบ registry ใกล้เคียงกับ `ManipulatorRegistry`/`PanelRegistry` ที่มีอยู่แล้ว (เพิ่ม `IconRegistry` resource คู่กันได้ทันที: `app.register_icon(IconId, atlas_index)` — ตรงกับ pattern `register_manipulator`/`register_editor_panel` ที่โปรเจกต์นี้ใช้อยู่แล้วทุกที่) — แต่เป็นข้อเสนอเปิดให้ถกก่อน ไม่ใช่ตัดสินใจปิดท้าย

**Phase:** ไม่ผูกกับ phase เดียว เพราะเป็น "โครงสร้างพื้นฐาน" ที่ F5 (title bar) และ toolbar (Phase 4, manipulator icon) ต้องใช้ร่วมกัน แนะนำทำ `IconRegistry` ขั้นต่ำ (registry + วิธีวาด icon หนึ่งดวงใน `bevy_ui`) **ก่อน** F5 เพราะ F5 อ้างถึงมันในสเปก title bar แล้ว ไม่ต้องรอให้ set icon ครบทุกจุดก่อนเริ่ม

---

## F7 — Panel: min/max size ที่ยึดอยู่จริง ไม่ใช่แค่ตอนลาก splitter

**สถานะปัจจุบัน:** มี min/max อยู่แล้ว **แต่บังคับใช้แค่ตอนกำลังลาก splitter ตัวนั้นอยู่เท่านั้น** — [`shell.rs`](../crates/bv_editor_ui/src/shell.rs) กำหนด `SIDE_PANEL_MIN_PX`/`MAX_PX` (150/640) ให้ Scene Tree/Components และ `BOTTOM_ROW_MIN_PX`/`MAX_PX` (80/560) ให้แถวล่าง ผ่าน `Splitter { min_px, max_px, .. }` ต่อตัว ซึ่ง [`splitter_drag_system`](../crates/bv_editor_ui/src/splitter.rs) เรียก `resize_value` clamp ทุกครั้งที่ลาก — ใช้งานได้ดีในสถานการณ์นั้นเพียงอย่างเดียว

ช่องโหว่ที่ยังไม่ถูกปิด (ไล่โค้ดแล้วพบ 3 จุด):

1. **Resize หน้าต่างหลัง startup ไม่ re-clamp เลย** — [`breakpoint.rs`](../crates/bv_editor_ui/src/breakpoint.rs) ในคอมเมนต์ของไฟล์เองยอมรับตรงๆ ว่า "reacting continuously to window resizes is not part of the Phase 1 scope" breakpoint ถูกคำนวณครั้งเดียวตอน `Startup` เท่านั้น (`spawn_shell_on_startup`) ถ้าผู้ใช้ลาก panel ไปที่ `max_px` (640px) แล้วค่อยลดขนาดหน้าต่าง OS ให้เล็กลงมากๆ ไม่มี system ไหนไปหด panel กลับเข้า bound เลย
2. **Viewport (กล่องกลาง) ไม่มี min size เลย** — ใน `shell.rs` viewport เป็นแค่ `Node { flex_grow: 1.0, height: Val::Percent(100.0), ..panel_node() }` ไม่มี `min_width` ถ้า Scene Tree + Components ทั้งสองฝั่งถูกลากไปใกล้ `max_px` (640+640=1280px) บนหน้าต่างที่ไม่ได้กว้างมาก viewport จะถูกบีบจนความกว้างเข้าใกล้ 0 หรือติดลบ (flexbox จะ clamp ที่ 0 แต่ผลคือ viewport หายไปเงียบๆ ไม่มี floor ป้องกัน)
3. **Bottom row (Project Files/Console) ก็ไม่มี min ต่อฝั่งเหมือนกัน** — มีแค่ splitter เดียวคุมความสูงของทั้งแถว (`bottom_row`) แต่ระหว่าง Project Files กับ Console เองใช้ `flex_grow` ล้วนๆ (2 ต่อ 1) ไม่มี `min_width` ต่อฝั่ง เข้าเคสเดียวกับข้อ 2 ถ้าแถวทั้งก้อนถูกบีบความสูงจนต่ำมาก (แม้ตัวแถวเองมี `BOTTOM_ROW_MIN_PX` กันไว้ที่ 80px ก็ตาม เนื้อหาภายในแถวยังโดนบีบความกว้างได้อิสระ)

**สเปก:**

- ทุก panel ต้องประกาศ `(min_px, max_px)` ของตัวเองเป็นค่าที่มีอยู่จริงตลอดเวลา ไม่ใช่แค่ค่าที่ splitter อ้างถึงตอนลาก — เพิ่ม **system ที่รันทุกครั้งที่ window resize** (ไม่ใช่แค่ตอน `Startup`) ไล่ตรวจ panel ที่มี bound แล้ว clamp ขนาดปัจจุบันกลับเข้า range ถ้าเกิน (ไม่ reset กลับไปที่ค่า breakpoint default — งานนี้ต้อง **ไม่ทำลาย** ขนาดที่ผู้ใช้ตั้งเองด้วยมือถ้ามันยังอยู่ในขอบเขตที่ยอมรับได้ของขนาดหน้าต่างใหม่)
- **Viewport ต้องมี min width/height เป็นของตัวเอง** (ตัวเลขที่ยังพอมองเห็นอะไรได้ เช่น 200px) แล้ว side-panel splitter ต้อง "รู้" bound นี้ด้วย ไม่ใช่แค่ bound ของ panel ตัวเอง — ลาก Scene Tree ให้กว้างขึ้นต้องหยุดก่อนที่ viewport จะแคบกว่า floor ของมัน แม้ Scene Tree เองยังไม่ถึง `max_px` ก็ตาม (คือ "min ของเพื่อนบ้าน" มีผลย้อนกลับมาจำกัด max ที่ลากได้จริงของ splitter)
- Bottom row: ให้ Project Files/Console มี `min_width` ต่อฝั่งเหมือนกัน (ตัวเลขเล็กพอจะยังเห็น breadcrumb/log ได้ เช่น 120px) ด้วยหลักการเดียวกับข้อบน
- เมื่อ **ผลรวม min ของทุก panel ในแถวเดียวกันมากกว่าความกว้างหน้าต่างจริง** (หน้าต่างเล็กมากๆ) เป็น edge case ที่ยอมรับให้ overflow/ซ้อนทับกันได้ในรอบแรก (ไม่ต้องมี layout สำรอง เช่น auto-collapse panel) — ระบุไว้ตรงๆ ว่าเป็น known limitation ไม่ใช่ bug ที่ต้องปิดในรอบนี้ เพื่อกันสโคปบาน

**ทางทำ (แนวคิด ไม่ใช่โค้ดจริง):**

- เพิ่ม `PanelSizeBounds { min_px: f32, max_px: f32 }` component แยกจาก `Splitter` (ปัจจุบัน bound ผูกอยู่กับ splitter โดยตรง ซึ่งใช้ได้ตอนลากอย่างเดียว) ใส่ให้ทุก panel entity ที่มี bound จริง แล้วให้ทั้ง `splitter_drag_system` และ system ใหม่ (เรียกว่า `enforce_panel_bounds_on_resize` หรือคล้ายกัน) อ่านตัวเดียวกันนี้ แทนที่จะมี bound สองชุดไม่ sync กัน
- ระบบใหม่ subscribe `bevy_window::WindowResized` event (มีอยู่แล้วใน `bevy_window`, built-in) แทนการ poll ทุกเฟรม — ตรงกับหลักการ "built-in ก่อนเสมอ" ของ section 4 DESIGN.md
- viewport's min เป็นค่าคงที่ระดับ shell (ไม่ใช่ต่อ splitter) — วิธีที่ตรงไปตรงมาที่สุดคือให้ `resize_value` (หรือ wrapper ใหม่) รับพารามิเตอร์เพิ่มคือ "ที่ว่างที่เหลือของเพื่อนบ้านตอนนี้" แล้วคำนวณ max ที่ใช้ได้จริง ณ ขณะนั้น (`effective_max = max_px.min(current_row_width - neighbor_min_px - splitter_thickness)`) เป็น pure function ใหม่ที่ต่อยอดจาก `resize_value` เดิม เทสแยกได้เหมือนกัน

**Phase:** เป็นการเติมของ Phase 1 (`bv_editor_ui`) ให้ "resizable splitter" ที่ระบุไว้แล้วใน DESIGN.md สมบูรณ์ขึ้น ไม่ใช่ phase ใหม่ — ทำได้อิสระจาก F1–F6 ทั้งหมด (ไม่พึ่งอะไรจากข้ออื่น และไม่มีข้อไหนพึ่งมันกลับ) จึงแทรกเข้าคิวตรงไหนก็ได้

---

## สรุปลำดับที่แนะนำ (ตอน implement จริง)

ลำดับนี้เรียงตามการพึ่งพากันของฟีเจอร์ ไม่ใช่ความสำคัญ — ต้องคุยและ agree สเปกด้านบนให้เรียบร้อยก่อนเริ่มโค้ดข้อไหนทั้งสิ้น:

1. **F1** (row highlight) — เล็ก อิสระ ทำได้ทันทีไม่พึ่งอะไรใหม่
2. **F2 → F3** (scrollbar) — ทำ widget กลางใน `bv_editor_ui` ครั้งเดียว ใช้ซ้ำสองที่
3. **F4 ส่วน cursor feedback** — เล็ก อิสระ แทรกเมื่อไหร่ก็ได้
4. **F7** (min/max ที่ยึดอยู่จริงตอน resize หน้าต่าง) — เล็ก-กลาง อิสระจากข้ออื่นทั้งหมด แทรกเมื่อไหร่ก็ได้เหมือนกัน แนะนำทำก่อน F5 เพราะ F5 (docking) จะยิ่งทำให้ layout ซับซ้อนขึ้นอีก ควรมี bound ที่แข็งแรงไว้ก่อน
5. **F6 ขั้นต่ำ** (`IconRegistry` + วาด icon เดี่ยวได้) — โครงสร้างพื้นฐานที่ F5 ต้องใช้
6. **F5** (docking) — ก้อนใหญ่สุด เสี่ยงบานปลายสุด ทำหลังสุด ตรงกับ Phase 10 เดิมของ DESIGN.md รวม F4 ส่วน persist layout เข้าไปพร้อมกัน

ทุกข้อยังต้องมี headless test ตามหลักการ section 8.1 ของ DESIGN.md (`bv_editor_test_utils`) ไม่ใช่ "ดูด้วยตา" อย่างเดียว — resize/scroll/drag math ทั้งหมดควรแยกเป็น pure function เทสได้แบบไม่พึ่ง ECS ก่อน (ตามแนวทางที่ `resize_value` ใน `splitter.rs` วางไว้แล้ว) แล้วค่อยห่อด้วย system บางๆ ต่อ
