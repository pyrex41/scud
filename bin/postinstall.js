#!/usr/bin/env node

/**
 * Post-install script for npm
 * Shows helpful information after installation
 */

console.log(`
╭─────────────────────────────────────╮
│                                     │
│  BMAD-TM Lite installed! 🚀         │
│                                     │
╰─────────────────────────────────────╯

To get started in your project:

  1. Initialize BMAD-TM Lite:
     $ bmad-tm init

  2. Check status:
     $ bmad-tm status

  3. Start workflow (in Claude Code):
     $ /tm-pm

📚 Documentation:
   • README.md in node_modules/bmad-tm-lite/
   • Or visit: https://github.com/yourusername/bmad-tm-lite

💡 Commands:
   • bmad-tm init       - Initialize in project
   • bmad-tm status     - Check workflow state
   • bmad-tm validate   - Validate setup
   • bmad-tm help       - Show help

Happy building! 🎉
`);
