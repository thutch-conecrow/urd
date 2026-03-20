# VHS tape: Project initialization demo
# Run from a fresh temp directory

Output vhs/init.gif
Set Shell fish
Set Theme "Catppuccin Mocha"
Set Width 1000
Set Height 500
Set FontSize 16
Set Padding 20
Set TypingSpeed 50ms

# Initialize with explicit environments
Type "urd init --env dev --env prod"
Enter
Sleep 3s

# Show what was created
Type "tree .urd"
Enter
Sleep 2s

# Check status
Type "urd status"
Enter
Sleep 2s

# Store a value
Type "urd set app.name -e dev 'my-app'"
Enter
Sleep 1.5s

# Verify it
Type "urd get app.name"
Enter
Sleep 2s
