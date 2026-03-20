# VHS tape: CLI usage demo
# Run from a temp directory or the demo/ directory

Output vhs/cli.gif
Set Shell fish
Set Theme "Catppuccin Mocha"
Set Width 1000
Set Height 600
Set FontSize 16
Set Padding 20
Set TypingSpeed 50ms

# List all items
Type "cd demo && urd list"
Enter
Sleep 2s

# Get a specific item
Type "urd get supabase.url"
Enter
Sleep 1.5s

# Get with env filter
Type "urd get supabase.url --env prod"
Enter
Sleep 1.5s

# List with tag filter
Type "urd list --tag vendor:stripe"
Enter
Sleep 2s

# Show catalog details
Type "urd catalog show stripe.secret_key"
Enter
Sleep 2s

# Check store status
Type "urd status"
Enter
Sleep 2s
