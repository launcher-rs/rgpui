# gpui-component Integration Example

This example demonstrates how to integrate gpui-term with gpui-component's theme system, showing:

- ✅ Automatic theme synchronization between gpui-component and terminal
- ✅ Theme palette UI with all available themes
- ✅ Light/Dark mode switching
- ✅ Real-time terminal color updates
- ✅ Theme color preview

## Features

### Theme Switching
- **Theme List**: Browse all available gpui-component themes in the sidebar
- **One-Click Switch**: Click any theme name to apply it to the terminal instantly
- **Mode Toggle**: Switch between Light and Dark modes with Cmd+D (Mac) or Ctrl+Shift+D (Linux/Windows)

### Terminal Integration
- **Auto-Sync**: Terminal colors automatically update when theme changes
- **ANSI Colors**: Full 256-color support mapped from gpui-component theme
- **Color Preview**: See the current theme's terminal colors in the info panel

## Prerequisites

Ensure you have `gpui-component` installed at the correct path:

```bash
# The example expects gpui-component at:
# ../../../gpui-component/crates/ui (relative to this example)
```

## Running

```bash
# From the examples/component-integration directory
cargo run

# Or from the workspace root
cargo run -p component-integration
```

## Keyboard Shortcuts

### macOS
- `Cmd+Q` - Quit
- `Cmd+D` - Toggle Light/Dark mode
- `Cmd+C` - Copy (in terminal)
- `Cmd+V` - Paste (in terminal)
- `Cmd+A` - Select all (in terminal)
- `Cmd+K` - Clear terminal

### Linux/Windows
- `Ctrl+Shift+Q` - Quit
- `Ctrl+Shift+D` - Toggle Light/Dark mode
- `Ctrl+Shift+C` - Copy (in terminal)
- `Ctrl+Shift+V` - Paste (in terminal)
- `Ctrl+Shift+A` - Select all (in terminal)
- `Ctrl+Shift+K` - Clear terminal

## UI Layout

```
┌─────────────────────────────────────────────────────────┐
│  gpui-component Integration Demo    [Switch to Dark]   │ ← Header
├──────────────┬──────────────────────────────────────────┤
│              │                                          │
│  Themes      │                                          │
│  ┌─────────┐ │         Terminal Window                 │
│  │ One Dark│ │                                          │
│  │ Dracula │ │                                          │
│  │ Nord    │ │                                          │
│  │ ...     │ │                                          │
│  └─────────┘ │                                          │
│              │                                          │
│  Colors      │                                          │
│  ■ Foreground│                                          │
│  ■ Background│                                          │
│  ■ Red       │                                          │
│  ■ Green     │                                          │
└──────────────┴──────────────────────────────────────────┘
```

## Code Highlights

### Setting up theme integration:

```rust
// Create terminal view with theme integration
let mut view = TerminalView::new(terminal, window, cx);

// ⭐ Apply gpui-component theme and enable auto-sync
view.apply_component_theme(cx);
view.observe_component_theme(cx);
```

### Switching themes programmatically:

```rust
// Get theme registry
let theme_config = ThemeRegistry::global(cx)
    .themes()
    .get("Catppuccin Latte")
    .cloned()?;

// Apply theme
ComponentTheme::global_mut(cx).apply_config(&theme_config);

// Terminal automatically updates!
```

## Troubleshooting

### "gpui-component not found"

Make sure `gpui-component` is at the correct path. You can symlink it:

```bash
cd ../../..  # Go to parent of gpui-term
ln -s /path/to/your/gpui-component ./gpui-component
```

### Terminal doesn't update when theme changes

Ensure you called `observe_component_theme()`:

```rust
terminal_view.update(cx, |view, cx| {
    view.observe_component_theme(cx);
});
```

## See Also

- [Integration Documentation](../../docs/GPUI_COMPONENT_INTEGRATION.md)
- [gpui-component Repository](https://github.com/longbridge/gpui-component)
- [GPUI Documentation](https://www.gpui.rs/)
