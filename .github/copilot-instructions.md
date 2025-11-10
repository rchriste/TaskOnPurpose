# OnPurpose AI Coding Agent Instructions

## Project Overview
OnPurpose is a neurodiversity-focused time management application written in Rust. It helps users decide "what to do next" through a terminal-based interface using SurrealDB for persistence and the `inquire` crate for interactive menus.

## Architecture & Core Concepts

### Data Flow Architecture
The application follows a layered architecture:
1. **Data Storage Layer**: Async SurrealDB operations via message passing (`data_storage/surrealdb_layer/`)
2. **Base Data Layer**: Raw data structures (`base_data/` - Item, Event, TimeSpent)
3. **Calculated Data Layer**: Derived relationships (`calculated_data/` - ParentLookup, item hierarchies)
4. **Node Layer**: Business logic models (`node/` - ItemNode with dependencies, urgency, actions)
5. **Systems Layer**: High-level business objects (`systems/do_now_list/`)
6. **Menu/Display Layer**: UI and formatting (`menu/inquire/`, `display/`)

### Key Domain Models
- **Item**: Core work unit with hierarchical parent-child relationships via `smaller_items_in_priority_order`
- **ItemNode**: Augmented Item with computed dependencies, urgency plans, and action recommendations
- **ActionWithItemStatus**: Represents what the user should do (MakeProgress, ReviewItem, SetReadyAndUrgency, etc.)
- **CurrentMode**: Work/life balance filter (core vs non-core motivational purposes)

### Node System Pattern
Items form complex graphs via parent-child relationships. The `node/item_node.rs` creates:
- `GrowingItemNode`: Parent chain traversal with circular reference protection
- `ShrinkingItemNode`: Child traversal with visited tracking
- Dependency resolution including time-based, item-based, and event-based dependencies

## Development Workflows

### Building & Running
```bash
# Development with in-memory database
cargo run -- inmemorydb

# Production with persistent database  
cargo run

# Install as binary
cargo install --path console
```

### Key Dependencies
- **SurrealDB**: Embedded database (`surrealdb-alpha = "2.0.9"`)
- **Inquire**: Terminal UI menus (custom fork: `git = "https://github.com/rchriste/inquire"`)
- **Tokio**: Async runtime for database operations
- **ahash**: Performance-critical hashmap operations (10x faster than std)
- **mimalloc**: Memory allocator (~15% performance improvement)

### Testing Patterns
Unit tests focus on circular reference detection in item hierarchies. See `node/item_node.rs` tests for examples of creating `SurrealItemBuilder` test fixtures.

## Code Patterns & Conventions

### Message Passing Pattern
Data operations use async channels:
```rust
// Main -> Data Storage Layer communication
let (send_to_data_storage_layer_tx, receive_rx) = mpsc::channel(20);
send_to_data_storage_layer_tx.send(DataLayerCommands::CreateItem(..)).await
```

### Display Trait Pattern  
All UI items implement `Display` with context-aware formatting:
```rust
pub struct DisplayActionWithItemStatus<'s> {
    item: &'s ActionWithItemStatus<'s>,
    filter: Filter,
    display_format: DisplayFormat,
}
```

### Builder Pattern Usage
Complex data structures use derive_builder:
```rust
#[derive(Builder)]
struct SurrealItem { /* fields */ }
```

### Lifetime Management
Extensive use of lifetime annotations for zero-copy data processing. The `'s` lifetime typically represents the lifetime of source data.

### Error Handling Strategy
Currently uses `Box<dyn std::error::Error>` for simplicity. Comments suggest future migration to `anyhow` (applications) or `thiserror` (libraries).

## Critical Implementation Details

### Performance Considerations
- `debug = false` in dev profile for faster compile times
- Custom allocator and hasher for data-intensive operations
- Database query timing monitoring (warns if >1 second)

### Data Persistence
Database location: `"file://c:/.on_purpose.db"` (Windows-specific, Linux TODO noted)

### Circular Reference Handling
The codebase has sophisticated circular reference detection in item relationships using `Visited` tracking. This is critical for the hierarchical item system.

### Unicode/Emoji Support
Requires Windows Terminal with Atlas rendering engine enabled for proper emoji display.

## Integration Points

### External System Integration (Planned)
- Microsoft Graph To Do APIs via OneDrive sync
- Windows API integration for system-level features

### Extension Points
- Menu system in `menu/inquire/` follows consistent patterns for adding new interactive flows
- Display formatters in `display/` for new presentation needs
- Action types in `node/action_with_item_status.rs` for new user workflows

## Common Pitfalls

1. **Lifetime Issues**: The complex lifetime relationships require careful attention to data ownership
2. **Circular Dependencies**: Always check for circular references when modifying item relationships
3. **Database Operations**: All database operations must be async and go through the message-passing layer
4. **Performance**: Be mindful of data structure choices - this app processes complex relationship graphs

## Key Files for Understanding
- `console/src/main.rs` - Application entry and architecture setup
- `node/item_node.rs` - Core domain model and relationship handling
- `menu/inquire/do_now_list_menu.rs` - Main user interaction flow
- `systems/do_now_list/mod.rs` - High-level business logic coordination