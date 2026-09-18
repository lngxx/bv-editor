# Examples

โฟลเดอร์นี้มี `minimal_game`/`third_party_extension` ที่ใช้ทดสอบ bv-editor
ทุก phase ตามที่ระบุไว้ใน [docs/DESIGN.md](../docs/DESIGN.md) หัวข้อ 3 และ 9 —
ทุก phase ใหม่ต้อง build ทับบน `minimal_game` เดิม ไม่ใช่สร้างเกมทดสอบใหม่ทุกครั้ง
กับ `ui_gallery` ที่เป็นคนละวัตถุประสงค์: ไม่ใช่ game ทดสอบ phase แต่เป็น
"ห้องแสดง widget" ไว้ดูด้วยตา/ทดสอบ UI widget เฉพาะของ bv-editor เอง ตาม
[docs/UI_FEATURES.md](../docs/UI_FEATURES.md)

รันคำสั่งทั้งหมดด้านล่างจาก root ของ workspace (`bv-editor/`)

## `minimal_game`

เกมเปล่าๆ (cube + light + camera) ที่ฝัง `EditorPlugin` เข้าไปด้วย
`app.add_plugins(EditorPlugin)` บรรทัดเดียว จำลองการที่เกมจริงเอา bv-editor
ไปฝังโดยไม่ต้องแก้โค้ดเกมของตัวเอง — เป็น binary ที่เปิดหน้าต่าง Bevy จริง

```sh
cargo run -p minimal_game
```

สิ่งที่ควรเห็น: หน้าต่าง Bevy เปิดขึ้นมา ไม่ crash และใน console จะมี log บอกว่า
plugin โหลดสำเร็จ:

```
INFO bv_editor_core: bv_editor_core: EditorCorePlugin loaded
INFO bv_editor: bv_editor: EditorPlugin loaded successfully
```

ตั้งแต่ Phase 1 เป็นต้นไป `EditorPlugin` จะ spawn shell layout ของ editor
(`bv_editor_ui`) เต็มหน้าต่างทับฉาก 3D ของเกมไว้ — เห็นกล่อง Toolbar บนสุด,
Scene Tree ซ้าย, Viewport กลาง, Components ขวา, Project Files/Console ล่าง,
และแถบสถานะล่างสุด (ยังเป็นกล่องว่างมี title เฉยๆ ตามสโคปของ Phase 1) เพราะ
UI วาดทับเต็มจอ จึงยังไม่เห็น cube/light ของเกมจริงจนกว่าจะถึง Phase 4 ที่
viewport เรนเดอร์ฉาก 3D ลงมาแปะในกล่อง Viewport เอง — ลากเส้นบางๆ ระหว่างกล่อง
(splitter) ด้วยเมาส์ซ้ายค้างเพื่อ resize แต่ละโซนได้แล้วตอนนี้

ปิดหน้าต่างตามปกติ (กด X หรือ Alt+F4) เพื่อออกจากโปรแกรม

ถ้าอยาก build โดยไม่รัน (เช่นเช็คว่า compile ผ่าน):

```sh
cargo build -p minimal_game
```

## `ui_gallery`

ไม่ใช่เกม ไม่มี gameplay logic ใดๆ — สร้างขึ้นเพื่อให้มี Scene Tree ที่มี
หลายแถวหลายชั้น + entity ที่มี component ครอบคลุม field editor ทุกชนิดที่
`bv_editor_reflect_ui` รู้จัก ไว้ทดสอบเฉพาะ **widget ของ bv-editor เอง**
(ไม่ใช่ widget มาตรฐานของ Bevy) ตาม [docs/UI_FEATURES.md](../docs/UI_FEATURES.md)

```sh
cargo run -p ui_gallery
```

สิ่งที่ทดสอบได้ในนี้:

- **Scene Tree row hover/selection highlight (F1)** — เอาเมาส์ชี้แถวไหนก็ได้
  (ไม่ต้องคลิก) จะเห็นสีพื้นหลังอ่อนๆ ต่างจากแถวปกติ, คลิกเลือกแล้วสีเปลี่ยน
  เป็นสีเข้มกว่าค้างไว้แม้เอาเมาส์ออกจากแถวนั้นไปแล้ว
- **Splitter ลากปรับขนาด panel ด้วยเมาส์ (F4)** — ลากเส้นบางๆ ระหว่าง Scene
  Tree/Viewport/Components และระหว่าง Project Files/Console
- **Drag-and-drop reparent** — ลากแถวหนึ่งในทรีไปปล่อยบนอีกแถว (เช่นลาก
  `Crate C` ไปวางบน `Lighting`) จะเห็น entity ย้าย parent จริงในทรี
- **Field editor ครบทุกชนิดพร้อมกัน** — เลือก `Crate A` หรือ `Crate B` ใน
  Scene Tree แล้วดู Components panel ขวา: `label` (String, คลิกแล้วพิมพ์ทับ),
  `enabled` (Bool, คลิกแล้ว toggle ทันที), `speed` (F32), `offset` (Vec3 —
  สามช่อง x/y/z), `tint` (Color — สี่ช่อง r/g/b/a), `uv` (Vec2 — ยังไม่มี
  editor เฉพาะ เลยแสดงเป็น read-only `{:?}` แบบ fallback)
- **Scene Tree scrollbar (F2)** — ทรีมี ~18 แถว (หลายกลุ่ม, มีลูกหลาน 2 ชั้นที่
  `Crate A Lid`, บวก `Marker 1`-`Marker 8` เป็น root เดี่ยวๆ) พอจะล้นพื้นที่
  Scene Tree panel เริ่มต้นได้ — ลองหมุนล้อเมาส์เหนือทรี หรือลากแถบเทาบางๆ
  ทางขวาของทรี (scrollbar thumb — โผล่เฉพาะตอนแถวล้นเท่านั้น) ถ้าไม่ล้น ลอง
  ลากแถบ splitter ระหว่าง Scene Tree/Viewport ให้แคบลงก่อน
- **Components panel scrollbar (F3)** — เลือก `Crate A`/`Crate B` แล้วดู
  Components panel ขวา: `Transform` (7 field) + `WidgetShowcase` (10 field)
  รวมกันมักล้นพื้นที่เริ่มต้นแล้ว scrollbar ทำงานแบบเดียวกับ F2 ทุกอย่าง
  ยกเว้นข้อเดียว — ลองเลื่อน scroll ลงไปก่อน แล้วสลับไปเลือก entity อื่น
  (เช่น `Crate C`) จะเห็นว่า scroll เด้งกลับขึ้นบนสุดทันที (ตั้งใจ ต่างจาก F2
  ที่ scroll ของ Scene Tree ควรคงตำแหน่งไว้ข้าม rebuild เล็กๆ)

## `third_party_extension`

crate ตัวอย่างที่จำลองส่วนขยาย (extension) จากภายนอก workspace หลัก — ผูกกับ
public API ของ bv-editor เท่านั้น เหมือนที่ third-party จริงจะทำผ่าน `cargo add`
ไม่ใช่ binary ที่รันได้ (ยังไม่มี panel จริงให้แสดงจนกว่าจะถึง Phase 7 ที่
`bv_editor_extension` มี trait ให้ implement) ตอนนี้มีแค่ `Plugin` เปล่าๆ ที่ log
ว่าโหลดสำเร็จ ใช้ยืนยันด้วย unit test แทน:

```sh
cargo test -p third_party_extension
```

## รันทุกอย่างพร้อมกัน (ทั้ง workspace)

```sh
cargo check --workspace --all-targets   # compile ทุก crate + example + test ให้ครบ ไม่รันจริง
cargo test --workspace                  # รัน automated test ของทุก phase ผ่าน bv_editor_test_utils
```
