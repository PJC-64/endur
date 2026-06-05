local telescope = require("telescope")
local finders = require("telescope.finders")
local pickers = require("telescope.pickers")
local conf = require("telescope.config").values
local actions = require("telescope.actions")
local action_state = require("telescope.actions.state")
local previewers = require("telescope.previewers")

-- Helper to trim string
local function trim(s)
  return (s:gsub("^%s*(.-)%s*$", "%1"))
end

local function show_picker(opts)
  opts = opts or {}
  local current_dir = vim.fn.expand("%:p:h")
  local current_file = vim.fn.expand("%:p:t")
  local current_file_path = vim.api.nvim_buf_get_name(0)

  -- Get git root for current dir
  local git_dir = vim.fs.find(".git", { path = current_dir, upward = true })[1]
  if not git_dir then
    vim.notify("endur: Not in a Git repository", vim.log.levels.ERROR)
    return
  end
  local git_root = vim.fs.dirname(git_dir)

  local endur_path = require("endur").config.endur_path
  if vim.fn.executable(endur_path) == 0 then
    vim.notify("endur: executable '" .. endur_path .. "' not found or not executable.", vim.log.levels.ERROR)
    return
  end

  -- Run endur list-snapshots in the git root
  local command = { endur_path, "list-snapshots", git_root }
  local output = vim.fn.systemlist(command)
  if vim.v.shell_error ~= 0 then
    local err = table.concat(output, "\n")
    vim.notify("Failed to fetch Endur snapshots: " .. err, vim.log.levels.ERROR)
    return
  end

  -- Parse snapshots (skip headers)
  local items = {}
  for i = 3, #output do
    local line = output[i]
    if line ~= "" then
      local hash = trim(line:sub(1, 40))
      local datetime = trim(line:sub(42, 66))
      local changes = trim(line:sub(68))
      table.insert(items, {
        hash = hash,
        datetime = datetime,
        changes = changes,
        display = string.format("%s │ %s │ %s", hash:sub(1, 8), datetime, changes)
      })
    end
  end

  pickers.new(opts, {
    prompt_title = "Endur Snapshots (" .. vim.fs.basename(git_root) .. ")",
    finder = finders.new_table({
      results = items,
      entry_maker = function(entry)
        return {
          value = entry,
          display = entry.display,
          ordinal = entry.hash .. " " .. entry.datetime,
        }
      end,
    }),
    sorter = conf.generic_sorter(opts),
    previewer = previewers.new_buffer_previewer({
      title = "Snapshot Diff",
      define_preview = function(self, entry, status)
        local hash = entry.value.hash
        local cmd = { "git", "diff", hash .. "^!" }
        
        vim.fn.jobstart(cmd, {
          stdout_buffered = true,
          on_stdout = function(_, data)
            if data then
              vim.api.nvim_buf_set_lines(self.state.bufnr, 0, -1, false, data)
              vim.api.nvim_set_option_value("filetype", "diff", { buf = self.state.bufnr })
            end
          end,
        })
      end,
    }),
    attach_mappings = function(prompt_bufnr, map)
      -- Enter: Restore entire repository
      actions.select_default:replace(function()
        actions.close(prompt_bufnr)
        local selection = action_state.get_selected_entry()
        local hash = selection.value.hash
        
        vim.notify("Restoring repository to snapshot " .. hash:sub(1, 8) .. "...")
        local out = vim.fn.system({ endur_path, "restore", hash, git_root })
        if vim.v.shell_error ~= 0 then
          vim.notify("endur: restore failed: " .. out, vim.log.levels.ERROR)
        else
          vim.notify("endur: successfully restored repository to snapshot " .. hash:sub(1, 8), vim.log.levels.INFO)
          vim.cmd("edit!")
        end
      end)

      -- Ctrl-F: Discrete restore of current buffer file only
      local function restore_current_file()
        actions.close(prompt_bufnr)
        local selection = action_state.get_selected_entry()
        local hash = selection.value.hash
        
        vim.notify("Restoring " .. current_file .. " to snapshot " .. hash:sub(1, 8) .. "...")
        local out = vim.fn.system({ endur_path, "restore", hash, git_root, "-f", current_file_path })
        if vim.v.shell_error ~= 0 then
          vim.notify("endur: restore failed: " .. out, vim.log.levels.ERROR)
        else
          vim.notify("endur: successfully restored " .. current_file .. " to snapshot " .. hash:sub(1, 8), vim.log.levels.INFO)
          vim.cmd("edit!")
        end
      end

      map("i", "<C-f>", restore_current_file)
      map("n", "<C-f>", restore_current_file)

      return true
    end,
  }):find()
end

return telescope.register_extension({
  exports = {
    snapshots = show_picker,
  },
})
