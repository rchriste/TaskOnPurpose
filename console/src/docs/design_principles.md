## Design Principles

### 1. Work with domain types in menus and UI, not raw SurrealDB types

- **Rule**: In `menu`, `display`, and other UI-facing code, work with **domain / processed types** from `base_data` and `node` (for example `Item`, `Mode`, `ItemStatus`, `ModeNode`) instead of raw SurrealDB types (`SurrealItem`, `SurrealMode`, etc.).
- **Why**:
  - Keeps UI code focused on behavior and user intent instead of storage details.
  - Makes it easier to evolve the database schema without touching the menus.
  - Centralizes SurrealDB-specific logic in one layer.
- **Where Surreal\* is allowed**:
  - In `data_storage::surrealdb_layer` and closely-related upgrade/migration code.
  - In conversion/adapter code that turns SurrealDB records into domain types.
- **How to apply**:
  - If a menu needs to manipulate something stored in SurrealDB, **add helper methods on the domain type** (for example, on `Mode` or `Item`) rather than reaching directly into the Surreal\* struct from the menu.
  - Pass `Mode` / `Item` / `ItemStatus` / `ModeNode` into menus, not `SurrealMode` / `SurrealItem`.

### 2. Keep data access and behavior separate

- **Rule**: Data loading, saving, and schema-upgrade logic live in the data-layer (`data_storage::surrealdb_layer`); higher-level behavior and workflows live in `base_data`, `node`, `menu`, and `systems`.
- **Why**:
  - Makes it clear which code can talk to the database.
  - Simplifies testing and reasoning about behavior.
- **How to apply**:
  - If a function needs fresh data from SurrealDB, have it call into a **data-layer helper** (for example, via `SurrealTables::new(...)`) and then immediately wrap the raw results in domain types before proceeding.
  - Avoid passing database handles (`Surreal<Any>`) deep into domain logic; keep those at the data-layer boundary.

### 3. Prefer small, composable helper functions on domain types

- **Rule**: When menu code needs a non-trivial operation or check, prefer to add a method to the relevant domain type instead of inlining logic in the menu.
- **Why**:
  - Keeps menus thin and focused on control flow and prompts.
  - Encourages reuse of domain logic across menus and systems.
- **How to apply**:
  - If a menu has to compute something from a `Mode` or `Item`, add an appropriately named method on that type (or a small helper module) and call it from the menu.

### 4. Use display helper types to render items and modes

- **Rule**: When printing items, modes, or related structures to the console, prefer the existing `display::*` helper types (for example `DisplayItem`, `DisplayItemNode`, `DisplayMode`, `DisplayModeNode`) instead of manually stitching together summaries, types, and parent chains.
- **Why**:
  - Keeps formatting logic centralized and consistent.
  - Makes it easier to change how things look in one place without touching every menu.
- **How to apply**:
  - Use `DisplayItem::new(item)` when you only need to show a single item.
  - Use `DisplayItemNode` / `DisplayModeNode` when you want to show items or modes along with their parents/children or tree context.
  - Avoid raw calls like `item.get_summary()` in menus; reach for a display helper first.
