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
- **ซ่อนทั้ง track และ thumb** (`Display::None`) เมื่อเนื้อหาพอดีกับพื้นที่ ไม่ใช่ซ่อนแค่ thumb — ของเดิม (ก่อน 2026-09-19) ซ่อนแค่ thumb เฉยๆ ทำให้แถบพื้นหลัง (track) ค้างโชว์ตลอดแม้ไม่มีอะไรให้ scroll ดูเหมือน scrollbar ค้าง
- scroll position "คงอยู่" ข้าม tree rebuild ได้ฟรีอยู่แล้ว เพราะ `ScrollPosition` อยู่บน container entity ที่ไม่เคยถูก despawn (`rebuild_scene_tree_ui` despawn เฉพาะแถว ไม่แตะ container)
- เทสครอบคลุมทั้ง pure math (`thumb_geometry`/`clamp_scroll`) และ ECS-level (wheel scroll เฉพาะ container ที่ hover, ลาก thumb แล้ว target scroll ตาม, thumb ซ่อน/โผล่ตามเนื้อหา) รวม 12 เทสใหม่ใน `bv_editor_ui` + 1 เทส integration ใน `bv_editor_scene_panel` ที่ยืนยันว่า thumb ที่ spawn จริงชี้ไปที่ `SceneTreeRowsContainer` ที่ถูกต้อง

**Bug fix (2026-09-19): track ค้างโชว์ตลอด + ต่อมาซ่อนค้างตลอด (deadlock)** — สองรอบติดกัน:
1. เดิม `sync_scrollbar_thumb_system` ซ่อนแค่ thumb ไม่แตะ track เลย → track (แถบพื้นหลังมืดๆ) โชว์ตลอดแม้เนื้อหาพอดีพื้นที่ ดูเหมือน scrollbar ค้าง
2. แก้รอบแรกโดยให้ซ่อน track ด้วยเมื่อไม่ overflow แล้วเจอบั๊กใหม่ที่ร้ายแรงกว่า: เงื่อนไข "overflow หรือไม่" ตอนนั้นใช้ `track_length > 0.0` ร่วมด้วย (เดิมมีอยู่แล้วเพื่อกันหารด้วยศูนย์) — แต่ `track_length` มาจาก `ComputedNode` ของ track เอง ซึ่งพอ track ถูกซ่อน (`Display::None`) `bevy_ui`'s layout engine (taffy) จะไม่จัด layout ให้ node นั้นเลย ทำให้ `ComputedNode.size` ของมันเหลือ 0 ค้างตลอดไป — ผลคือเกิด **deadlock ในตัวเอง**: track ถูกซ่อนแล้ว size กลายเป็น 0 แล้ว 0 นั้นเองก็ทำให้เงื่อนไข "overflow" เป็น false ตลอดกาล (เพราะต้องการ `track_length > 0`) ต่อให้เนื้อหา overflow มากแค่ไหนก็ไม่มีทางโชว์ track กลับมาได้อีกเลย
   - แก้โดยตัด `track_length` ออกจากเงื่อนไข overflow ทั้งหมด — ตัดสิน "overflow หรือไม่" จาก `content > visible` ของ **target** (container ที่ scroll ได้ ไม่ใช่ตัว track เอง) เพียงอย่างเดียว เพราะ target ไม่เคยถูกซ่อนโดยระบบนี้ ค่า `ComputedNode` ของมันเชื่อถือได้เสมอ ส่วน `track_length` ยังใช้ต่อได้ปกติแต่ใช้แค่ตอนคำนวณขนาด thumb (`thumb_geometry`) เท่านั้น ไม่ใช่ตัดสินใจ show/hide
   - เทส `sync_can_reveal_a_track_whose_own_computed_size_is_still_zero` (`bv_editor_ui/src/scrollbar.rs`) จำลองสถานการณ์นี้ตรงๆ: track ComputedNode.size = 0 แต่ content ของ target overflow visible จริง แล้วยืนยันว่า track/thumb ยัง Flex ได้ ไม่ค้าง None ตลอดไป
   - **บทเรียน:** อย่าตัดสิน "จะโชว์ widget นี้ไหม" จากขนาดที่ตัว widget เดียวกันวัดตัวเองได้ ถ้า widget นั้นถูกซ่อนด้วยเงื่อนไขเดียวกัน — จะเกิด self-referential deadlock ทันที ต้องตัดสินจากแหล่งข้อมูลที่ไม่ถูกซ่อน (ในที่นี้คือ target/container) เสมอ

---

## F3 — Inspector (Components) panel: Scrollbar เมื่อ field/component เยอะ ✅ implemented

**สถานะ:** ทำเสร็จแล้ว — ใช้ widget กลางจาก F2 (`bv_editor_ui::scrollbar`) ตรงๆ ไม่ต้องเขียนกลไก scroll ใหม่ ตามที่วางแผนไว้ การเปลี่ยนแปลงอยู่ใน [`bv_editor_inspector_panel/src/lib.rs`](../crates/bv_editor_inspector_panel/src/lib.rs):
- `InspectorBody` ได้ `flex_grow: 1.0` + `min_height: Val::Px(0.0)` + `overflow: Overflow::scroll_y()` เหมือน Scene Tree's rows container ใน F2 พร้อม scrollbar track/thumb ข้างๆ
- **ต่างจาก F2 ตรงตามสเปก:** `rebuild_inspector_ui` reset `ScrollPosition.y` กลับ 0 ทุกครั้งที่ rebuild จริง (ไม่ใช่แค่ตอน dirty flag ถูก set เฉยๆ) — ใช้ได้เพราะ `InspectorDirty` ถูก set จาก `Selection` เปลี่ยนแปลงเท่านั้น (`detect_selection_change`) ไม่มีสาเหตุอื่น ต่างจาก Scene Tree ที่ dirty มาจากทั้ง selection และ hierarchy เปลี่ยนแปลงปนกัน จึงต้อง "คงอยู่" แทน
- เทสครอบคลุมทั้ง wiring (`scrollbar_thumb_targets_the_inspector_body_and_wheel_scrolls_it`) และพฤติกรรม reset (`switching_selection_resets_the_scroll_position`: scroll ไปที่ 123px ด้วยมือ แล้วสลับ selection แล้วต้องกลับเป็น 0)

**เพิ่ม horizontal scrollbar (2026-09-19):** field บางตัว (เช่น `Vec3`/`Color` ที่ขยายเป็นหลายกล่อง `f32` ต่อแถว) กว้างเกิน panel แคบๆ ได้ ก่อนหน้านี้ `InspectorBody` scroll ได้แค่แนวตั้ง (`Overflow::scroll_y()`) เนื้อหาที่กว้างเกินเลยทะลุขอบ panel ออกไปแบบไม่มีทางเลื่อนไปดูได้เลย —
- `bv_editor_ui::scrollbar` generalize ให้ `ScrollbarThumb` มี field `axis: ScrollbarAxis` (`Horizontal`/`Vertical`) แทนที่จะ hardcode แนวตั้งอย่างเดียว — `scrollbar_drag_system`/`sync_scrollbar_thumb_system` ใช้ `axis` เลือกว่าจะอ่าน/เขียน `.x` หรือ `.y` ของ `ScrollPosition`/ขนาด thumb (`width`+`left` สำหรับแนวนอน, `height`+`top` สำหรับแนวตั้ง) — widget เดิมใช้ซ้ำได้ทั้งสองแกนโดยไม่ต้องเขียน type ใหม่ (`wheel_scroll_system` ยังเป็นแนวตั้งอย่างเดียวตามเดิม ไม่ผูก wheel กับแนวนอน เพราะไม่มีคนขอ)
- `InspectorBody` เปลี่ยนเป็น `Overflow::scroll()` (ทั้ง x และ y) + เพิ่ม `min_width: Val::Px(0.0)` คู่กับ `min_height: Val::Px(0.0)` เดิม (เหตุผลเดียวกับ F2/F3: ไม่งั้น flexbox ไม่ยอมบีบ body ให้แคบกว่าความกว้างธรรมชาติของเนื้อหา)
- layout เปลี่ยนเป็นทรง "L" (แบบ scroll pane ทั่วไป): `content_row` (body | vertical track) อยู่แถวบน, horizontal track เต็มความกว้างอยู่แถวล่าง — ทั้งคู่เป็น `ScrollbarThumb` ที่ `target: body` เดียวกัน คนละ `axis`
- เทสใหม่ 3 เคสใน `bv_editor_ui/src/scrollbar.rs`: `sync_sizes_a_horizontal_thumb_by_width_not_height`, `dragging_a_horizontal_thumb_scrolls_the_target_on_x`, และเทส regression ของ deadlock bug ด้านบน (`sync_can_reveal_a_track_whose_own_computed_size_is_still_zero`)
- **ข้อควรระวังตอน debug ฟีเจอร์นี้:** headless test fake `ComputedNode` ตรงๆ เลยไม่มีทางเจอ deadlock bug ด้านบนได้เอง (fake ค่าไม่ผ่าน `Display::None` จริง) ต้องรันแอปจริง (`ui_gallery`) แล้วอ่าน log ค่า track/visible/content สดๆ ถึงจับได้ — ถ้าจะแก้ scrollbar ต่อในอนาคต แนะนำ reproduce ผ่านแอปจริงเสมอ ไม่ใช่เชื่อแค่ headless test ผ่าน

**Polish (2026-09-19): track หนาขึ้น + cursor feedback ตอน hover/ลาก**
- `SCROLLBAR_TRACK_WIDTH_PX` (ทั้ง `bv_editor_scene_panel` และ `bv_editor_inspector_panel`) จาก 8px → 12px — track/thumb แนวนอนที่เพิ่งเพิ่มบางเกินไปจนแทบมองไม่เห็น/กดยาก โดยเฉพาะตอน `ui_scale` default 0.5 (แสดงจริงแค่ ~4px จอ) เพิ่มพร้อมกันทั้งสองแกนเพื่อความสมมาตร
- hover หรือลาก scrollbar thumb เปลี่ยน mouse cursor เป็น `SystemCursorIcon::Grab` (hover)/`Grabbing` (กำลังลาก) จริง ผ่าน `bevy_window::CursorIcon` บน primary window — ระบบใหม่ `scrollbar_cursor_system` (`scrollbar.rs`) ตรรกะเดียวกับ `splitter_cursor_system` ของ F4 ทุกประการ: เพิ่ม resource `ActiveScrollbarDrag` (แทนที่ `Local<Option<Entity>>` เดิมของ `scrollbar_drag_system`) แชร์สถานะ "กำลังลาก thumb ไหนอยู่" กันคนละ system อ่านได้ ลำดับความสำคัญ: กำลังลาก > hover > ไม่มีทั้งคู่ (cursor กลับ default)
- เทสใหม่ 2 เคส: `hovering_a_scrollbar_thumb_sets_the_grab_cursor_and_clears_it_after`, `dragging_a_scrollbar_thumb_sets_the_grabbing_cursor`

**Refactor (2026-09-19): แยกเป็น widget `ScrollArea`** — ก่อนหน้านี้ `bv_editor_scene_panel::spawn_scene_panel_chrome` และ `bv_editor_inspector_panel::spawn_inspector_chrome` ต่างคน spawn Node tree ของ container+track+thumb เองแยกกัน (มีแค่ component/system เบื้องหลังใน `bv_editor_ui::scrollbar` ที่ใช้ร่วมกัน) — เป็นสาเหตุที่บั๊ก `flex_shrink` (ด้านบน) ต้องแก้ซ้ำ 2 ที่ เพราะไม่มีจุดเดียวที่เป็นความจริงหนึ่งเดียว (single source of truth) ของ "หน้าตา scroll area ควรเป็นยังไง"
- เพิ่มไฟล์ใหม่ [`bv_editor_ui::scroll_area`](../crates/bv_editor_ui/src/scroll_area.rs) — ฟังก์ชัน `spawn_scroll_area(commands, parent, axes: ScrollAxes, style: ScrollAreaStyle) -> Entity` ที่ spawn โครงสร้างทั้งหมด (root column + content_row + body + track(s)/thumb(s) พร้อม `flex_shrink: 0.0` กัน crush) คืนแค่ entity ของ **body** (ยังไม่มี marker component ของ panel) ให้ caller ทำ `commands.entity(body).insert(YourMarker)` ต่อเอง — เป็น pattern "reserve id แล้ว insert component จริงทีหลัง" แบบเดียวกับที่ `shell.rs` ใช้อยู่แล้ว
- `ScrollAxes` enum (`Vertical`/`Horizontal`/`Both`) เลือกว่าจะได้ track กี่แกน — `Vertical` ให้โครงสร้างเดียวกับที่ Scene Tree เคยเขียนเอง, `Both` ให้ทรง "L" เดียวกับที่ Components panel เคยเขียนเอง, `Horizontal` เผื่ออนาคต (ยังไม่มีใครใช้ แต่ implement ไว้ให้ครบตาม pattern เดียวกัน)
- `bv_editor_scene_panel`/`bv_editor_inspector_panel` เปลี่ยนมาเรียก `spawn_scroll_area` แทนของเดิมทั้งหมด — ผลลัพธ์ทาง layout/พฤติกรรมเหมือนเดิมทุกอย่าง (เทสเดิมทั้ง 19+5 เคสผ่านหมดไม่ต้องแก้อะไรเลย) โค้ด `spawn_scene_panel_chrome`/`spawn_inspector_chrome` สั้นลงมาก
- เทสใหม่ 3 เคสใน `scroll_area.rs`: `vertical_only_gets_exactly_one_vertical_thumb`, `both_axes_gets_one_thumb_per_axis`, `every_track_opts_out_of_flex_shrink` (เทส regression ของบั๊ก `flex_shrink` ตรงๆ ที่จุดเดียวนี้ ครอบคลุมทั้งสอง panel ไปในตัว)

**Phase:** อยู่ในขอบเขต Phase 3 เดิม (Inspector ผ่าน bevy_reflect) ตามที่วางแผนไว้

---

## F4 — Side panel: ปรับขนาดด้วยเมาส์ลาก

**สถานะปัจจุบัน: มีอยู่แล้วและใช้งานได้จริง** — [`splitter.rs`](../crates/bv_editor_ui/src/splitter.rs) + [`shell.rs`](../crates/bv_editor_ui/src/shell.rs) ทำ splitter ที่ลากปรับ width ของ Scene Tree (ซ้าย) และ Components (ขวา) ได้แล้ว รวมถึง splitter แนวนอนปรับความสูงแถวล่าง (Project Files/Console) มี min/max px กันลากจนพังด้วย (`SIDE_PANEL_MIN_PX`/`MAX_PX` เป็นต้น) และมี unit test ของ resize math (`resize_value`) อยู่แล้ว

**Bug fix (2026-09-19): ทิศทางลาก invert กับเมาส์บน panel ขวา/ล่าง** — `resize_value` ทำ `current + delta` เสมอ ซึ่งถูกต้องเฉพาะ splitter ที่เป็น**ขอบขวา**ของ target (Scene Tree ซ้ายมือ) แต่ splitter ของ Components panel เป็น**ขอบซ้าย**ของ target และ splitter ของแถวล่างเป็น**ขอบบน**ของ target — ทั้งสองกรณีนี้ลากขวา/ลง (`delta` บวก) ต้องทำให้ target **เล็กลง** ไม่ใช่ใหญ่ขึ้น โค้ดเดิมไม่ได้แยกกรณีนี้เลยจึงลาก Components panel/แถวล่างแล้วขนาดวิ่งสวนทางเมาส์
- แก้โดยเพิ่ม field `Splitter::invert: bool` — `true` สำหรับ splitter ที่เป็นขอบซ้าย/บนของ target (Components panel, แถวล่าง), `false` สำหรับขอบขวา/ล่าง (Scene Tree) — `splitter_drag_system` กลับเครื่องหมาย `delta` ก่อนส่งเข้า `resize_value` เมื่อ `invert == true`
- เทส `inverted_splitter_drag_resizes_its_target_opposite_the_mouse` (`bv_editor_ui/src/lib.rs`) ยืนยันว่าลาก Components panel splitter ไปทางขวา 40px แล้ว panel เล็กลง 40px (ไม่ใช่ใหญ่ขึ้น) แยกจาก `splitter_drag_resizes_its_target` เดิมที่เทส Scene Tree (ไม่ invert)

**Cursor feedback ✅ implemented (2026-09-19):** hover หรือลาก splitter เปลี่ยน mouse cursor เป็น `SystemCursorIcon::EwResize` (`↔`, `SplitterAxis::Horizontal`) หรือ `NsResize` (`↕`, `SplitterAxis::Vertical`) จริง โดย insert/remove component `bevy_window::CursorIcon` บน primary window entity —
- `splitter_cursor_system` (ใน `splitter.rs`) รันต่อจาก `splitter_drag_system` ทุกเฟรม (`.chain()`) อ่าน resource `ActiveSplitterDrag` (เพิ่มใหม่ แทนที่ `Local<Option<Entity>>` เดิมของ `splitter_drag_system` เพื่อให้สอง system แชร์สถานะ "กำลังลากตัวไหนอยู่" กันได้) — ถ้ากำลังลากอยู่ ยึด cursor ตามแกนของตัวที่ลากอยู่ก่อนเสมอ (กันไม่ให้ cursor กระพริบกลับ default ตอนลากเร็วจนเมาส์หลุดพื้นที่บางๆ ของ splitter) ถ้าไม่ได้ลาก fallback ไปดู splitter ที่ hover อยู่แทน ถ้าไม่มีทั้งคู่ก็ `remove::<CursorIcon>()` คืนเป็น default ของแพลตฟอร์ม
- no-op ปลอดภัยเมื่อไม่มี primary window (เช่นใน headless test) เหมือน pattern เดิมของ `spawn_shell_on_startup`
- เทสครอบคลุมทั้ง pure mapping (`cursor_icon_matches_axis`: axis → cursor icon ที่ถูกต้อง) และ ECS-level (`hovering_a_splitter_sets_the_resize_cursor_and_clears_it_after`: hover แล้วเช็ค `CursorIcon` component บน window entity ถูก insert/remove ถูกต้อง)

**ช่องว่างที่ยังไม่ครบ:**
- **จำขนาดข้ามเซสชัน** — ปัจจุบัน panel กลับไปใช้ `side_panel_width_px(breakpoint)` เริ่มต้นทุกครั้งที่เปิดโปรแกรมใหม่ ยังไม่ persist ขนาดที่ผู้ใช้ปรับไว้ (เกี่ยวโยงกับ `editor_layout.ron` ที่ระบุไว้แล้วใน DESIGN.md Phase 10 — แนะนำรวมเป็นงานเดียวกับ F5 เพราะทั้งคู่ต้อง save/load layout state เหมือนกัน)
- **min/max ไม่ทนต่อการ resize หน้าต่างหลัง startup** — รายละเอียดเต็มอยู่ที่ F7 ด้านล่าง เพราะกระทบมากกว่าแค่ splitter ตัวเดียว (รวม viewport ที่ยังไม่มี min เลย) แยกเป็นฟีเจอร์ของตัวเอง

**Phase:** ตัว resize หลักและ cursor feedback ถือว่า **เสร็จแล้วจาก Phase 1** ส่วน persist ข้ามเซสชันผูกกับ F5/Phase 10, ส่วน min/max ที่ทนต่อการ resize หน้าต่างอยู่ใน F7

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

## F6 — Icon: ทางเพิ่ม icon มาใช้ใน UI ✅ implemented (ขั้นต่ำ — registry เท่านั้น)

**สถานะ:** ทำเสร็จแล้วเฉพาะส่วน "โครงสร้างพื้นฐาน" ตามที่สรุปลำดับแนะนำไว้ด้านล่าง ("ทำ `IconRegistry` ขั้นต่ำ ... ไม่ต้องรอให้ set icon ครบทุกจุดก่อนเริ่ม") — ใหม่ทั้งไฟล์ [`bv_editor_ui::icon`](../crates/bv_editor_ui/src/icon.rs):

- **ไม่ใช้ `TextureAtlasLayout`/`TextureAtlas` asset เลย** ต่างจากข้อเสนอเดิมในสเปกด้านล่าง (sprite sheet/texture atlas) — ระหว่าง implement พบว่า `bevy_ui`'s `ImageNode` มี field `rect: Option<Rect>` (พิกัดพิกเซล) อยู่แล้วในตัว ซึ่ง doc comment ของมันเองบอกตรงๆ ว่าเป็น "an easy one-off alternative to using a `TextureAtlas`" — ใช้ field นี้ตรงๆ พอสำหรับ "sprite sheet เดียว + rect ต่อ icon" โดยไม่ต้องลงทะเบียน asset type ใหม่ (`TextureAtlasLayout`) หรือพึ่ง `Assets<TextureAtlasLayout>`/`Handle<TextureAtlasLayout>` เลย ตรงกับหลักการ "built-in ก่อนเสมอ" ยิ่งกว่าแผนเดิมอีกขั้นหนึ่ง (เหมือนที่ F7 เจอกับ `Node.min_*`/`max_*`)
- `IconId(&'static str)` เป็น string key ธรรมดา (ไม่ใช่ enum ปิด) ตามหลักการเดียวกับ `bv_editor_core::HotkeyRegistry` ที่ key hotkey action ด้วย string — extension ลงทะเบียน icon ของตัวเองได้โดยที่ `bv_editor_ui` ไม่ต้องรู้จักล่วงหน้า
- `IconRegistry` (resource) เก็บ `IconId → (Handle<Image>, Rect)` ผ่าน `register()`/`image_node()` — `image_node()` คืน `None` ถ้าไม่มีใครลงทะเบียน id นั้น ให้ caller "ไม่วาดอะไรเลย" แทนที่จะ panic (คตินิยมเดียวกับ `*Slot`-less startup system ที่มีอยู่แล้วในโปรเจกต์)
- `spawn_icon(commands, parent, registry, id, size_px) -> Option<Entity>` widget ระดับ spawn — คืน `None` และไม่ spawn อะไรเลยถ้า id ไม่ได้ลงทะเบียน (ไม่ fallback เป็นกล่องเปล่า เพราะ unregistered id ถือเป็นบั๊กของ caller ที่ต้องแก้ ไม่ใช่ state ที่ควร render เผื่อไว้)
- **built-in icon เดียว** `PLACEHOLDER_ICON` (`"bv_editor.placeholder"`) — สี่เหลี่ยมสีเทาแบนๆ 16×16px สร้างแบบ procedural ผ่าน `Image::new_fill` ตอน `Startup` (เหตุผลเดียวกับที่ `bv_editor_viewport`'s render-target image เป็น procedural: repo นี้ยังไม่มี asset pipeline จริง และสี่เหลี่ยมแบนไม่ต้องพึ่งเครื่องมือนอกวงเลย) — มีไว้พิสูจน์ว่า pipeline registry→sprite sheet→`ImageNode` ทำงานจริงครบวงจร ไม่ใช่ icon art จริง
- `IconPlugin` (`init_resource::<IconRegistry>()` + ระบบ `Startup` ที่ลงทะเบียน built-in icon) ต่อเข้ากับ `EditorUiPlugin::build` แบบ idempotent เหมือน `DragAndDropPlugin`/`ScrollbarPlugin` เดิม
- เพิ่ม dependency ใหม่ 3 ตัวใน `bv_editor_ui`: `bevy_asset`, `bevy_image` (สำหรับ `Handle<Image>`/`Image`) และ `wgpu-types` ตรงๆ (สำหรับ `Extent3d`/`TextureDimension`/`TextureFormat` เท่านั้น) — เลือก `wgpu-types` แทน `bevy_render` เพราะเป็น crate ต้นทางที่ `bevy_image` เอง import type พวกนี้มาอยู่แล้ว (ไม่ re-export ให้ใช้ตรงๆ) หนักกว่าเดิมน้อยกว่าการดึง `bevy_render` ทั้งก้อน (ซึ่งลาก `wgpu` เต็มๆ) เข้ามาทั้งที่ `bv_editor_ui` ไม่มี renderer ของตัวเองเลย
- **ผลข้างเคียงที่ต้องแก้:** `EditorUiPlugin` เดิมใช้ headless-test ได้โดยไม่ต้องมี `AssetPlugin` (แค่ `MinimalPlugins`) — พอ `IconPlugin`'s Startup system ต้องการ `ResMut<Assets<Image>>` เทสเดิม 5 เคสใน `bv_editor_ui/src/lib.rs` ที่ compose `EditorUiPlugin::default()` ตรงๆ (ไม่ใช่เทสที่เรียก `spawn_editor_shell` ตรงๆ ซึ่งไม่ผ่าน plugin) ต้องเพิ่ม `AssetPlugin` + `init_asset::<Image>()` ก่อน — เพิ่ม `setup()` helper ในเทสไฟล์นั้นตาม pattern เดียวกับที่ `bv_editor_viewport`/`bv_editor`'s เทสไฟล์ใช้อยู่แล้ว (ไม่ใช่บั๊กใหม่ แค่เป็นผลตรงไปตรงมาจากการเพิ่ม asset dependency)
- เทสใหม่ 5 เคสใน `icon.rs`: unregistered id คืน `None`, register แล้ว resolve ได้ rect/handle ถูกต้อง, `IconPlugin` ลงทะเบียน built-in placeholder จริงตอน startup, `spawn_icon` spawn entity ขนาดถูกและเป็นลูกของ parent จริง, และ `spawn_icon` ไม่ spawn อะไรเลยเมื่อ id ไม่รู้จัก

**ยังไม่ทำ (ตั้งใจเว้นไว้ตามคำแนะนำเดิม "ไม่ต้องรอให้ set icon ครบทุกจุดก่อนเริ่ม"):**
- ยังไม่ได้ต่อเข้ากับ toolbar (ปุ่ม manipulator ยังไม่มีจริง แค่ label ลอย), panel/tab title bar (รอ F5), หรือ Scene Tree row (nice-to-have เดิม) เลยสักจุดเดียว — เมื่อ F5/toolbar จริงเริ่มทำ ค่อยเรียก `spawn_icon`/`IconRegistry::image_node` ที่มีอยู่แล้วได้ทันที ไม่ต้องแก้ core ของ `icon.rs`
- ยังไม่มี built-in icon set ที่แยกตามประเภท entity/panel จริง (มีแค่ placeholder เดียว) และยังไม่มีทาง "extension ใส่ icon ของตัวเองแบบ asset-reference" ที่เป็น API สำเร็จรูป (ตอนนี้ extension เรียก `IconRegistry::register` เองตรงๆ ได้อยู่แล้วผ่าน `ResMut<IconRegistry>` ก็จริง แต่ยังไม่มี helper โหลดจากไฟล์ภาพจริงให้)

**สเปกเดิม (สำหรับอ้างอิง — งานที่ยังไม่ทำด้านบนอิงจากนี้):**

**สถานะปัจจุบันตอนร่างสเปก (ก่อน implement):** ไม่มีเลย — ไม่มี icon asset, icon font, หรือ icon widget ในโค้ดตอนนั้น (ค้นทั้ง workspace ไม่เจอคำว่า "icon" เลยนอกจาก `docs/DESIGN.md` ที่พูดถึง `ManipulatorDescriptor { name, icon, hotkey }` เป็นแค่ field ที่ยังไม่ implement — ดู section 7.2 ของ DESIGN.md) Toolbar/panel title ปัจจุบันเป็นตัวหนังสือล้วน

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

## F7 — Panel: min/max size ที่ยึดอยู่จริง ไม่ใช่แค่ตอนลาก splitter ✅ implemented

**สถานะ:** ทำเสร็จแล้ว — **ง่ายกว่าที่ร่างสเปกไว้ตอนแรกมาก** เพราะระหว่าง implement พบว่า `bevy_ui`'s `Node` มี `min_width`/`max_width`/`min_height`/`max_height` (`Val`) เป็น field จริงอยู่แล้ว ซึ่ง flexbox layout engine (taffy) บังคับใช้เป็น **hard floor/ceiling ทุกเฟรมโดย engine เอง** ไม่ว่าที่มาของการเปลี่ยนขนาดจะเป็นอะไร (ลาก splitter, resize หน้าต่าง, โหลด layout ในอนาคต) — ตรงกับหลักการ "built-in ก่อนเสมอ" section 4 ของ DESIGN.md ยิ่งกว่าแผนเดิมที่จะเขียน custom system ฟัง `WindowResized` เอง (แผนเดิมทิ้งไปทั้งหมด ไม่ต้องมี `PanelSizeBounds` component หรือ `enforce_panel_bounds_on_resize` system ตามที่ร่างไว้ — native `Node.min_*`/`max_*` ทำงานแทนได้หมดโดยไม่ต้องเขียน system ใหม่เลยสักตัว)

การเปลี่ยนแปลงจริงทั้งหมดอยู่ใน [`shell.rs`](../crates/bv_editor_ui/src/shell.rs), ล้วนเป็นการเพิ่ม field ใน `Node` ที่ spawn อยู่แล้ว:

- **ช่องโหว่ 1 (resize หน้าต่างไม่ re-clamp)** — ปิดแล้วโดยอัตโนมัติ: `min_width`/`max_width` (ฝั่ง)/`min_height`/`max_height` (แถวล่าง) เป็น constraint จริงที่ layout engine คำนวณใหม่ทุกเฟรมอยู่แล้ว ไม่ใช่แค่ตอน `Startup`
- **ช่องโหว่ 2 (viewport ไม่มี min)** — viewport ได้ `min_width: VIEWPORT_MIN_WIDTH_PX (200)` + `min_height: VIEWPORT_MIN_HEIGHT_PX (150)` เป็นของตัวเอง และ `middle_row` (แถวกลางที่บรรจุ Scene Tree/Viewport/Components) ก็ได้ `min_height` เท่ากันด้วย เพราะ `middle_row` คือตัวที่แข่งพื้นที่แนวตั้งกับ `bottom_row` โดยตรงใน flex column ของ `root` — ใส่ min ไว้ที่ viewport อย่างเดียวไม่พอสำหรับแกนตั้ง เนื่องจากมันไม่ใช่ direct sibling ของ `bottom_row`
- **ช่องโหว่ 3 (Project Files/Console ไม่มี min)** — ทั้งคู่ได้ `min_width: BOTTOM_PANEL_MIN_WIDTH_PX (120)` แล้ว แม้ยังไม่มี splitter โต้ตอบได้ระหว่างสองอันนี้ (ยังเป็นแค่เส้นแบ่งนิ่งๆ) min ก็ยังมีผลจริงป้องกันไม่ให้ถูกบีบจนหายไปถ้าทั้งแถวถูกบีบสูงสั้นมาก
- **known limitation ที่ยอมรับตามสเปกเดิม:** ถ้าผลรวม min ของทุก panel ในแถวเดียวกันมากกว่าความกว้าง/สูงหน้าต่างจริงๆ (หน้าต่างเล็กมาก) flexbox จะจัดการแบบ CSS มาตรฐาน (บีบตามสัดส่วน `flex_shrink`, ไม่ panic ไม่ตัวเลขติดลบ) แต่ layout อาจดูอึดอัด — ยังไม่มี fallback พิเศษ (เช่น auto-collapse panel) ตามที่ระบุไว้เดิมว่าไม่ต้องปิดรอบนี้
- เทส `panels_carry_real_min_max_size_bounds` ใน [`bv_editor_ui/src/lib.rs`](../crates/bv_editor_ui/src/lib.rs) ตรวจว่าทุก panel (Scene Tree, Components, viewport, bottom row, Project Files, Console) มี `min_width`/`max_width`/`min_height`/`max_height` เป็น `Val::Px` ที่มากกว่า 0 จริง ไม่ใช่แค่ default `Val::Auto`

**Phase:** เติมของ Phase 1 (`bv_editor_ui`) เดียวกับที่ร่างไว้ — ทำเสร็จโดยไม่แตะ F1–F6 เลยตามที่คาด

---

## สรุปลำดับที่แนะนำ (ตอน implement จริง)

ลำดับนี้เรียงตามการพึ่งพากันของฟีเจอร์ ไม่ใช่ความสำคัญ — ต้องคุยและ agree สเปกด้านบนให้เรียบร้อยก่อนเริ่มโค้ดข้อไหนทั้งสิ้น:

1. ~~**F1** (row highlight)~~ ✅ เสร็จแล้ว
2. ~~**F2** (Scene Tree scrollbar)~~ ✅ เสร็จแล้ว — widget กลาง `bv_editor_ui::scrollbar` พร้อมให้ F3 ใช้ซ้ำ
3. ~~**F7** (min/max ที่ยึดอยู่จริง)~~ ✅ เสร็จแล้ว — ทำได้เร็วกว่าคาดเพราะใช้ `Node.min_*`/`max_*` ของ `bevy_ui` ตรงๆ
4. ~~**F3** (Components panel scrollbar)~~ ✅ เสร็จแล้ว — ต่อ widget จาก F2 เข้ากับ Inspector panel ตามแผน
5. ~~**F4 ส่วน cursor feedback**~~ ✅ เสร็จแล้ว — เหลือแค่ persist ขนาดข้ามเซสชัน ผูกกับ F5 ด้านล่าง
6. ~~**F6 ขั้นต่ำ**~~ ✅ เสร็จแล้ว — `IconRegistry` + `spawn_icon` ใช้งานได้จริง (ผ่าน `ImageNode::rect` ไม่ใช่ `TextureAtlas` ตามที่ร่างไว้แต่แรก) แต่ยังไม่ได้ต่อเข้า toolbar/title bar/Scene Tree row จุดไหนเลย รอ F5/toolbar phase เรียกใช้ต่อ
7. **F5** (docking) — ก้อนใหญ่สุด เสี่ยงบานปลายสุด ทำหลังสุด ตรงกับ Phase 10 เดิมของ DESIGN.md รวม F4 ส่วน persist layout เข้าไปพร้อมกัน

ทุกข้อยังต้องมี headless test ตามหลักการ section 8.1 ของ DESIGN.md (`bv_editor_test_utils`) ไม่ใช่ "ดูด้วยตา" อย่างเดียว — resize/scroll/drag math ทั้งหมดควรแยกเป็น pure function เทสได้แบบไม่พึ่ง ECS ก่อน (ตามแนวทางที่ `resize_value` ใน `splitter.rs` วางไว้แล้ว) แล้วค่อยห่อด้วย system บางๆ ต่อ
