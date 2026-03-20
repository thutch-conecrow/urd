# VHS tape: Assembly dry-run demo
# Run from the demo/ directory with the demo key installed

Output vhs/assemble.gif
Set Shell fish
Set Theme "Catppuccin Mocha"
Set Width 1100
Set Height 700
Set FontSize 16
Set Padding 20
Set TypingSpeed 50ms

# Show the topology file
Type "cd demo && cat topologies.yaml"
Enter
Sleep 3s

# Dry-run assembly for all-local
Type "urd assemble --topology all-local --dry-run"
Enter
Sleep 3s

# Assemble just one component
Type "urd assemble --topology all-local --dry-run -c api"
Enter
Sleep 3s
