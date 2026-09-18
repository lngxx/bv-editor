# Examples

โฟลเดอร์นี้มี 2 crate ตัวอย่างที่ใช้ทดสอบ bv-editor ทุก phase ตามที่ระบุไว้ใน
[docs/DESIGN.md](../docs/DESIGN.md) หัวข้อ 3 และ 9 — ทุก phase ใหม่ต้อง build
ทับบน `minimal_game` เดิม ไม่ใช่สร้างเกมทดสอบใหม่ทุกครั้ง

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
