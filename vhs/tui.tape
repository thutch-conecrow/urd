# VHS tape: TUI browsing demo
# Run from the demo/ directory with the demo key installed

Output vhs/tui.gif
Set Shell fish
Set Theme "Catppuccin Mocha"
Set Width 1200
Set Height 700
Set FontSize 16
Set Padding 20
Set TypingSpeed 60ms

Type "cd demo && urd"
Enter
Sleep 2s

# Navigate down through items
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms

# Expand an item
Type "l"
Sleep 800ms

# Navigate into env values
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms

# Reveal values for this item
Type "r"
Sleep 1.5s

# Navigate back up
Type "k"
Sleep 400ms
Type "k"
Sleep 400ms
Type "k"
Sleep 400ms

# Collapse
Type "h"
Sleep 600ms

# Expand all items
Type "+"
Sleep 1.5s

# Reveal all values
Type "R"
Sleep 2s

# Hide all values
Type "R"
Sleep 1s

# Collapse all
Type "-"
Sleep 1s

# Use search/filter
Type "/"
Sleep 400ms
Type "stripe"
Sleep 1s
Escape
Sleep 800ms

# Clear filter and quit
Type "/"
Sleep 300ms
Escape
Sleep 500ms
Type "q"
Sleep 500ms
