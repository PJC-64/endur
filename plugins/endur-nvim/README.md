# endur-nvim

A Neovim plugin for the [endur](https://github.com/PJC-64/endur) Git auto-backup tool.

## Features
- **Auto-Watch**: Automatically registers git repositories with `endur watch` on buffer write events.
- **Commands**: Manual commands to watch and manage files within Neovim.

## Installation

Using [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  "PJC-64/endur-nvim",
  config = function()
    require("endur").setup({
      auto_watch = true,
    })
  end
}
```

## Usage
- `:EndurWatch` — Manually registers the current file's repository path with Endur.
- `:EndurSnapshots` — Opens the Telescope snapshot picker to view or restore snapshots.

## Statusline Integration

### 1. Built-in Statusline
Set `statusline = true` in setup options, then append `%{%v:lua.require('endur').statusline()%}` to your `statusline` setting.

### 2. lualine.nvim Integration
You can integrate `endur-nvim` with `lualine.nvim` by adding a custom component to your lualine configuration:

```lua
require("lualine").setup({
  sections = {
    lualine_x = {
      {
        function()
          return require("endur").statusline_raw()
        end,
        color = function()
          local status_info = require("endur").get_current_repo_status()
          if status_info and status_info.status == "OK" then
            return "EndurClean"
          else
            return "EndurModified"
          end
        end,
        cond = function()
          return require("endur").get_current_repo_status() ~= nil
        end,
      },
    },
  },
})
```

